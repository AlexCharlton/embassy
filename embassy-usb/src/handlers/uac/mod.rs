#[allow(missing_docs)]
pub mod codes;
#[allow(missing_docs)]
pub mod descriptors;

use crate::control::Request;

use super::{EnumerationInfo, RegisterError};
use crate::host::descriptor::DEFAULT_MAX_DESCRIPTOR_SIZE;
use aligned::{Aligned, A4};
use embassy_time::{Duration, Instant, Timer};
use embassy_usb_driver::host::{channel, ChannelError, HostError, RequestType, SetupPacket, UsbChannel, UsbHostDriver};
use embassy_usb_driver::{Direction, EndpointInfo, EndpointType, Speed};
use heapless::{String, Vec};

const MAX_RANGES: usize = 16;
// 256 is the maximum buffer size that can be used to store a string
const MAX_STRING_BUF_SIZE: usize = 255;
// But because these are UTF-16 strings, we can only store 127 characters (first two bytes are part of the header)
const MAX_STRING_LENGTH: usize = 127;

pub struct UacHandler<H: UsbHostDriver> {
    pub interface_collection: descriptors::AudioInterfaceCollection,
    pub control_channel: H::Channel<channel::Control, channel::InOut>,
    pub output_channel: Option<H::Channel<channel::Isochronous, channel::Out>>,
    pub feedback_channel: Option<H::Channel<channel::Isochronous, channel::In>>,
    input_terminal_id: u8,
    output_interface_idx: usize,
    speed: Speed,
}

#[derive(Debug)]
pub enum RequestError {
    RequestFailed(ChannelError),
    DeviceDisconnected,
    InvalidResponse,
    NoSupportedInterface,
}

impl<H: UsbHostDriver> UacHandler<H> {
    pub async fn try_register(host: &H, enum_info: EnumerationInfo) -> Result<Self, RegisterError> {
        // Steps taken:
        // 1. Find the first streaming interface with an output endpoint
        // 2. Connect it to its terminal to find the sampling frequency
        // 3. Check its format type
        // 4. Select the right alternate setting for the interface with a SET_INTERFACE request
        // 5. Allocate up the output channel and the corresponding feedback channel, store on Self

        let mut control_channel = host.alloc_channel::<channel::Control, channel::InOut>(
            enum_info.device_address,
            &EndpointInfo::new(
                0.into(),
                EndpointType::Control,
                (enum_info.device_desc.max_packet_size0 as u16).min(enum_info.speed.max_packet_size()),
            ),
            enum_info.ls_over_fs,
        )?;

        let mut cfg_desc_buf = [0u8; DEFAULT_MAX_DESCRIPTOR_SIZE];
        let configuration = enum_info
            .active_config_or_set_default(&mut control_channel, &mut cfg_desc_buf)
            .await?;

        let interface_collection = match descriptors::AudioInterfaceCollection::try_from_configuration(&configuration) {
            Ok(collection) => collection,
            Err(e) => {
                warn!("Failed to parse Audio Interface Collection: {:#?}", e);
                return Err(RegisterError::NoSupportedInterface);
            }
        };
        debug!("[UAC] Audio Interface Collection: {:#?}", interface_collection);

        // Find the first streaming interface with an output endpoint
        let output_interface_idx = interface_collection
            .audio_streaming_interfaces
            .iter()
            .position(|i| {
                i.endpoint_descriptor
                    .map(|e| e.ep_dir() == Direction::Out)
                    .unwrap_or(false)
            })
            .ok_or(RegisterError::NoSupportedInterface)?;
        let output_interface = &interface_collection.audio_streaming_interfaces[output_interface_idx];

        // Check to see that the terminal link id is valid
        let input_terminal = interface_collection
            .control_interface
            .terminal_descriptors
            .get(&output_interface.class_descriptor.terminal_link_id)
            .ok_or(RegisterError::NoSupportedInterface)?;

        debug!("[UAC] Input Terminal: {:#?}", input_terminal);

        // Check to see that the format is PCM
        if output_interface.class_descriptor.format != codes::format_type::Format::Type1(codes::format_type::Type1::PCM)
        {
            error!(
                "[UAC] Only PCM format is supported, got {:?}",
                output_interface.class_descriptor.format
            );
            return Err(RegisterError::NoSupportedInterface);
        }

        // Find the interface with the most endpoints
        let streaming_interface = output_interface
            .interface_descriptors
            .iter()
            .max_by_key(|i| i.num_endpoints)
            .ok_or(RegisterError::NoSupportedInterface)?;

        // Allocate the channels
        let mut output_channel = None;
        let mut feedback_channel = None;
        let input_terminal_id = output_interface.class_descriptor.terminal_link_id;
        // Select the correct alternate setting
        let packet = SetupPacket {
            request_type: RequestType::OUT | RequestType::TYPE_STANDARD | RequestType::RECIPIENT_INTERFACE,
            request: Request::SET_INTERFACE,
            value: streaming_interface.alternate_setting as u16,
            index: streaming_interface.interface_number as u16,
            length: 0,
        };
        control_channel
            .control_out(&packet, &mut [])
            .await
            .map_err(|e| RegisterError::HostError(HostError::ChannelError(e)))?;
        debug!(
            "[UAC] Set output interface to alternate setting: {}",
            streaming_interface.alternate_setting
        );

        if streaming_interface.num_endpoints > 0 {
            output_channel = Some(host.alloc_channel::<channel::Isochronous, channel::Out>(
                enum_info.device_address,
                &output_interface.endpoint_descriptor.unwrap().into(),
                false,
            )?);
        }
        if streaming_interface.num_endpoints > 1 {
            if let Some(feedback_endpoint) = output_interface.feedback_endpoint_descriptor {
                feedback_channel = Some(host.alloc_channel::<channel::Isochronous, channel::In>(
                    enum_info.device_address,
                    &feedback_endpoint.into(),
                    false,
                )?);
            }
        }

        Ok(Self {
            interface_collection,
            control_channel,
            output_channel,
            feedback_channel,
            input_terminal_id,
            output_interface_idx,
            speed: enum_info.speed,
        })
    }

