#[allow(missing_docs)]
pub mod codes;
#[allow(missing_docs)]
pub mod descriptors;

use crate::control::Request;

use super::{EnumerationInfo, RegisterError};
use aligned::{Aligned, A4};
use embassy_usb_driver::host::{channel, ChannelError, HostError, RequestType, SetupPacket, UsbChannel, UsbHostDriver};
use embassy_usb_driver::{Direction, EndpointInfo, EndpointType};
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
}

#[derive(Debug)]
pub enum RequestError {
    RequestFailed(ChannelError),
    InvalidResponse,
}

impl<H: UsbHostDriver> UacHandler<H> {
    pub async fn try_register(host: &H, enum_info: EnumerationInfo) -> Result<Self, RegisterError> {
        // Steps taken:
        // 1. Find the first streaming interface with an output endpoint
        // 2. Connect it to its terminal to find the sampling frequency
        // 3. Check its format type
        // 4. Select the right alternate setting for the interface with a SET_INTERFACE request
        // 5. Allocate up the output channel and the corresponding feedback channel, store on Self
        // 6. TODO: Send at least <lock durration> of zeroes to ensure the device locks onto the stream

        let interface_collection =
            match descriptors::AudioInterfaceCollection::try_from_configuration(&enum_info.cfg_desc) {
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
        let mut control_channel = host.alloc_channel::<channel::Control, channel::InOut>(
            enum_info.device_address,
            &EndpointInfo::new(
                0.into(),
                EndpointType::Control,
                (enum_info.device_desc.max_packet_size0 as u16).min(enum_info.speed.max_packet_size()),
            ),
            enum_info.ls_over_fs,
        )?;

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
        })
    }

    pub fn input_terminal(&self) -> &descriptors::TerminalDescriptor {
        &self.interface_collection.control_interface.terminal_descriptors[&self.input_terminal_id]
    }

    pub fn output_interface(&self) -> &descriptors::AudioStreamingInterface {
        &self.interface_collection.audio_streaming_interfaces[self.output_interface_idx]
    }

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

    pub async fn get_sampling_freq(&mut self, terminal_id: u8) -> Result<u32, RequestError> {
        self.get_curr_entity3(
            codes::control_selector::clock_source::SAMPLING_FREQ_CONTROL,
            0,
            terminal_id,
            0,
        )
        .await
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
