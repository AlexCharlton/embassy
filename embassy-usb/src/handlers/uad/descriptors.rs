use super::codes::*;
use crate::host::descriptor::{ConfigurationDescriptor, EndpointDescriptor, StringIndex, USBDescriptor};
use core::iter::Peekable;
use heapless::Vec;

const MAX_AUDIO_STREAMING_INTERFACES: usize = 16;
const MAX_ALTERNATE_SETTINGS: usize = 4;

#[derive(Debug, PartialEq)]
pub struct AudioInterfaceCollection {
    pub interface_association_descriptor: InterfaceAssociationDescriptor,
    pub control_interface: AudioControlInterface,
    pub audio_streaming_interfaces: Vec<AudioStreamingInterface, MAX_AUDIO_STREAMING_INTERFACES>,
}

#[derive(Debug, PartialEq)]
pub enum AudioInterfaceError {
    BufferFull,
    MissingControlInterface,
    MissingControlInterfaceHeader,
    InvalidDescriptor,
    NoAudioConfiguration,
    MissingAudioStreamingClassDescriptor,
}

impl AudioInterfaceCollection {
    pub fn try_from_configuration(cfg: &ConfigurationDescriptor) -> Result<Self, AudioInterfaceError> {
        let mut descriptors = cfg.iter_descriptors().peekable();

        // Find Interface Association Descriptor for Audio Function
        let iad = Self::find_interface_association_descriptor(&mut descriptors)?;

        // Find and parse Audio Control Interface
        let mut control_interface = None;
        let mut streaming_interfaces = Vec::new();

        while let Some(interfaces) =
            Self::next_interface_descriptors(&mut descriptors, iad.first_interface, iad.num_interfaces)?
        {
            trace!("Found interfaces: {:?}", interfaces);
            let first_interface = interfaces.first().unwrap();
            if first_interface.interface_class == interface::AUDIO {
                match first_interface.interface_subclass {
                    interface::subclass::AUDIOCONTROL => {
                        if control_interface.is_some() {
                            warn!("Audio Control Interface already parsed, skipping");
                            continue;
                        }
                        if first_interface.interface_protocol != function_protocol::AF_VERSION_02_00 {
                            debug!(
                                "Skipping interface with unsupported protocol: {:#04x}",
                                first_interface.interface_protocol
                            );
                            continue;
                        }
                        control_interface = Some(Self::collect_audio_control_interface(&mut descriptors, interfaces)?);
                    }
                    interface::subclass::AUDIOSTREAMING => {
                        if first_interface.interface_protocol != function_protocol::AF_VERSION_02_00 {
                            debug!(
                                "Skipping interface with unsupported protocol: {:#04x}",
                                first_interface.interface_protocol
                            );
                            continue;
                        }
                        streaming_interfaces
                            .push(Self::collect_audio_streaming_interface(&mut descriptors, interfaces)?)
                            .map_err(|_| AudioInterfaceError::BufferFull)?;
                    }
                    _ => {
                        trace!(
                            "Skipping unknown audio subclass: {:#04x}",
                            first_interface.interface_subclass
                        );
                    }
                }
            }
        }

        // Create the audio interface collection
        Ok(Self {
            interface_association_descriptor: iad,
            control_interface: control_interface.ok_or(AudioInterfaceError::MissingControlInterface)?,
            audio_streaming_interfaces: streaming_interfaces,
        })
    }