    pub fn output(&mut self) -> Result<UacOut<H>, RequestError> {
        if self.output_channel.is_none() || self.feedback_channel.is_none() {
            error!("[UAC] Output or feedback channel not allocated");
            return Err(RequestError::DeviceDisconnected);
        }
        let output_interface = self.output_interface();
        let num_channels = output_interface.class_descriptor.num_channels;
        let lock_delay = if let Some(desc) = &output_interface.audio_endpoint_descriptor {
            if desc.lock_delay_units == 1 {
                LockDelay::Milliseconds(desc.lock_delay)
            } else if desc.lock_delay_units == 2 {
                LockDelay::Samples(desc.lock_delay)
            } else {
                LockDelay::Samples(0)
            }
        } else {
            LockDelay::Samples(0)
        };
        let bytes_per_sample = match output_interface.format_type_descriptor {
            Some(descriptors::FormatTypeDescriptor::I(descriptors::FormatTypeI { subslot_size, .. })) => {
                subslot_size as usize
            }
            _ => return Err(RequestError::NoSupportedInterface),
        };
        let max_bytes_per_packet = output_interface.endpoint_descriptor.unwrap().max_packet_size as usize;

        Ok(UacOut::<H> {
            output_channel: self.output_channel.take().unwrap(),
            feedback_channel: self.feedback_channel.take().unwrap(),
            speed: self.speed,
            samples_per_microframe: 0.0,
            microframes_per_microsecond: 1.0 / (if self.speed == Speed::High { 125.0 } else { 1000.0 }),
            send_start: Instant::from_ticks(0),
            num_frames: 0,
            last_feedback_time: Instant::from_ticks(0),
            lock_delay,
            num_channels,
            bytes_per_sample,
            max_bytes_per_packet,
        })
    }

