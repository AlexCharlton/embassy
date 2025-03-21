#[allow(missing_docs)]
pub mod codes;
#[allow(missing_docs)]
pub mod descriptors;

use super::{EnumerationInfo, RegisterError};
use aligned::{Aligned, A4};
use embassy_usb_driver::host::{channel, RequestType, SetupPacket, UsbChannel, UsbHostDriver};
use embassy_usb_driver::{EndpointInfo, EndpointType};
use heapless::Vec;

const MAX_RANGES: usize = 16;

pub struct UadHandler<H: UsbHostDriver> {
    control_channel: H::Channel<channel::Control, channel::InOut>,
}

impl<H: UsbHostDriver> UadHandler<H> {
    pub async fn try_register(host: &H, enum_info: EnumerationInfo) -> Result<Self, RegisterError> {
        let audio_interface_collection =
            match descriptors::AudioInterfaceCollection::try_from_configuration(&enum_info.cfg_desc) {
                Ok(collection) => collection,
                Err(e) => {
                    warn!("Failed to parse Audio Interface Collection: {:#?}", e);
                    return Err(RegisterError::NoSupportedInterface);
                }
            };

        debug!("[UAD] Audio Interface Collection: {:#?}", audio_interface_collection);
        let input_terminal = audio_interface_collection
            .control_interface
            .terminal_descriptors
            .iter()
            .find(|(id, t)| match t {
                descriptors::TerminalDescriptor::Input(input) => {
                    input.terminal_type == descriptors::TerminalType::UsbStreaming
                }
                _ => false,
            })
            .unwrap();

        debug!("[UAD] Input Terminal: {:#?}", input_terminal);

        let mut control_channel = host.alloc_channel::<channel::Control, channel::InOut>(
            enum_info.device_address,
            &EndpointInfo::new(
                0.into(),
                EndpointType::Control,
                (enum_info.device_desc.max_packet_size0 as u16).min(enum_info.speed.max_packet_size()),
            ),
            enum_info.ls_over_fs,
        )?;

        let packet = SetupPacket {
            request_type: RequestType::IN | RequestType::TYPE_CLASS | RequestType::RECIPIENT_INTERFACE,
            request: codes::request_code::RANGE,
            value: codes::control_selector::clock_source::SAMPLING_FREQ_CONTROL,
            index: input_terminal.1.clock_source_id() as u16,
            length: size_of::<Layout3ParameterBlock>() as u16,
        };

        let mut buf = Aligned::<A4, _>([0; size_of::<Layout3ParameterBlock>()]);

        control_channel.control_out(&packet, buf.as_mut_slice()).await.unwrap();

        let layout = Layout3ParameterBlock::try_from_bytes(buf.as_slice()).unwrap();
        debug!("[UAD] Frequency Ranges: {:#?}", layout);

        Ok(Self { control_channel })
    }
}

#[derive(Debug)]
pub struct Layout1ParameterBlock {
    pub ranges: Vec<Range1, MAX_RANGES>,
}

impl Layout1ParameterBlock {
    pub fn try_from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 5 {
            return None;
        }
        let num_ranges = u16::from_le_bytes([bytes[0], bytes[1]]);
        if num_ranges > MAX_RANGES as u16 {
            warn!(
                "Number of ranges in Get Range response is greater than the maximum allowed: {}",
                num_ranges
            );
            return None;
        }
        let mut ranges = Vec::new();
        let mut bytes = &bytes[2..];
        for _ in 0..num_ranges {
            let range = Range1 {
                min: bytes[0],
                max: bytes[1],
                step: bytes[2],
            };
            ranges.push(range).unwrap();
            bytes = &bytes[3..];
        }
        Some(Self { ranges })
    }
}

#[derive(Debug)]
pub struct Layout2ParameterBlock {
    pub ranges: Vec<Range2, MAX_RANGES>,
}

impl Layout2ParameterBlock {
    pub fn try_from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 8 {
            return None;
        }
        let num_ranges = u16::from_le_bytes([bytes[0], bytes[1]]);
        if num_ranges > MAX_RANGES as u16 {
            warn!(
                "Number of ranges in Get Range response is greater than the maximum allowed: {}",
                num_ranges
            );
            return None;
        }
        let mut ranges = Vec::new();
        let mut bytes = &bytes[2..];
        for _ in 0..num_ranges {
            let range = Range2 {
                min: u16::from_le_bytes([bytes[0], bytes[1]]),
                max: u16::from_le_bytes([bytes[2], bytes[3]]),
                step: u16::from_le_bytes([bytes[4], bytes[5]]),
            };
            ranges.push(range).unwrap();
            bytes = &bytes[6..];
        }
        Some(Self { ranges })
    }
}

#[derive(Debug)]
pub struct Layout3ParameterBlock {
    pub ranges: Vec<Range4, MAX_RANGES>,
}

impl Layout3ParameterBlock {
    pub fn try_from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 14 {
            return None;
        }
        let mut ranges = Vec::new();
        let num_ranges = u16::from_le_bytes([bytes[0], bytes[1]]);
        if num_ranges > MAX_RANGES as u16 {
            warn!(
                "Number of ranges in Get Range response is greater than the maximum allowed: {}",
                num_ranges
            );
            return None;
        }
        let mut bytes = &bytes[2..];
        for _ in 0..num_ranges {
            let range = Range4 {
                min: u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
                max: u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
                step: u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
            };
            ranges.push(range).unwrap();
            bytes = &bytes[12..];
        }
        Some(Self { ranges })
    }
}

#[derive(Debug)]
pub struct Range4 {
    pub min: u32,
    pub max: u32,
    pub step: u32,
}

#[derive(Debug)]
pub struct Range2 {
    pub min: u16,
    pub max: u16,
    pub step: u16,
}

#[derive(Debug)]
pub struct Range1 {
    pub min: u8,
    pub max: u8,
    pub step: u8,
}
