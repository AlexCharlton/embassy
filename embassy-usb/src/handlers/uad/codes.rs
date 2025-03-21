//! USB Audio Device Class constants from UAC2 specification
//! Based on USB Device Class Definition for Audio Devices, Release 2.0 (May 31, 2006)

/// Audio Function Class Code
pub const AUDIO_FUNCTION: u8 = 0x01;

/// Audio Function Subclass Codes
pub const FUNCTION_SUBCLASS_UNDEFINED: u8 = 0x00;

/// Audio Function Protocol Codes
pub mod function_protocol {
    pub const UNDEFINED: u8 = 0x00;
    pub const AF_VERSION_02_00: u8 = 0x20;
}

/// Audio Interface Class/Subclass Codes
pub mod interface {
    pub const AUDIO: u8 = 0x01;

    pub mod subclass {
        pub const UNDEFINED: u8 = 0x00;
        pub const AUDIOCONTROL: u8 = 0x01;
        pub const AUDIOSTREAMING: u8 = 0x02;
        pub const MIDISTREAMING: u8 = 0x03;
    }

    pub mod protocol {
        pub const UNDEFINED: u8 = 0x00;
        pub const IP_VERSION_02_00: u8 = 0x20;
    }
}

/// Audio Function Category Codes
pub mod function_category {
    pub const UNDEFINED: u8 = 0x00;
    pub const DESKTOP_SPEAKER: u8 = 0x01;
    pub const HOME_THEATER: u8 = 0x02;
    pub const MICROPHONE: u8 = 0x03;
    pub const HEADSET: u8 = 0x04;
    pub const TELEPHONE: u8 = 0x05;
    pub const CONVERTER: u8 = 0x06;
    pub const VOICE_SOUND_RECORDER: u8 = 0x07;
    pub const IO_BOX: u8 = 0x08;
    pub const MUSICAL_INSTRUMENT: u8 = 0x09;
    pub const PRO_AUDIO: u8 = 0x0A;
    pub const AUDIO_VIDEO: u8 = 0x0B;
    pub const CONTROL_PANEL: u8 = 0x0C;
    pub const OTHER: u8 = 0xFF;
}

/// Audio Class-Specific Descriptor Types
pub mod descriptor_type {
    pub const CS_UNDEFINED: u8 = 0x20;
    pub const CS_DEVICE: u8 = 0x21;
    pub const CS_CONFIGURATION: u8 = 0x22;
    pub const CS_STRING: u8 = 0x23;
    pub const CS_INTERFACE: u8 = 0x24;
    pub const CS_ENDPOINT: u8 = 0x25;
}

/// Audio Class-Specific AC Interface Descriptor Subtypes
pub mod ac_descriptor {
    pub const UNDEFINED: u8 = 0x00;
    pub const HEADER: u8 = 0x01;
    pub const INPUT_TERMINAL: u8 = 0x02;
    pub const OUTPUT_TERMINAL: u8 = 0x03;
    pub const MIXER_UNIT: u8 = 0x04;
    pub const SELECTOR_UNIT: u8 = 0x05;
    pub const FEATURE_UNIT: u8 = 0x06;
    pub const EFFECT_UNIT: u8 = 0x07;
    pub const PROCESSING_UNIT: u8 = 0x08;
    pub const EXTENSION_UNIT: u8 = 0x09;
    pub const CLOCK_SOURCE: u8 = 0x0A;
    pub const CLOCK_SELECTOR: u8 = 0x0B;
    pub const CLOCK_MULTIPLIER: u8 = 0x0C;
    pub const SAMPLE_RATE_CONVERTER: u8 = 0x0D;
}

/// Audio Class-Specific AS Interface Descriptor Subtypes
pub mod as_descriptor {
    pub const UNDEFINED: u8 = 0x00;
    pub const GENERAL: u8 = 0x01;
    pub const FORMAT_TYPE: u8 = 0x02;
    pub const ENCODER: u8 = 0x03;
    pub const DECODER: u8 = 0x04;
}