    pub fn input_terminal(&self) -> &descriptors::TerminalDescriptor {
        &self.interface_collection.control_interface.terminal_descriptors[&self.input_terminal_id]
    }

    pub fn output_interface(&self) -> &descriptors::AudioStreamingInterface {
        &self.interface_collection.audio_streaming_interfaces[self.output_interface_idx]
    }

    pub async fn get_sampling_freq(&mut self, terminal_id: u8) -> Result<u32, RequestError> {
        self.get_curr_entity3(
            codes::control_selector::clock_source::SAMPLING_FREQ_CONTROL,
            0,
            terminal_id,
            0,
        )
        .await
    }

    //-------------------------------------------------------
    // MARK: Getters
    // For the control interface

    pub async fn get_supported_language(&mut self) -> Result<u16, RequestError> {
        let packet = SetupPacket {
            request_type: RequestType::IN | RequestType::TYPE_STANDARD | RequestType::RECIPIENT_DEVICE,
            request: Request::GET_DESCRIPTOR,
            value: 0x0300, // String descriptor at index 0x00
            index: 0x00,
            length: 4,
        };

        let mut buf = Aligned::<A4, _>([0; 4]);

        self.control_channel
            .control_in(&packet, buf.as_mut_slice())
            .await
            .map_err(|e| RequestError::RequestFailed(e))?;

        Ok(u16::from_le_bytes([buf[2], buf[3]]))
    }

    pub async fn get_string(
        &mut self,
        index: crate::StringIndex,
        lang_id: u16,
    ) -> Result<String<MAX_STRING_LENGTH>, RequestError> {
        // First, get just the length
        let packet = SetupPacket {
            request_type: RequestType::IN | RequestType::TYPE_STANDARD | RequestType::RECIPIENT_DEVICE,
            request: Request::GET_DESCRIPTOR,
            value: (0x03 << 8) | index.0 as u16,
            index: lang_id,
            length: 2, // Just get the length byte and type byte
        };

        let mut length_buf = Aligned::<A4, _>([0; 2]);
        self.control_channel
            .control_in(&packet, length_buf.as_mut_slice())
            .await
            .map_err(|e| RequestError::RequestFailed(e))?;

        if length_buf[1] != 0x03 {
            return Err(RequestError::InvalidResponse);
        }

        let total_length = length_buf[0] as u16;
        if total_length == 0 || total_length > MAX_STRING_BUF_SIZE as u16 {
            return Err(RequestError::InvalidResponse);
        }

        // Now get the full string with the correct length
        let packet = SetupPacket {
            request_type: RequestType::IN | RequestType::TYPE_STANDARD | RequestType::RECIPIENT_DEVICE,
            request: Request::GET_DESCRIPTOR,
            value: (0x03 << 8) | index.0 as u16,
            index: lang_id,
            length: total_length,
        };

        let mut buf = Aligned::<A4, _>([0; MAX_STRING_BUF_SIZE]);
        self.control_channel
            .control_in(&packet, &mut buf.as_mut_slice()[..total_length as usize])
            .await
            .map_err(|e| RequestError::RequestFailed(e))?;

        // Rest of the string parsing code...
        let mut buf = &buf.as_mut_slice()[2..total_length as usize];
        let mut str = String::new();
        while buf.len() >= 2 {
            let b = u16::from_le_bytes([buf[0], buf[1]]);
            if b == 0 {
                break;
            }
            if let Some(c) = char::from_u32(b as u32) {
                str.push(c).unwrap(); // We know we won't exceed the buffer size
            } else {
                return Err(RequestError::InvalidResponse);
            }
            buf = &buf[2..];
        }

        Ok(str)
    }

