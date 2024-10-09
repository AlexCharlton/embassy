//! Host driver for USB hubs.
//!
//! It has it's own enumerate implementation to deal with the deferred `bus_reset` and state/speed detection.
//! It requires the usb-driver to implement/support `Interrupt` `ChannelIn` endpoints (which resolves a call to `[ChannelIn::read]`).

use core::num::NonZeroU8;

use super::{EnumerationInfo, HandlerEvent, RegisterError, UsbHostHandler};
use crate::{
    control::Request,
    host::{
        descriptor::{InterfaceDescriptor, USBDescriptor},
        ControlChannelExt,
    },
};
use embassy_time::Timer;
use embassy_usb_driver::{
    host::{channel, EndpointDescriptor, HostError, RequestType, SetupPacket, UsbChannel, UsbHostDriver},
    EndpointType,
};
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

pub struct HubHandler<H: UsbHostDriver, const MAX_PORTS: usize> {
    interrupt_channel: H::Channel<channel::Interrupt, channel::In>,
    interrupt_ep: EndpointDescriptor,
    control_channel: H::Channel<channel::Control, channel::InOut>,
    desc: HubDescriptor,
    device_address: u8,
    device_lut: [Option<NonZeroU8>; MAX_PORTS],
}

pub enum HubEvent {
    DeviceDetected { port: u8, speed: Speed },
    DeviceRemoved { address: Option<NonZeroU8>, port: u8 },
}

impl<H: UsbHostDriver, const MAX_PORTS: usize> UsbHostHandler for HubHandler<H, MAX_PORTS> {
    type PollEvent = HubEvent;
    type Driver = H;

    fn static_spec() -> super::StaticHandlerSpec {
        super::StaticHandlerSpec {
            device_filter: Some(super::DeviceFilter {
                base_class: Some(unsafe { NonZeroU8::new_unchecked(0x09) }), // Hub
                sub_class: Some(0x00),
                protocol: None, // 00 for FS, otherwise HS or higher
            }),
        }
    }

    async fn try_register(bus: &mut H, device_address: u8, enum_info: EnumerationInfo) -> Result<Self, RegisterError> {
        let control_ep = EndpointDescriptor::control(
            device_address,
            enum_info
                .device_desc
                .max_packet_size0
                .min(if enum_info.ls_over_fs { 8 } else { 64 }) as u16,
        );

        let iface = enum_info
            .cfg_desc
            .iter_interface()
            .find(|v| match v {
                InterfaceDescriptor {
                    interface_class: 0x09,
                    interface_subclass: 0x0,
                    interface_protocol: 0x0,
                    ..
                } => true,
                _ => false,
            })
            .ok_or(RegisterError::NoSupportedInterface)?;

        // TODO: check if IN
        let interrupt_ep = iface
            .iter_endpoints()
            .find(|v| v.ep_type() == EndpointType::Interrupt)
            .ok_or(RegisterError::NoSupportedInterface)?;

        let interrupt_channel =
            bus.alloc_channel::<channel::Interrupt, channel::In>(device_address, &interrupt_ep, enum_info.ls_over_fs)?;

        let mut control_channel =
            bus.alloc_channel::<channel::Control, channel::InOut>(device_address, &control_ep, enum_info.ls_over_fs)?;

        let desc = control_channel.request_descriptor::<HubDescriptor, 64>(true).await?;

        let mut hub = HubHandler {
            interrupt_channel,
            interrupt_ep,
            control_channel,
            desc,
            device_address,
            device_lut: [None; MAX_PORTS],
        };

        // Power-On ports
        for port in 0..hub.desc.port_num {
            hub.port_feature(true, PortFeature::Power, port, 0).await?;
        }

        Timer::after_millis(hub.desc.power_on_delay as u64 * 2).await;

        Ok(hub)
    }

    async fn wait_for_event(&mut self) -> Result<HandlerEvent<HubEvent>, HostError> {
        loop {
            // Wait for status change
            let mut buf = [0u8; 16];
            // 1 bit per port + 1 reserved
            let slice = &mut buf[..(self.desc.port_num as usize / 8) + 1];

            // TODO: attempt to handle the interval in the UsbHostDriver (sometimes done in HW)
            Timer::after_millis(self.interrupt_ep.interval as u64).await;
            self.interrupt_channel.request_in(slice).await?;

            // Find first changed port
            let changed = slice.iter().enumerate().find(|(_, v)| v.leading_zeros() < 7);

            if let Some((idx, v)) = changed {
                let idx = idx as u8;
                trace!(
                    "HUB {}: port {} is changed, requesting status",
                    self.device_address,
                    idx
                );

                // Target to self
                // self.control_channel = UsbChannel::retarget_channel(
                //     self.control_channel,
                //     self.device_address,
                //     &EndpointDescriptor::control(0, 64),
                //     false,
                // )
                // .unwrap();

                // Get status
                let (status, change) = self.get_port_status(idx).await?;
                debug!(
                    "HUB {}: port {} status: {} change: {}",
                    self.device_address, idx, status, change
                );

                // TODO: Overcurrent protection
                // Clear reset change after reset
                if change.contains(PortStatusChange::RESET) {
                    self.port_feature(false, PortFeature::ChangeReset, idx, 0).await?;
                }

                if change.contains(PortStatusChange::CONNECT) {
                    // Clear connect status change
                    self.port_feature(false, PortFeature::ChangeConnection, idx, 0).await?;
                    match status.contains(PortStatus::CONNECTED) {
                        // Device connected, perform bus reset and configure
                        true => {
                            // Determine speed
                            let speed = Speed::from_status(status);

                            debug!(
                                "HUB {}: Device connected to port {} with {} speed",
                                self.device_address, idx, speed
                            );

                            // User can now `enumerate_port`
                            return Ok(HandlerEvent::HandlerEvent(HubEvent::DeviceDetected {
                                port: idx,
                                speed,
                            }));
                        }
                        // Device disconnected, remove from registry
                        false => {
                            debug!("HUB {}: Device disconnected from port {}", self.device_address, idx);
                            let device_ref = self.device_lut.get_mut(idx as usize);
                            return Ok(HandlerEvent::HandlerEvent(HubEvent::DeviceRemoved {
                                address: device_ref.and_then(|v| v.take()),
                                port: idx,
                            }));
                        }
                    }
                }
            }
        }
    }
}