/// Effect Unit Effect Types
pub mod effect_type {
    pub const UNDEFINED: u8 = 0x00;
    pub const PARAM_EQ_SECTION_EFFECT: u8 = 0x01;
    pub const REVERBERATION_EFFECT: u8 = 0x02;
    pub const MOD_DELAY_EFFECT: u8 = 0x03;
    pub const DYN_RANGE_COMP_EFFECT: u8 = 0x04;
}

/// Processing Unit Process Types
pub mod process_type {
    pub const UNDEFINED: u8 = 0x00;
    pub const UP_DOWNMIX_PROCESS: u8 = 0x01;
    pub const DOLBY_PROLOGIC_PROCESS: u8 = 0x02;
    pub const STEREO_EXTENDER_PROCESS: u8 = 0x03;
}

/// Audio Class-Specific Endpoint Descriptor Subtypes
pub mod endpoint_descriptor {
    pub const UNDEFINED: u8 = 0x00;
    pub const EP_GENERAL: u8 = 0x01;
}

/// Audio Class-Specific Request Codes
pub mod request_code {
    pub const UNDEFINED: u8 = 0x00;
    pub const CUR: u8 = 0x01;
    pub const RANGE: u8 = 0x02;
    pub const MEM: u8 = 0x03;
}

/// Encoder Type Codes
pub mod encoder_type {
    pub const UNDEFINED: u8 = 0x00;
    pub const OTHER_ENCODER: u8 = 0x01;
    pub const MPEG_ENCODER: u8 = 0x02;
    pub const AC_3_ENCODER: u8 = 0x03;
    pub const WMA_ENCODER: u8 = 0x04;
    pub const DTS_ENCODER: u8 = 0x05;
}

/// Decoder Type Codes
pub mod decoder_type {
    pub const UNDEFINED: u8 = 0x00;
    pub const OTHER_DECODER: u8 = 0x01;
    pub const MPEG_DECODER: u8 = 0x02;
    pub const AC_3_DECODER: u8 = 0x03;
    pub const WMA_DECODER: u8 = 0x04;
    pub const DTS_DECODER: u8 = 0x05;
}

/// Control Selector Codes
pub mod control_selector {
    /// Clock Source Control Selectors
    pub mod clock_source {
        pub const UNDEFINED: u8 = 0x00;
        pub const SAM_FREQ_CONTROL: u8 = 0x01;
        pub const CLOCK_VALID_CONTROL: u8 = 0x02;
    }

    /// Clock Selector Control Selectors
    pub mod clock_selector {
        pub const UNDEFINED: u8 = 0x00;
        pub const CLOCK_SELECTOR_CONTROL: u8 = 0x01;
    }

    /// Clock Multiplier Control Selectors
    pub mod clock_multiplier {
        pub const UNDEFINED: u8 = 0x00;
        pub const NUMERATOR_CONTROL: u8 = 0x01;
        pub const DENOMINATOR_CONTROL: u8 = 0x02;
    }

    /// Terminal Control Selectors
    pub mod terminal {
        pub const UNDEFINED: u8 = 0x00;
        pub const COPY_PROTECT_CONTROL: u8 = 0x01;
        pub const CONNECTOR_CONTROL: u8 = 0x02;
        pub const OVERLOAD_CONTROL: u8 = 0x03;
        pub const CLUSTER_CONTROL: u8 = 0x04;
        pub const UNDERFLOW_CONTROL: u8 = 0x05;
        pub const OVERFLOW_CONTROL: u8 = 0x06;
        pub const LATENCY_CONTROL: u8 = 0x07;
    }

    /// Mixer Control Selectors
    pub mod mixer {
        pub const UNDEFINED: u8 = 0x00;
        pub const MIXER_CONTROL: u8 = 0x01;
        pub const CLUSTER_CONTROL: u8 = 0x02;
        pub const UNDERFLOW_CONTROL: u8 = 0x03;
        pub const OVERFLOW_CONTROL: u8 = 0x04;
        pub const LATENCY_CONTROL: u8 = 0x05;
    }

    /// Selector Control Selectors
    pub mod selector {
        pub const UNDEFINED: u8 = 0x00;
        pub const SELECTOR_CONTROL: u8 = 0x01;
        pub const LATENCY_CONTROL: u8 = 0x02;
    }