    pub async fn get_curr_entity1(
        &mut self,
        control_selector: u16,
        channel: u8,
        entity: u8,
        interface: u8,
    ) -> Result<u8, RequestError> {
        let packet = SetupPacket {
            request_type: RequestType::IN | RequestType::TYPE_CLASS | RequestType::RECIPIENT_INTERFACE,
            request: codes::request_code::CUR,
            value: (channel as u16) << 8 | control_selector as u16,
            index: (entity as u16) << 8 | interface as u16,
            length: 1,
        };

        let mut buf = Aligned::<A4, _>([0; 1]);

        self.control_channel
            .control_in(&packet, buf.as_mut_slice())
            .await
            .map_err(|e| RequestError::RequestFailed(e))?;

        Ok(buf[0])
    }

    pub async fn get_curr_entity2(
        &mut self,
        control_selector: u16,
        channel: u8,
        entity: u8,
        interface: u8,
    ) -> Result<u16, RequestError> {
        let packet = SetupPacket {
            request_type: RequestType::IN | RequestType::TYPE_CLASS | RequestType::RECIPIENT_INTERFACE,
            request: codes::request_code::CUR,
            value: (channel as u16) << 8 | control_selector,
            index: (entity as u16) << 8 | interface as u16,
            length: 2,
        };

        let mut buf = Aligned::<A4, _>([0; 2]);

        self.control_channel
            .control_in(&packet, buf.as_mut_slice())
            .await
            .map_err(|e| RequestError::RequestFailed(e))?;

        Ok(u16::from_le_bytes([buf[0], buf[1]]))
    }

    pub async fn get_curr_entity3(
        &mut self,
        control_selector: u16,
        channel: u8,
        entity: u8,
        interface: u8,
    ) -> Result<u32, RequestError> {
        let packet = SetupPacket {
            request_type: RequestType::IN | RequestType::TYPE_CLASS | RequestType::RECIPIENT_INTERFACE,
            request: codes::request_code::CUR,
            value: (channel as u16) << 8 | control_selector,
            index: (entity as u16) << 8 | interface as u16,
            length: 4,
        };

        let mut buf = Aligned::<A4, _>([0; 4]);

        self.control_channel
            .control_in(&packet, buf.as_mut_slice())
            .await
            .map_err(|e| RequestError::RequestFailed(e))?;

        Ok(u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]))
    }

    pub async fn get_range_entity1(
        &mut self,
        control_selector: u16,
        channel: u8,
        entity: u8,
        interface: u8,
    ) -> Result<Layout1ParameterBlock, RequestError> {
        let packet = SetupPacket {
            request_type: RequestType::IN | RequestType::TYPE_CLASS | RequestType::RECIPIENT_INTERFACE,
            request: codes::request_code::RANGE,
            value: (channel as u16) << 8 | control_selector,
            index: (entity as u16) << 8 | interface as u16,
            length: size_of::<Layout1ParameterBlock>() as u16,
        };

        let mut buf = Aligned::<A4, _>([0; size_of::<Layout1ParameterBlock>()]);

        self.control_channel
            .control_in(&packet, buf.as_mut_slice())
            .await
            .map_err(|e| RequestError::RequestFailed(e))?;

        let layout = Layout1ParameterBlock::try_from_bytes(buf.as_slice()).ok_or(RequestError::InvalidResponse)?;

        Ok(layout)
    }

    pub async fn get_range_entity2(
        &mut self,
        control_selector: u16,
        channel: u8,
        entity: u8,
        interface: u8,
    ) -> Result<Layout2ParameterBlock, RequestError> {
        let packet = SetupPacket {
            request_type: RequestType::IN | RequestType::TYPE_CLASS | RequestType::RECIPIENT_INTERFACE,
            request: codes::request_code::RANGE,
            value: (channel as u16) << 8 | control_selector,
            index: (entity as u16) << 8 | interface as u16,
            length: size_of::<Layout2ParameterBlock>() as u16,
        };

        let mut buf = Aligned::<A4, _>([0; size_of::<Layout2ParameterBlock>()]);

        self.control_channel
            .control_in(&packet, buf.as_mut_slice())
            .await
            .map_err(|e| RequestError::RequestFailed(e))?;

        let layout = Layout2ParameterBlock::try_from_bytes(buf.as_slice()).ok_or(RequestError::InvalidResponse)?;

        Ok(layout)
    }

    pub async fn get_range_entity3(
        &mut self,
        control_selector: u16,
        channel: u8,
        entity: u8,
        interface: u8,
    ) -> Result<Layout3ParameterBlock, RequestError> {
        let packet = SetupPacket {
            request_type: RequestType::IN | RequestType::TYPE_CLASS | RequestType::RECIPIENT_INTERFACE,
            request: codes::request_code::RANGE,
            value: (channel as u16) << 8 | control_selector,
            index: (entity as u16) << 8 | interface as u16,
            length: size_of::<Layout3ParameterBlock>() as u16,
        };

        let mut buf = Aligned::<A4, _>([0; size_of::<Layout3ParameterBlock>()]);

        self.control_channel
            .control_in(&packet, buf.as_mut_slice())
            .await
            .map_err(|e| RequestError::RequestFailed(e))?;

        let layout = Layout3ParameterBlock::try_from_bytes(buf.as_slice()).ok_or(RequestError::InvalidResponse)?;

        Ok(layout)
    }
}