impl<H: UsbHostDriver, const MAX_PORTS: usize> HubHandler<H, MAX_PORTS> {
    async fn enumerate_port(&mut self, port: u8, speed: Speed) -> Result<EnumerationInfo, HostError> {
        // NOTE: we probably could do this in the wait loop but it would require a arc mutex registry which seems unnecessary
        // TODO: add registry as parameter (or the next device_id), and add device to our lut
        todo!();

        // self.port_feature(true, PortFeature::Reset, port, 0).await?;
        // Timer::after_millis(50).await;
        // self.port_feature(false, PortFeature::ChangeReset, port, 0).await?;

        // return super::configure_device(
        //     self.control.driver,
        //     &mut self.control.channel,
        //     self.control.registry,
        //     needs_pre,
        //     Some((self.addr, port)),
        // ).await;
    }

    /// Set/Clear PortFeature
    ///
    /// USB 2.0 Spec: 11.24.2.13,1
    async fn port_feature(&mut self, set: bool, feature: PortFeature, port: u8, selector: u8) -> Result<(), HostError> {
        let setup = SetupPacket {
            request_type: RequestType::OUT | RequestType::TYPE_CLASS | RequestType::RECIPIENT_OTHER,
            request: if set {
                Request::SET_FEATURE
            } else {
                Request::CLEAR_FEATURE
            },
            value: feature as u16,
            index: (selector as u16) << 8 | (port + 1) as u16,
            length: 0,
        };

        self.control_channel.control_out(&setup, &mut []).await?;
        Ok(())
    }

    /// GetPortStatus
    ///
    /// USB 2.0 Spec: 11.24.2.7
    async fn get_port_status(&mut self, port: u8) -> Result<(PortStatus, PortStatusChange), HostError> {
        let setup = SetupPacket {
            request_type: RequestType::IN | RequestType::TYPE_CLASS | RequestType::RECIPIENT_OTHER,
            request: Request::GET_STATUS,
            value: 0,
            index: (port + 1) as u16,
            length: 4,
        };

        let mut buf = [0u16; 2];

        self.control_channel.control_in(&setup, buf.as_mut_bytes()).await?;

        let status = PortStatus::from_bits_truncate(buf[0]);
        let change = PortStatusChange::from_bits_truncate(buf[1]);

        Ok((status, change))
    }
}

/// USB 2.0 Spec heading: 11.23.2.1
#[derive(KnownLayout, FromBytes, Immutable, Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(C)]
struct HubDescriptor {
    len: u8,
    desc_type: u8,
    port_num: u8,
    characteristics0: u8,
    characteristics1: u8,
    /// ms x 2
    power_on_delay: u8,
    /// Maximum controller current
    max_current: u8,
    /// 8 + 8 bits per port, at max 127 ports => 32 bytes
    port_buf: [u8; 32],
}

impl USBDescriptor for HubDescriptor {
    const SIZE: usize = size_of::<Self>();

    const DESC_TYPE: u8 = 0x29;

    type Error = ();

    fn try_from_bytes(bytes: &[u8]) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let (byref, _) = Self::ref_from_prefix(bytes).map_err(|_| ())?;
        if byref.desc_type != Self::DESC_TYPE {
            return Err(());
        }

        Ok(byref.clone())
    }
}

/// USB 2.0 Spec: 11.24.2 Table 11-17
#[derive(Clone, Copy)]
#[repr(u8)]
enum PortFeature {
    Connection = 0,
    Enable,
    Suspend,
    OverCurrent,
    Reset,
    Power = 8,
    LowSpeed,
    ChangeConnection = 16,
    ChangeEnable,
    ChangeSuspend,
    ChangeOverCurrent,
    ChangeReset,
    Test,
    Indicator,
}

bitflags! {
    /// USB 2.0 Spec: 11.24.2.7.1
    struct PortStatus: u16 {
        const CONNECTED   = 1 << 0;
        const ENABLED     = 1 << 1;
        const SUSPENDED   = 1 << 2;
        const OVERCURRENT = 1 << 3;
        const RESET       = 1 << 4;
        // Reserved: 5..8
        const POWERED     = 1 << 8;
        const LOW_SPEED   = 1 << 9;
        const HIGH_SPEED  = 1 << 10;
        const TEST_MODE   = 1 << 11;
        const INDICATOR_CUSTOM_COLOR = 1 << 12;
        // Reserved: 13..16
    }
}

bitflags! {
    /// USB 2.0 Spec: 11.24.2.7.2
    struct PortStatusChange: u16 {
        const CONNECT     = 1 << 0;
        const ENABLE      = 1 << 1;
        const SUSPEND     = 1 << 2;
        const OVERCURRENT = 1 << 3;
        const RESET       = 1 << 4;
        // Reserved: 5..16
    }
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
enum Speed {
    Low,
    Full,
    High,
}

impl Speed {
    pub fn from_status(status: PortStatus) -> Self {
        let ls = status.contains(PortStatus::LOW_SPEED);
        let hs = status.contains(PortStatus::HIGH_SPEED);

        match (ls, hs) {
            (true, _) => Speed::Low,
            (false, false) => Speed::Full,
            (false, true) => Speed::High,
        }
    }
}