    /// Feature Unit Control Selectors
    pub mod feature_unit {
        pub const UNDEFINED: u8 = 0x00;
        pub const MUTE_CONTROL: u8 = 0x01;
        pub const VOLUME_CONTROL: u8 = 0x02;
        pub const BASS_CONTROL: u8 = 0x03;
        pub const MID_CONTROL: u8 = 0x04;
        pub const TREBLE_CONTROL: u8 = 0x05;
        pub const GRAPHIC_EQUALIZER_CONTROL: u8 = 0x06;
        pub const AUTOMATIC_GAIN_CONTROL: u8 = 0x07;
        pub const DELAY_CONTROL: u8 = 0x08;
        pub const BASS_BOOST_CONTROL: u8 = 0x09;
        pub const LOUDNESS_CONTROL: u8 = 0x0A;
        pub const INPUT_GAIN_CONTROL: u8 = 0x0B;
        pub const INPUT_GAIN_PAD_CONTROL: u8 = 0x0C;
        pub const PHASE_INVERTER_CONTROL: u8 = 0x0D;
        pub const UNDERFLOW_CONTROL: u8 = 0x0E;
        pub const OVERFLOW_CONTROL: u8 = 0x0F;
        pub const LATENCY_CONTROL: u8 = 0x10;
    }

    /// Effect Unit Control Selectors
    pub mod effect_unit {
        /// Parametric Equalizer Section Effect Unit Control Selectors
        pub mod parametric_equalizer {
            pub const UNDEFINED: u8 = 0x00;
            pub const ENABLE_CONTROL: u8 = 0x01;
            pub const CENTERFREQ_CONTROL: u8 = 0x02;
            pub const QFACTOR_CONTROL: u8 = 0x03;
            pub const GAIN_CONTROL: u8 = 0x04;
            pub const UNDERFLOW_CONTROL: u8 = 0x05;
            pub const OVERFLOW_CONTROL: u8 = 0x06;
            pub const LATENCY_CONTROL: u8 = 0x07;
        }

        /// Reverberation Effect Unit Control Selectors
        pub mod reverberation {
            pub const UNDEFINED: u8 = 0x00;
            pub const ENABLE_CONTROL: u8 = 0x01;
            pub const TYPE_CONTROL: u8 = 0x02;
            pub const LEVEL_CONTROL: u8 = 0x03;
            pub const TIME_CONTROL: u8 = 0x04;
            pub const FEEDBACK_CONTROL: u8 = 0x05;
            pub const PREDELAY_CONTROL: u8 = 0x06;
            pub const DENSITY_CONTROL: u8 = 0x07;
            pub const HIFREQ_ROLLOFF_CONTROL: u8 = 0x08;
            pub const UNDERFLOW_CONTROL: u8 = 0x09;
            pub const OVERFLOW_CONTROL: u8 = 0x0A;
            pub const LATENCY_CONTROL: u8 = 0x0B;
        }

        /// Modulation Delay Effect Unit Control Selectors
        pub mod modulation_delay {
            pub const UNDEFINED: u8 = 0x00;
            pub const ENABLE_CONTROL: u8 = 0x01;
            pub const BALANCE_CONTROL: u8 = 0x02;
            pub const RATE_CONTROL: u8 = 0x03;
            pub const DEPTH_CONTROL: u8 = 0x04;
            pub const TIME_CONTROL: u8 = 0x05;
            pub const FEEDBACK_CONTROL: u8 = 0x06;
            pub const UNDERFLOW_CONTROL: u8 = 0x07;
            pub const OVERFLOW_CONTROL: u8 = 0x08;
            pub const LATENCY_CONTROL: u8 = 0x09;
        }