//-------------------------------------------------------
// MARK: Output stream

pub struct UacOut<H: UsbHostDriver> {
    pub output_channel: H::Channel<channel::Isochronous, channel::Out>,
    pub feedback_channel: H::Channel<channel::Isochronous, channel::In>,
    speed: Speed,
    samples_per_microframe: f32,
    microframes_per_microsecond: f32,
    send_start: Instant,
    num_frames: u64,
    last_feedback_time: Instant,
    lock_delay: LockDelay,
    num_channels: u8,
    bytes_per_sample: usize,
    max_bytes_per_packet: usize,
}

enum LockDelay {
    Milliseconds(u16),
    Samples(u16),
}

impl<H: UsbHostDriver> UacOut<H> {
    // Maybe TODO: should this handle disconnects in some way?
    pub async fn output_stream(&mut self, mut callback: impl FnMut(&mut [u8])) -> Result<(), RequestError> {
        // First, update the sampling frequency
        self.update_sampling_freq().await?;

        let mut output_buffer = Aligned::<A4, _>([0; 1024]);
        let lock_delay = self.lock_delay_samples();

        // First, send the lock request
        let data = &output_buffer.as_mut_slice()
            [..(lock_delay as usize * self.bytes_per_sample).min(self.max_bytes_per_packet)]; // Should already be zeroed out
        self.output_channel
            .request_out(data)
            .await
            .map_err(|e| RequestError::RequestFailed(e))?;

        debug!("[UAC] Lock request sent, starting stream");

        // Readjust the max bytes per packet to be a multiple of the frame size
        let bytes_per_frame = self.bytes_per_sample * self.num_channels as usize;
        let max_samples_per_packet = self.max_bytes_per_packet / bytes_per_frame;
        let max_bytes_per_packet = max_samples_per_packet * bytes_per_frame;
        trace!(
            "[UAC] Max bytes per packet: {}; max samples per packet: {}",
            max_bytes_per_packet,
            max_samples_per_packet
        );
        let mut sample_accumulator = 0.0;
        loop {
            // Does the sampling frequency need to be updated?
            if self.last_feedback_time.elapsed() > Duration::from_millis(1) {
                self.update_sampling_freq().await?;
            }
            let mut microframes_elapsed = self.microframes_elapsed_since_last_frame();

            // Do we need to wait until the next frame?
            if microframes_elapsed < 1.0 {
                Timer::after(Duration::from_micros(
                    ((1.0 - microframes_elapsed) / self.microframes_per_microsecond) as u64,
                ))
                .await;
                microframes_elapsed = self.microframes_elapsed_since_last_frame();
            }
            let num_microframes_elapsed = microframes_elapsed as u64;

            // Figure out how many samples to send
            sample_accumulator += self.samples_per_microframe * num_microframes_elapsed as f32;
            let samples_to_send = sample_accumulator as usize;
            sample_accumulator -= samples_to_send as f32;
            let mut bytes_to_send = samples_to_send * bytes_per_frame;
            // trace!("[UAC] Bytes to send: {}", bytes_to_send);
            // Chunk the data if it's too large
            while bytes_to_send > 0 {
                let num_bytes = max_bytes_per_packet.min(bytes_to_send);
                // Fill the buffer with data
                let data = &mut output_buffer.as_mut_slice()[..num_bytes];
                data.fill(0);

                callback(data);
                bytes_to_send -= num_bytes;

                // Send the data
                let _len = self
                    .output_channel
                    .request_out(data)
                    .await
                    .map_err(|e| RequestError::RequestFailed(e))?;
            }

            self.num_frames += num_microframes_elapsed;
        }
    }

