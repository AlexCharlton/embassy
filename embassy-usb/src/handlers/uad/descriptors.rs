use super::codes::*;
use crate::host::descriptor::{ConfigurationDescriptor, StringIndex, USBDescriptor};
use heapless::Vec;

const MAX_AUDIO_STREAMING_INTERFACES: usize = 16;
const MAX_ALTERNATE_SETTINGS: usize = 4;

#[derive(Debug, PartialEq)]
struct AudioInterfaceCollection {
    interface_association_descriptor: InterfaceAssociationDescriptor,
    control_interface: AudioControlInterface,
    audio_streaming_interfaces: Vec<AudioStreamingInterface, MAX_AUDIO_STREAMING_INTERFACES>,
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
        let iad = loop {
            let desc = descriptors.next().ok_or(AudioInterfaceError::NoAudioConfiguration)?;
            if let Ok(iad) = InterfaceAssociationDescriptor::try_from_bytes(desc) {
                if iad.is_audio_association() {
                    break iad;
                }
            }
        };

        // Find and parse Audio Control Interface
        let mut control_interface = None;
        let mut streaming_interfaces = Vec::new();
        let mut current_interface_descriptors = Vec::new();

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
                if interface.interface_number >= iad.first_interface + iad.num_interfaces {
                    trace!(
                        "Interface number {} outside of IAD range, stopping",
                        interface.interface_number
                    );
                    break;
                }
                let next_desc = descriptors.peek();
                if let Some(next_desc) = next_desc {
                    if let Ok(next_interface) = InterfaceDescriptor::try_from_bytes(next_desc) {
                        if next_interface.interface_number == interface.interface_number {
                            trace!(
                                "Found next interface with same number: {:#04x}",
                                next_interface.interface_number
                            );
                            current_interface_descriptors
                                .push(interface)
                                .map_err(|_| AudioInterfaceError::BufferFull)?;
                            // The next descriptor is the same interface, so we collect all descriptors for this interface
                            continue;
                        }
                    }
                }
                if interface.interface_class == interface::AUDIO {
                    match interface.interface_subclass {
                        interface::subclass::AUDIOCONTROL => {
                            trace!("Processing Audio Control Interface");
                            if interface.interface_protocol != function_protocol::AF_VERSION_02_00 {
                                debug!(
                                    "Skipping interface with unsupported protocol: {:#04x}",
                                    interface.interface_protocol
                                );
                                continue;
                            }
                            if control_interface.is_some() {
                                warn!("Audio Control Interface already parsed, skipping");
                                current_interface_descriptors.clear();
                                continue;
                            }
                            let header = descriptors
                                .next()
                                .ok_or(AudioInterfaceError::MissingControlInterfaceHeader)?;
                            if let Ok(header) = AudioControlHeaderDescriptor::try_from_bytes(header) {
                                debug!(
                                    "Found Audio Control Header: version={}.{}",
                                    header.audio_device_class.0, header.audio_device_class.1
                                );
                                current_interface_descriptors
                                    .push(interface)
                                    .map_err(|_| AudioInterfaceError::BufferFull)?;
                                control_interface = Some(AudioControlInterface {
                                    interface_descriptors: current_interface_descriptors.clone(),
                                    header_descriptor: header,
                                });
                            } else {
                                return Err(AudioInterfaceError::MissingControlInterfaceHeader);
                            }
                            current_interface_descriptors.clear();
                        }
                        interface::subclass::AUDIOSTREAMING => {
                            if interface.interface_protocol != function_protocol::AF_VERSION_02_00 {
                                debug!(
                                    "Skipping interface with unsupported protocol: {:#04x}",
                                    interface.interface_protocol
                                );
                                continue;
                            }
                            trace!("Processing Audio Streaming Interface");
                            let class_descriptor = descriptors
                                .next()
                                .ok_or(AudioInterfaceError::MissingAudioStreamingClassDescriptor)?;
                            if let Ok(class_descriptor) =
                                AudioStreamingClassDescriptor::try_from_bytes(class_descriptor)
                            {
                                trace!(
                                    "Found Audio Streaming Class Descriptor: {:#04x}",
                                    class_descriptor.format_type
                                );
                                current_interface_descriptors
                                    .push(interface)
                                    .map_err(|_| AudioInterfaceError::BufferFull)?;
                                streaming_interfaces
                                    .push(AudioStreamingInterface {
                                        interface_descriptors: current_interface_descriptors.clone(),
                                        class_descriptor: class_descriptor,
                                    })
                                    .map_err(|_| AudioInterfaceError::BufferFull)?;
                            } else {
                                return Err(AudioInterfaceError::MissingAudioStreamingClassDescriptor);
                            }
                            current_interface_descriptors.clear();
                        }
                        _ => {
                            trace!("Skipping unknown audio subclass: {:#04x}", interface.interface_subclass);
                        }
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
struct AudioControlInterface {
    interface_descriptors: Vec<InterfaceDescriptor, MAX_ALTERNATE_SETTINGS>,
    header_descriptor: AudioControlHeaderDescriptor,
}

#[derive(Debug, PartialEq)]
struct AudioControlHeaderDescriptor {
    audio_device_class: (u8, u8), // Major, minor version
    category: u8,
    controls_bitmap: u8,
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
struct AudioStreamingInterface {
    interface_descriptors: Vec<InterfaceDescriptor, MAX_ALTERNATE_SETTINGS>,
    class_descriptor: AudioStreamingClassDescriptor,
}

#[derive(Debug, Clone, PartialEq)]
struct AudioStreamingClassDescriptor {
    terminal_link_id: u8,
    controls_bitmap: u8,
    format_type: u8,
    format_bitmap: u32,
    num_channels: u8,
    channel_config_bitmap: u32,
    channel_name: StringIndex,
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
            9, 4, 3, 0, 0, 1, 1, 0, 0, // Audio Control Interface
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
                },
            ])
            .unwrap(),
        };
        let audio_interface_collection = AudioInterfaceCollection::try_from_configuration(&descriptor).unwrap();
        assert_eq!(audio_interface_collection, expected);
    }
}