        /// Dynamic Range Compressor Effect Unit Control Selectors
        pub mod dynamic_range_compressor {
            pub const UNDEFINED: u8 = 0x00;
            pub const ENABLE_CONTROL: u8 = 0x01;
            pub const COMPRESSION_RATE_CONTROL: u8 = 0x02;
            pub const MAXAMPL_CONTROL: u8 = 0x03;
            pub const THRESHOLD_CONTROL: u8 = 0x04;
            pub const ATTACK_TIME_CONTROL: u8 = 0x05;
            pub const RELEASE_TIME_CONTROL: u8 = 0x06;
            pub const UNDERFLOW_CONTROL: u8 = 0x07;
            pub const OVERFLOW_CONTROL: u8 = 0x08;
            pub const LATENCY_CONTROL: u8 = 0x09;
        }
    }

    /// Processing Unit Control Selectors
    pub mod processing_unit {
        /// Up/Down-mix Processing Unit Control Selectors
        pub mod up_downmix {
            pub const UNDEFINED: u8 = 0x00;
            pub const ENABLE_CONTROL: u8 = 0x01;
            pub const MODE_SELECT_CONTROL: u8 = 0x02;
            pub const CLUSTER_CONTROL: u8 = 0x03;
            pub const UNDERFLOW_CONTROL: u8 = 0x04;
            pub const OVERFLOW_CONTROL: u8 = 0x05;
            pub const LATENCY_CONTROL: u8 = 0x06;
        }

        /// Dolby Prologic Processing Unit Control Selectors
        pub mod dolby_prologic {
            pub const UNDEFINED: u8 = 0x00;
            pub const ENABLE_CONTROL: u8 = 0x01;
            pub const MODE_SELECT_CONTROL: u8 = 0x02;
            pub const CLUSTER_CONTROL: u8 = 0x03;
            pub const UNDERFLOW_CONTROL: u8 = 0x04;
            pub const OVERFLOW_CONTROL: u8 = 0x05;
            pub const LATENCY_CONTROL: u8 = 0x06;
        }

        /// Stereo Extender Processing Unit Control Selectors
        pub mod stereo_extender {
            pub const UNDEFINED: u8 = 0x00;
            pub const ENABLE_CONTROL: u8 = 0x01;
            pub const WIDTH_CONTROL: u8 = 0x02;
            pub const UNDERFLOW_CONTROL: u8 = 0x03;
            pub const OVERFLOW_CONTROL: u8 = 0x04;
            pub const LATENCY_CONTROL: u8 = 0x05;
        }
    }

    /// Extension Unit Control Selectors
    pub mod extension_unit {
        pub const UNDEFINED: u8 = 0x00;
        pub const ENABLE_CONTROL: u8 = 0x01;
        pub const CLUSTER_CONTROL: u8 = 0x02;
        pub const UNDERFLOW_CONTROL: u8 = 0x03;
        pub const OVERFLOW_CONTROL: u8 = 0x04;
        pub const LATENCY_CONTROL: u8 = 0x05;
    }

    /// AudioStreaming Interface Control Selectors
    pub mod audio_streaming {
        pub const UNDEFINED: u8 = 0x00;
        pub const ACT_ALT_SETTING_CONTROL: u8 = 0x01;
        pub const VAL_ALT_SETTINGS_CONTROL: u8 = 0x02;
        pub const AUDIO_DATA_FORMAT_CONTROL: u8 = 0x03;
    }

    /// Encoder Control Selectors
    pub mod encoder {
        pub const UNDEFINED: u8 = 0x00;
        pub const BIT_RATE_CONTROL: u8 = 0x01;
        pub const QUALITY_CONTROL: u8 = 0x02;
        pub const VBR_CONTROL: u8 = 0x03;
        pub const TYPE_CONTROL: u8 = 0x04;
        pub const UNDERFLOW_CONTROL: u8 = 0x05;
        pub const OVERFLOW_CONTROL: u8 = 0x06;
        pub const ENCODER_ERROR_CONTROL: u8 = 0x07;
        pub const PARAM1_CONTROL: u8 = 0x08;
        pub const PARAM2_CONTROL: u8 = 0x09;
        pub const PARAM3_CONTROL: u8 = 0x0A;
        pub const PARAM4_CONTROL: u8 = 0x0B;
        pub const PARAM5_CONTROL: u8 = 0x0C;
        pub const PARAM6_CONTROL: u8 = 0x0D;
        pub const PARAM7_CONTROL: u8 = 0x0E;
        pub const PARAM8_CONTROL: u8 = 0x0F;
    }