    fn find_interface_association_descriptor<'a>(
        descriptors: &mut Peekable<impl Iterator<Item = &'a [u8]>>,
    ) -> Result<InterfaceAssociationDescriptor, AudioInterfaceError> {
        loop {
            let desc = descriptors.next().ok_or(AudioInterfaceError::NoAudioConfiguration)?;
            if let Ok(iad) = InterfaceAssociationDescriptor::try_from_bytes(desc) {
                if iad.is_audio_association() {
                    return Ok(iad);
                }
            }
        }
    }

    fn next_interface_descriptors<'a>(
        descriptors: &mut Peekable<impl Iterator<Item = &'a [u8]>>,
        first_interface: u8,
        num_interfaces: u8,
    ) -> Result<Option<Vec<InterfaceDescriptor, MAX_ALTERNATE_SETTINGS>>, AudioInterfaceError> {
        let mut interface_descriptors = Vec::new();

        while let Some(desc) = descriptors.next() {
            if let Ok(interface) = InterfaceDescriptor::try_from_bytes(desc) {
                trace!(
                    "Found interface: number={}, class={:#04x}, subclass={:#04x}, protocol={:#04x} from {:?}",
                    interface.interface_number,
                    interface.interface_class,
                    interface.interface_subclass,
                    interface.interface_protocol,
                    desc,
                );

                // Check if we're still within the audio function's interfaces
                if interface.interface_number >= first_interface + num_interfaces {
                    trace!(
                        "Interface number {} outside of IAD range, stopping",
                        interface.interface_number
                    );
                    break;
                }
                interface_descriptors
                    .push(interface)
                    .map_err(|_| AudioInterfaceError::BufferFull)?;

                // Do we have contiguous (alternate) interfaces?
                while let Some(next_desc) = descriptors.peek() {
                    if let Ok(next_interface) = InterfaceDescriptor::try_from_bytes(next_desc) {
                        if next_interface.interface_number == interface.interface_number {
                            // The next descriptor is the same interface, so we collect all descriptors for this interface
                            trace!(
                                "Found next interface with same number: {:#04x}",
                                next_interface.interface_number
                            );
                            descriptors.next(); // consume the next descriptor
                            interface_descriptors
                                .push(next_interface)
                                .map_err(|_| AudioInterfaceError::BufferFull)?;
                            continue;
                        }
                    }
                    break;
                }
                break;
            }
        }

        if interface_descriptors.is_empty() {
            Ok(None)
        } else {
            Ok(Some(interface_descriptors))
        }
    }

    fn collect_audio_control_interface<'a>(
        descriptors: &mut Peekable<impl Iterator<Item = &'a [u8]>>,
        interfaces: Vec<InterfaceDescriptor, MAX_ALTERNATE_SETTINGS>,
    ) -> Result<AudioControlInterface, AudioInterfaceError> {
        trace!("Processing Audio Control Interface");
        let header = descriptors
            .next()
            .ok_or(AudioInterfaceError::MissingControlInterfaceHeader)?;
        if let Ok(header) = AudioControlHeaderDescriptor::try_from_bytes(header) {
            debug!(
                "Found Audio Control Header: version={}.{}",
                header.audio_device_class.0, header.audio_device_class.1
            );
            Ok(AudioControlInterface {
                interface_descriptors: interfaces,
                header_descriptor: header,
            })
        } else {
            Err(AudioInterfaceError::MissingControlInterfaceHeader)
        }
    }

    fn collect_audio_streaming_interface<'a>(
        descriptors: &mut Peekable<impl Iterator<Item = &'a [u8]>>,
        interfaces: Vec<InterfaceDescriptor, MAX_ALTERNATE_SETTINGS>,
    ) -> Result<AudioStreamingInterface, AudioInterfaceError> {
        trace!("Processing Audio Streaming Interface");
        let class_descriptor = descriptors
            .next()
            .ok_or(AudioInterfaceError::MissingAudioStreamingClassDescriptor)?;
        if let Ok(class_descriptor) = AudioStreamingClassDescriptor::try_from_bytes(class_descriptor) {
            trace!(
                "Found Audio Streaming Class Descriptor: {:#04x}",
                class_descriptor.format_type
            );
            let mut streaming_interface = AudioStreamingInterface {
                interface_descriptors: interfaces,
                class_descriptor: class_descriptor,
                endpoint_descriptor: None,
                feedback_endpoint_descriptor: None,
                audio_endpoint_descriptor: None,
                format_type_descriptor: None,
            };
            loop {
                if let Some(desc) = descriptors.peek() {
                    if InterfaceDescriptor::try_from_bytes(desc).is_ok() {
                        break;
                    }
                    let desc = descriptors.next().unwrap();
                    if let Ok(endpoint) = EndpointDescriptor::try_from_bytes(desc) {
                        if endpoint.attributes == 0b010001 {
                            streaming_interface.feedback_endpoint_descriptor = Some(endpoint);
                        } else {
                            streaming_interface.endpoint_descriptor = Some(endpoint);
                        }
                    }
                    if let Ok(audio_endpoint) = AudioEndpointDescriptor::try_from_bytes(desc) {
                        streaming_interface.audio_endpoint_descriptor = Some(audio_endpoint);
                    }
                    if let Ok(format_type) = FormatTypeDescriptor::try_from_bytes(desc) {
                        streaming_interface.format_type_descriptor = Some(format_type);
                    }
                } else {
                    break;
                }
            }
            Ok(streaming_interface)
        } else {
            Err(AudioInterfaceError::MissingAudioStreamingClassDescriptor)
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct InterfaceAssociationDescriptor {
    pub first_interface: u8,
    pub num_interfaces: u8,
    pub class: u8,
    pub subclass: u8,
    pub protocol: u8,
    pub name: StringIndex,
}

impl USBDescriptor for InterfaceAssociationDescriptor {
    const SIZE: usize = 8;
    const DESC_TYPE: u8 = 0x0B;
    type Error = ();

    fn try_from_bytes(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes.len() < Self::SIZE {
            return Err(());
        }
        if bytes[1] != Self::DESC_TYPE {
            return Err(());
        }
        Ok(Self {
            first_interface: bytes[2],
            num_interfaces: bytes[3],
            class: bytes[4],
            subclass: bytes[5],
            protocol: bytes[6],
            name: bytes[7],
        })
    }
}

impl InterfaceAssociationDescriptor {
    pub fn is_audio_association(&self) -> bool {
        self.class == AUDIO_FUNCTION && self.protocol == function_protocol::AF_VERSION_02_00
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[allow(missing_docs)]
pub struct InterfaceDescriptor {
    pub len: u8,
    pub descriptor_type: u8,
    pub interface_number: u8,
    pub alternate_setting: u8,
    pub num_endpoints: u8,
    pub interface_class: u8,
    pub interface_subclass: u8,
    pub interface_protocol: u8,
    pub interface_name: StringIndex,
}

impl USBDescriptor for InterfaceDescriptor {
    const SIZE: usize = 9;
    const DESC_TYPE: u8 = 0x04;
    type Error = ();

    fn try_from_bytes(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes.len() < Self::SIZE {
            return Err(());
        }
        if bytes[1] != Self::DESC_TYPE {
            return Err(());
        }
        Ok(Self {
            len: bytes[0],
            descriptor_type: bytes[1],
            interface_number: bytes[2],
            alternate_setting: bytes[3],
            num_endpoints: bytes[4],
            interface_class: bytes[5],
            interface_subclass: bytes[6],
            interface_protocol: bytes[7],
            interface_name: bytes[8],
        })
    }
}

#[derive(Debug, PartialEq)]
pub struct AudioControlInterface {
    pub interface_descriptors: Vec<InterfaceDescriptor, MAX_ALTERNATE_SETTINGS>,
    pub header_descriptor: AudioControlHeaderDescriptor,
    // TODO: Endpoint descriptors
    // TODO: Clock, unit, terminal descriptors
}

#[derive(Debug, PartialEq)]
pub struct AudioControlHeaderDescriptor {
    pub audio_device_class: (u8, u8), // Major, minor version
    pub category: u8,
    pub controls_bitmap: u8,
}

impl USBDescriptor for AudioControlHeaderDescriptor {
    const SIZE: usize = 9;
    const DESC_TYPE: u8 = descriptor_type::CS_INTERFACE;
    type Error = ();

    fn try_from_bytes(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes.len() < Self::SIZE {
            return Err(());
        }
        if bytes[1] != Self::DESC_TYPE {
            return Err(());
        }
        if bytes[2] != ac_descriptor::HEADER {
            return Err(());
        }
        Ok(Self {
            audio_device_class: (bytes[4], bytes[3]),
            category: bytes[5],
            controls_bitmap: bytes[8],
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioStreamingInterface {
    pub interface_descriptors: Vec<InterfaceDescriptor, MAX_ALTERNATE_SETTINGS>,
    pub class_descriptor: AudioStreamingClassDescriptor,
    pub endpoint_descriptor: Option<EndpointDescriptor>,
    pub feedback_endpoint_descriptor: Option<EndpointDescriptor>,
    pub audio_endpoint_descriptor: Option<AudioEndpointDescriptor>,
    pub format_type_descriptor: Option<FormatTypeDescriptor>,
    // TODO: Encoder, decoder descriptors
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioStreamingClassDescriptor {
    pub terminal_link_id: u8,
    pub controls_bitmap: u8,
    pub format_type: u8,
    pub format_bitmap: u32,
    pub num_channels: u8,
    pub channel_config_bitmap: u32,
    pub channel_name: StringIndex,
}

impl USBDescriptor for AudioStreamingClassDescriptor {
    const SIZE: usize = 16;
    const DESC_TYPE: u8 = descriptor_type::CS_INTERFACE;
    type Error = ();

    fn try_from_bytes(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes.len() < Self::SIZE {
            return Err(());
        }
        if bytes[1] != Self::DESC_TYPE {
            return Err(());
        }
        if bytes[2] != as_descriptor::GENERAL {
            return Err(());
        }
        Ok(Self {
            terminal_link_id: bytes[3],
            controls_bitmap: bytes[4],
            format_type: bytes[5],
            format_bitmap: u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]),
            num_channels: bytes[10],
            channel_config_bitmap: u32::from_le_bytes([bytes[11], bytes[12], bytes[13], bytes[14]]),
            channel_name: bytes[15],
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioEndpointDescriptor {
    pub attributes_bitmap: u8,
    pub controls_bitmap: u8,
    pub lock_delay_units: u8,
    pub lock_delay: u16,
}

impl USBDescriptor for AudioEndpointDescriptor {
    const SIZE: usize = 6;
    const DESC_TYPE: u8 = descriptor_type::CS_ENDPOINT;
    type Error = ();

    fn try_from_bytes(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes.len() < Self::SIZE {
            return Err(());
        }
        if bytes[1] != Self::DESC_TYPE {
            return Err(());
        }
        if bytes[2] != as_descriptor::GENERAL {
            return Err(());
        }
        Ok(Self {
            attributes_bitmap: bytes[3],
            controls_bitmap: bytes[4],
            lock_delay_units: bytes[5],
            lock_delay: u16::from_le_bytes([bytes[6], bytes[7]]),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum FormatTypeDescriptor {
    I(FormatTypeI),
    II(FormatTypeII),
    III(FormatTypeIII),
    IV,
    ExtendedI(FormatTypeExtendedI),
    ExtendedII(FormatTypeExtendedII),
    ExtendedIII(FormatTypeExtendedIII),
}

impl FormatTypeDescriptor {
    fn try_from_bytes(bytes: &[u8]) -> Result<Self, ()> {
        if bytes.len() < 4 {
            // minimum length of a format type descriptor
            return Err(());
        }
        let len = bytes[0] as usize;
        if bytes[1] != descriptor_type::CS_INTERFACE {
            return Err(());
        }
        if bytes[2] != as_descriptor::FORMAT_TYPE {
            return Err(());
        }
        match bytes[3] {
            format_type::I => {
                if len != 6 {
                    return Err(());
                }
                Ok(Self::I(FormatTypeI {
                    subslot_size: bytes[4],
                    bit_resolution: bytes[5],
                }))
            }
            format_type::II => {
                if len != 8 {
                    return Err(());
                }
                Ok(Self::II(FormatTypeII {
                    max_bit_rate: u16::from_le_bytes([bytes[4], bytes[5]]),
                    slots_per_frame: u16::from_le_bytes([bytes[6], bytes[7]]),
                }))
            }
            format_type::III => {
                if len != 6 {
                    return Err(());
                }
                Ok(Self::III(FormatTypeIII {
                    subslot_size: bytes[4],
                    bit_resolution: bytes[5],
                }))
            }
            format_type::IV => Ok(Self::IV),
            format_type::EXT_I => {
                if len != 9 {
                    return Err(());
                }
                Ok(Self::ExtendedI(FormatTypeExtendedI {
                    subslot_size: bytes[4],
                    bit_resolution: bytes[5],
                    header_length: bytes[6],
                    control_size: bytes[7],
                    sideband_protocol: bytes[8],
                }))
            }
            format_type::EXT_II => {
                if len != 10 {
                    return Err(());
                }
                Ok(Self::ExtendedII(FormatTypeExtendedII {
                    max_bit_rate: u16::from_le_bytes([bytes[4], bytes[5]]),
                    samples_per_frame: u16::from_le_bytes([bytes[6], bytes[7]]),
                    header_length: bytes[8],
                    sideband_protocol: bytes[9],
                }))
            }
            format_type::EXT_III => {
                if len != 8 {
                    return Err(());
                }
                Ok(Self::ExtendedIII(FormatTypeExtendedIII {
                    subslot_size: bytes[4],
                    bit_resolution: bytes[5],
                    header_length: bytes[6],
                    sideband_protocol: bytes[7],
                }))
            }
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FormatTypeI {
    pub subslot_size: u8,
    pub bit_resolution: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FormatTypeII {
    pub max_bit_rate: u16,
    pub slots_per_frame: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FormatTypeIII {
    pub subslot_size: u8,
    pub bit_resolution: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FormatTypeExtendedI {
    pub subslot_size: u8,
    pub bit_resolution: u8,
    pub header_length: u8,
    pub control_size: u8,
    pub sideband_protocol: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FormatTypeExtendedII {
    pub max_bit_rate: u16,
    pub samples_per_frame: u16,
    pub header_length: u8,
    pub sideband_protocol: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FormatTypeExtendedIII {
    pub subslot_size: u8,
    pub bit_resolution: u8,
    pub header_length: u8,
    pub sideband_protocol: u8,
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::host::descriptor::ConfigurationDescriptor;
    use env_logger;
    use heapless::Vec;

    #[test]
    fn test_parse() {
        // Initialize logger
        env_logger::init();

        let mut buffer: [u8; 512] = [0; 512];
        let descriptors = [
            8, 11, 0, 4, 1, 0, 32, 0, // Interface Association Descriptor
            9, 4, 0, 0, 0, 1, 1, 32, 7, // Audio Control Interface
            9, 36, 1, 0, 2, 8, 223, 0, 0, // Audio Control Header Descriptor
            8, 36, 10, 40, 1, 7, 0, 16, // Clock Source Descriptor
            17, 36, 2, 2, 1, 1, 0, 40, 16, 0, 0, 0, 0, 18, 0, 0, 2, // Input Terminal Descriptor
            // Feature Unit Descriptor
            74, 36, 6, 10, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 15, 12, 36, 3, 20, 1, 3, 0, 10, 40, 0, 0, 5, 17, 36, 2, 1, 1, 2, 0, 40, 16, 0, 0, 0, 0, 50, 0, 0,
            3, // Input Terminal Descriptor
            // Feature Unit Descriptor
            74, 36, 6, 11, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 14, 12, 36, 3, 22, 1, 1, 0, 11, 40, 0, 0, 4, // Output Terminal Descriptor
            9, 4, 1, 0, 0, 1, 2, 32, 8, // Audio Streaming Interface Descriptor (Alt Setting 0)
            9, 4, 1, 1, 2, 1, 2, 32, 9, // Audio Streaming Interface Descriptor (Alt Setting 1)
            16, 36, 1, 2, 0, 1, 1, 0, 0, 0, 16, 0, 0, 0, 0, 18, // AS Interface Descriptor (General)
            6, 36, 2, 1, 4, 24, // Format Type Descriptor
            7, 5, 1, 5, 0, 2, 1, // Endpoint Descriptor
            8, 37, 1, 0, 0, 2, 8, 0, // AS Endpoint Descriptor
            7, 5, 129, 17, 4, 0, 4, // Endpoint Descriptor (Feedback)
            9, 4, 2, 0, 0, 1, 2, 32, 10, // Audio Streaming Interface Descriptor (Alt Setting 0)
            9, 4, 2, 1, 1, 1, 2, 32, 11, // Audio Streaming Interface Descriptor (Alt Setting 1)
            16, 36, 1, 22, 0, 1, 1, 0, 0, 0, 16, 0, 0, 0, 0, 50, // AS Interface Descriptor (General)
            6, 36, 2, 1, 4, 24, // Format Type Descriptor
            7, 5, 130, 5, 0, 2, 1, // Endpoint Descriptor
            8, 37, 1, 0, 0, 2, 8, 0, // AS Endpoint Descriptor
            9, 4, 3, 0, 0, 1, 1, 0, 0, // Audio Control Interface (UAD 1)
            9, 36, 1, 0, 1, 9, 0, 1, 1, // Audio Control Header Descriptor
            9, 4, 4, 0, 2, 1, 3, 0, 0, // MIDI Streaming Interface Descriptor
            7, 36, 1, 0, 1, 61, 0, // MS Interface Header Descriptor
            6, 36, 2, 1, 51, 0, // MIDI IN Jack Descriptor (Embedded)
            6, 36, 2, 2, 52, 82, // MIDI IN Jack Descriptor (External)
            9, 36, 3, 1, 55, 1, 52, 1, 0, // MIDI OUT Jack Descriptor (Embedded)
            9, 36, 3, 2, 56, 1, 51, 1, 83, // MIDI OUT Jack Descriptor (External)
            7, 5, 131, 2, 0, 2, 0, // Endpoint Descriptor (OUT)
            5, 37, 1, 1, 55, // MS Endpoint Descriptor
            7, 5, 2, 2, 0, 2, 0, // Endpoint Descriptor (IN)
            5, 37, 1, 1, 51, // MS Endpoint Descriptor
            9, 4, 5, 0, 0, 254, 1, 1, 0, // DFU Interface Descriptor
            7, 33, 7, 250, 0, 64, 0, // DFU Functional Descriptor
        ];
        buffer[..descriptors.len()].copy_from_slice(&descriptors);
        let descriptor = ConfigurationDescriptor {
            len: 0,
            descriptor_type: 0,
            total_len: 0,
            num_interfaces: 0,
            configuration_value: 1,
            configuration_name: 0,
            attributes: 0,
            max_power: 0,
            buffer,
        };
        let expected = AudioInterfaceCollection {
            interface_association_descriptor: InterfaceAssociationDescriptor {
                first_interface: 0,
                num_interfaces: 4,
                class: 1,
                subclass: 0,
                protocol: 32,
                name: 0,
            },
            control_interface: AudioControlInterface {
                interface_descriptors: Vec::from_slice(&[InterfaceDescriptor {
                    len: 9,
                    descriptor_type: 4,
                    interface_number: 0,
                    alternate_setting: 0,
                    num_endpoints: 0,
                    interface_class: 1,
                    interface_subclass: 1,
                    interface_protocol: 32,
                    interface_name: 7,
                }])
                .unwrap(),
                header_descriptor: AudioControlHeaderDescriptor {
                    audio_device_class: (2, 0),
                    category: 8,
                    controls_bitmap: 0,
                },
            },
            audio_streaming_interfaces: Vec::from_slice(&[
                AudioStreamingInterface {
                    interface_descriptors: Vec::from_slice(&[
                        InterfaceDescriptor {
                            len: 9,
                            descriptor_type: 4,
                            interface_number: 1,
                            alternate_setting: 0,
                            num_endpoints: 0,
                            interface_class: 1,
                            interface_subclass: 2,
                            interface_protocol: 32,
                            interface_name: 8,
                        },
                        InterfaceDescriptor {
                            len: 9,
                            descriptor_type: 4,
                            interface_number: 1,
                            alternate_setting: 1,
                            num_endpoints: 2,
                            interface_class: 1,
                            interface_subclass: 2,
                            interface_protocol: 32,
                            interface_name: 9,
                        },
                    ])
                    .unwrap(),
                    class_descriptor: AudioStreamingClassDescriptor {
                        terminal_link_id: 2,
                        controls_bitmap: 0,
                        format_type: 1,
                        format_bitmap: 1,
                        num_channels: 16,
                        channel_config_bitmap: 0,
                        channel_name: 18,
                    },
                    endpoint_descriptor: Some(EndpointDescriptor {
                        len: 7,
                        descriptor_type: 5,
                        endpoint_address: 1,
                        attributes: 5,
                        max_packet_size: 512,
                        interval: 1,
                    }),
                    feedback_endpoint_descriptor: Some(EndpointDescriptor {
                        len: 7,
                        descriptor_type: 5,
                        endpoint_address: 129,
                        attributes: 17,
                        max_packet_size: 4,
                        interval: 4,
                    }),
                    audio_endpoint_descriptor: Some(AudioEndpointDescriptor {
                        attributes_bitmap: 0,
                        controls_bitmap: 0,
                        lock_delay_units: 2,
                        lock_delay: 8,
                    }),
                    format_type_descriptor: Some(FormatTypeDescriptor::I(FormatTypeI {
                        subslot_size: 4,
                        bit_resolution: 24,
                    })),
                },
                AudioStreamingInterface {
                    interface_descriptors: Vec::from_slice(&[
                        InterfaceDescriptor {
                            len: 9,
                            descriptor_type: 4,
                            interface_number: 2,
                            alternate_setting: 0,
                            num_endpoints: 0,
                            interface_class: 1,
                            interface_subclass: 2,
                            interface_protocol: 32,
                            interface_name: 10,
                        },
                        InterfaceDescriptor {
                            len: 9,
                            descriptor_type: 4,
                            interface_number: 2,
                            alternate_setting: 1,
                            num_endpoints: 1,
                            interface_class: 1,
                            interface_subclass: 2,
                            interface_protocol: 32,
                            interface_name: 11,
                        },
                    ])
                    .unwrap(),
                    class_descriptor: AudioStreamingClassDescriptor {
                        terminal_link_id: 22,
                        controls_bitmap: 0,
                        format_type: 1,
                        format_bitmap: 1,
                        num_channels: 16,
                        channel_config_bitmap: 0,
                        channel_name: 50,
                    },
                    endpoint_descriptor: Some(EndpointDescriptor {
                        len: 7,
                        descriptor_type: 5,
                        endpoint_address: 130,
                        attributes: 5,
                        max_packet_size: 512,
                        interval: 1,
                    }),
                    feedback_endpoint_descriptor: None,
                    audio_endpoint_descriptor: Some(AudioEndpointDescriptor {
                        attributes_bitmap: 0,
                        controls_bitmap: 0,
                        lock_delay_units: 2,
                        lock_delay: 8,
                    }),
                    format_type_descriptor: Some(FormatTypeDescriptor::I(FormatTypeI {
                        subslot_size: 4,
                        bit_resolution: 24,
                    })),
                },
            ])
            .unwrap(),
        };
        let audio_interface_collection = AudioInterfaceCollection::try_from_configuration(&descriptor).unwrap();
        // info!("{:#?}", audio_interface_collection);
        assert_eq!(audio_interface_collection, expected);
    }
}