    pub async fn update_sampling_freq(&mut self) -> Result<(), RequestError> {
        let mut feedback_buffer = Aligned::<A4, _>([0; 4]);
        let len = self
            .feedback_channel
            .request_in(feedback_buffer.as_mut_slice())
            .await
            .map_err(|e| RequestError::RequestFailed(e))?;
        if let Some(samples_per_microframe) = parse_feedback(self.speed, &feedback_buffer.as_slice()[..len]) {
            self.samples_per_microframe = samples_per_microframe;
            trace!("[UAC] Samples per microframe: {}", self.samples_per_microframe);
        }
        self.last_feedback_time = Instant::now();
        Ok(())
    }

    fn microframes_elapsed_since_last_frame(&self) -> f32 {
        if self.send_start == Instant::from_ticks(0) {
            // We're at the start of the stream, so we want to send 1 microframe
            1.0
        } else {
            (self.send_start.elapsed().as_micros() - (self.num_frames * self.microseconds_per_microframe())) as f32
                * self.microframes_per_microsecond
        }
    }

    fn microseconds_per_microframe(&self) -> u64 {
        if self.speed == Speed::High {
            125
        } else {
            1000
        }
    }

    fn lock_delay_samples(&self) -> u16 {
        match self.lock_delay {
            LockDelay::Milliseconds(ms) => (ms as f32 * self.samples_per_frame()) as u16,
            LockDelay::Samples(samples) => samples,
        }
    }

    fn microframes_per_frame(&self) -> u16 {
        if self.speed == Speed::High {
            8
        } else {
            1
        }
    }

    fn samples_per_frame(&self) -> f32 {
        self.samples_per_microframe * self.microframes_per_frame() as f32
    }
}

/// Parse USB feedback endpoint response into a floating point number.
/// Returns the number of samples per USB frame (full-speed) or microframe (high-speed).
pub fn parse_feedback(speed: Speed, data: &[u8]) -> Option<f32> {
    match speed {
        Speed::Low => None, // Low-speed doesn't support isochronous transfers.

        Speed::High => {
            if data.len() < 4 {
                return None;
            }

            let fractional_part = u16::from_le_bytes([data[0], data[1]]);
            let integer_part = u16::from_le_bytes([data[2], data[3]]);
            // Convert fractional part to float (divide by 2^16 since it's a 16-bit fraction)
            Some(integer_part as f32 + (fractional_part as f32 / 65536.0))
        }

        Speed::Full => {
            if data.len() < 3 {
                return None;
            }

            // USB 2.0 spec says 10.14 fixed point, left-justified in 24 bits
            // So we take the 3 bytes and shift left by 2 bits to convert to 10.16-style format
            let raw = ((data[2] as u32) << 16) | ((data[1] as u32) << 8) | data[0] as u32;
            let shifted = raw << 2;

            let fractional_part = (shifted & 0xFFFF) as u16;
            let integer_part = (shifted >> 16) as u16;
            // Convert fractional part to float (divide by 2^16 since it's a 16-bit fraction)
            Some(integer_part as f32 + (fractional_part as f32 / 65536.0))
        }
    }
}

//-------------------------------------------------------
// MARK: Response types
// For the control interface

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