    /// Decoder Control Selectors
    pub mod decoder {
        /// MPEG Decoder Control Selectors
        pub mod mpeg {
            pub const UNDEFINED: u8 = 0x00;
            pub const DUAL_CHANNEL_CONTROL: u8 = 0x01;
            pub const SECOND_STEREO_CONTROL: u8 = 0x02;
            pub const MULTILINGUAL_CONTROL: u8 = 0x03;
            pub const DYN_RANGE_CONTROL: u8 = 0x04;
            pub const SCALING_CONTROL: u8 = 0x05;
            pub const HILO_SCALING_CONTROL: u8 = 0x06;
            pub const UNDERFLOW_CONTROL: u8 = 0x07;
            pub const OVERFLOW_CONTROL: u8 = 0x08;
            pub const DECODER_ERROR_CONTROL: u8 = 0x09;
        }

        /// AC-3 Decoder Control Selectors
        pub mod ac_3 {
            pub const UNDEFINED: u8 = 0x00;
            pub const MODE_CONTROL: u8 = 0x01;
            pub const DYN_RANGE_CONTROL: u8 = 0x02;
            pub const SCALING_CONTROL: u8 = 0x03;
            pub const HILO_SCALING_CONTROL: u8 = 0x04;
            pub const UNDERFLOW_CONTROL: u8 = 0x05;
            pub const OVERFLOW_CONTROL: u8 = 0x06;
            pub const DECODER_ERROR_CONTROL: u8 = 0x07;
        }

        /// WMA Decoder Control Selectors
        pub mod wma {
            pub const UNDEFINED: u8 = 0x00;
            pub const UNDERFLOW_CONTROL: u8 = 0x01;
            pub const OVERFLOW_CONTROL: u8 = 0x02;
            pub const DECODER_ERROR_CONTROL: u8 = 0x03;
        }

        /// DTS Decoder Control Selectors
        pub mod dts {
            pub const UNDEFINED: u8 = 0x00;
            pub const UNDERFLOW_CONTROL: u8 = 0x01;
            pub const OVERFLOW_CONTROL: u8 = 0x02;
            pub const DECODER_ERROR_CONTROL: u8 = 0x03;
        }
    }

    /// Endpoint Control Selectors
    pub mod endpoint {
        pub const UNDEFINED: u8 = 0x00;
        pub const PITCH_CONTROL: u8 = 0x01;
        pub const DATA_OVERRUN_CONTROL: u8 = 0x02;
        pub const DATA_UNDERRUN_CONTROL: u8 = 0x03;
    }
}

pub mod format_type {
    pub const UNDEFINED: u8 = 0x00;
    pub const I: u8 = 0x01;
    pub const II: u8 = 0x02;
    pub const III: u8 = 0x03;
    pub const IV: u8 = 0x04;
    pub const EXT_I: u8 = 0x81;
    pub const EXT_II: u8 = 0x82;
    pub const EXT_III: u8 = 0x83;
}

pub mod terminal_type {
    // USB Terminal Types (0x01xx)
    pub mod usb {
        pub const UNDEFINED: u16 = 0x0100;
        pub const STREAMING: u16 = 0x0101;
        pub const VENDOR_SPECIFIC: u16 = 0x01FF;
    }

    // Input Terminal Types (0x02xx)
    pub mod input {
        pub const UNDEFINED: u16 = 0x0200;
        pub const MICROPHONE: u16 = 0x0201;
        pub const DESKTOP_MICROPHONE: u16 = 0x0202;
        pub const PERSONAL_MICROPHONE: u16 = 0x0203;
        pub const OMNI_DIRECTIONAL_MICROPHONE: u16 = 0x0204;
        pub const MICROPHONE_ARRAY: u16 = 0x0205;
        pub const PROCESSING_MICROPHONE_ARRAY: u16 = 0x0206;
    }

    // Output Terminal Types (0x03xx)
    pub mod output {
        pub const UNDEFINED: u16 = 0x0300;
        pub const SPEAKER: u16 = 0x0301;
        pub const HEADPHONES: u16 = 0x0302;
        pub const HEAD_MOUNTED_DISPLAY_AUDIO: u16 = 0x0303;
        pub const DESKTOP_SPEAKER: u16 = 0x0304;
        pub const ROOM_SPEAKER: u16 = 0x0305;
        pub const COMMUNICATION_SPEAKER: u16 = 0x0306;
        pub const LOW_FREQUENCY_EFFECTS_SPEAKER: u16 = 0x0307;
    }

    // Bi-directional Terminal Types (0x04xx)
    pub mod bidirectional {
        pub const UNDEFINED: u16 = 0x0400;
        pub const HANDSET: u16 = 0x0401;
        pub const HEADSET: u16 = 0x0402;
        pub const SPEAKERPHONE_NO_ECHO: u16 = 0x0403;
        pub const ECHO_SUPPRESSING_SPEAKERPHONE: u16 = 0x0404;
        pub const ECHO_CANCELING_SPEAKERPHONE: u16 = 0x0405;
    }

    // Telephony Terminal Types (0x05xx)
    pub mod telephony {
        pub const UNDEFINED: u16 = 0x0500;
        pub const PHONE_LINE: u16 = 0x0501;
        pub const TELEPHONE: u16 = 0x0502;
        pub const DOWN_LINE_PHONE: u16 = 0x0503;
    }

    // External Terminal Types (0x06xx)
    pub mod external {
        pub const UNDEFINED: u16 = 0x0600;
        pub const ANALOG_CONNECTOR: u16 = 0x0601;
        pub const DIGITAL_AUDIO_INTERFACE: u16 = 0x0602;
        pub const LINE_CONNECTOR: u16 = 0x0603;
        pub const LEGACY_AUDIO_CONNECTOR: u16 = 0x0604;
        pub const SPDIF_INTERFACE: u16 = 0x0605;
        pub const DA_STREAM_1394: u16 = 0x0606;
        pub const DV_STREAM_SOUNDTRACK_1394: u16 = 0x0607;
        pub const ADAT_LIGHTPIPE: u16 = 0x0608;
        pub const TDIF: u16 = 0x0609;
        pub const MADI: u16 = 0x060A;
    }

    // Embedded Function Terminal Types (0x07xx)
    pub mod embedded {
        pub const UNDEFINED: u16 = 0x0700;
        pub const LEVEL_CALIBRATION_NOISE_SOURCE: u16 = 0x0701;
        pub const EQUALIZATION_NOISE: u16 = 0x0702;
        pub const CD_PLAYER: u16 = 0x0703;
        pub const DAT: u16 = 0x0704;
        pub const DCC: u16 = 0x0705;
        pub const COMPRESSED_AUDIO_PLAYER: u16 = 0x0706;
        pub const ANALOG_TAPE: u16 = 0x0707;
        pub const PHONOGRAPH: u16 = 0x0708;
        pub const VCR_AUDIO: u16 = 0x0709;
        pub const VIDEO_DISC_AUDIO: u16 = 0x070A;
        pub const DVD_AUDIO: u16 = 0x070B;
        pub const TV_TUNER_AUDIO: u16 = 0x070C;
        pub const SATELLITE_RECEIVER_AUDIO: u16 = 0x070D;
        pub const CABLE_TUNER_AUDIO: u16 = 0x070E;
        pub const DSS_AUDIO: u16 = 0x070F;
        pub const RADIO_RECEIVER: u16 = 0x0710;
        pub const RADIO_TRANSMITTER: u16 = 0x0711;
        pub const MULTI_TRACK_RECORDER: u16 = 0x0712;
        pub const SYNTHESIZER: u16 = 0x0713;
        pub const PIANO: u16 = 0x0714;
        pub const GUITAR: u16 = 0x0715;
        pub const DRUMS_RHYTHM: u16 = 0x0716;
        pub const OTHER_MUSICAL_INSTRUMENT: u16 = 0x0717;
    }
}
