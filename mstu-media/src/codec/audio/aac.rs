#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u8)]
pub enum AudioObjectType {
    AacMain = 1,                                       // AAC Main Profile
    AacLowComplexity = 2,                              // AAC Low Complexity
    AacScalableSampleRate = 3,                         // AAC Scalable Sample Rate
    AacLongTermPrediction = 4,                         // AAC Long Term Predictor
    SpectralBandReplication = 5,                       // Spectral band Replication
    AACScalable = 6,                                   // AAC Scalable
    TwinVQ = 7,                                        // Twin VQ
    CodeExcitedLinearPrediction = 8,                   // CELP
    HarmonicVectorExcitationCoding = 9,                // HVXC
    TextToSpeechtInterface = 12,                       // TTSI
    MainSynthetic = 13,                                // Main Synthetic
    WavetableSynthesis = 14,                           // Wavetable Synthesis
    GeneralMIDI = 15,                                  // General MIDI
    AlgorithmicSynthesis = 16,                         // Algorithmic Synthesis
    ErrorResilientAacLowComplexity = 17,               // ER AAC LC
    ErrorResilientAacLongTermPrediction = 19,          // ER AAC LTP
    ErrorResilientAacScalable = 20,                    // ER AAC Scalable
    ErrorResilientAacTwinVQ = 21,                      // ER AAC TwinVQ
    ErrorResilientAacBitSlicedArithmeticCoding = 22,   // ER Bit Sliced Arithmetic Coding
    ErrorResilientAacLowDelay = 23,                    // ER AAC Low Delay
    ErrorResilientCodeExcitedLinearPrediction = 24,    // ER CELP
    ErrorResilientHarmonicVectorExcitationCoding = 25, // ER HVXC
    ErrorResilientHarmonicIndividualLinesNoise = 26,   // ER HILN
    ErrorResilientParametric = 27,                     // ER Parametric
    SinuSoidalCoding = 28,                             // SSC
    ParametricStereo = 29,                             // PS
    MpegSurround = 30,                                 // MPEG Surround
    MpegLayer1 = 32,                                   // MPEG Layer 1
    MpegLayer2 = 33,                                   // MPEG Layer 2
    MpegLayer3 = 34,                                   // MPEG Layer 3
    DirectStreamTransfer = 35,                         // DST Direct Stream Transfer
    AudioLosslessCoding = 36,                          // ALS Audio Lossless Coding
    ScalableLosslessCoding = 37,                       // SLC Scalable Lossless Coding
    ScalableLosslessCodingNoneCore = 38,               // SLC non-core
    ErrorResilientAacEnhancedLowDelay = 39,            // ER AAC ELD
    SymbolicMusicRepresentationSimple = 40,            // SMR Simple
    SymbolicMusicRepresentationMain = 41,              // SMR Main
    UnifiedSpeechAudioCoding = 42,                     // USAC
    SpatialAudioObjectCoding = 43,                     // SAOC
    LowDelayMpegSurround = 44,                         // LD MPEG Surround
    SpatialAudioObjectCodingDialogueEnhancement = 45,  // SAOC-DE
    AudioSync = 46,                                    // Audio Sync
    Unknown(u8),
}

impl From<u8> for AudioObjectType {
    fn from(value: u8) -> AudioObjectType {
        match value {
            1 => AudioObjectType::AacMain,
            2 => AudioObjectType::AacLowComplexity,
            3 => AudioObjectType::AacScalableSampleRate,
            4 => AudioObjectType::AacLongTermPrediction,
            5 => AudioObjectType::SpectralBandReplication,
            6 => AudioObjectType::AACScalable,
            7 => AudioObjectType::TwinVQ,
            8 => AudioObjectType::CodeExcitedLinearPrediction,
            9 => AudioObjectType::HarmonicVectorExcitationCoding,
            12 => AudioObjectType::TextToSpeechtInterface,
            13 => AudioObjectType::MainSynthetic,
            14 => AudioObjectType::WavetableSynthesis,
            15 => AudioObjectType::GeneralMIDI,
            16 => AudioObjectType::AlgorithmicSynthesis,
            17 => AudioObjectType::ErrorResilientAacLowComplexity,
            19 => AudioObjectType::ErrorResilientAacLongTermPrediction,
            20 => AudioObjectType::ErrorResilientAacScalable,
            21 => AudioObjectType::ErrorResilientAacTwinVQ,
            22 => AudioObjectType::ErrorResilientAacBitSlicedArithmeticCoding,
            23 => AudioObjectType::ErrorResilientAacLowDelay,
            24 => AudioObjectType::ErrorResilientCodeExcitedLinearPrediction,
            25 => AudioObjectType::ErrorResilientHarmonicVectorExcitationCoding,
            26 => AudioObjectType::ErrorResilientHarmonicIndividualLinesNoise,
            27 => AudioObjectType::ErrorResilientParametric,
            28 => AudioObjectType::SinuSoidalCoding,
            29 => AudioObjectType::ParametricStereo,
            30 => AudioObjectType::MpegSurround,
            32 => AudioObjectType::MpegLayer1,
            33 => AudioObjectType::MpegLayer2,
            34 => AudioObjectType::MpegLayer3,
            35 => AudioObjectType::DirectStreamTransfer,
            36 => AudioObjectType::AudioLosslessCoding,
            37 => AudioObjectType::ScalableLosslessCoding,
            38 => AudioObjectType::ScalableLosslessCodingNoneCore,
            39 => AudioObjectType::ErrorResilientAacEnhancedLowDelay,
            40 => AudioObjectType::SymbolicMusicRepresentationSimple,
            41 => AudioObjectType::SymbolicMusicRepresentationMain,
            42 => AudioObjectType::UnifiedSpeechAudioCoding,
            43 => AudioObjectType::SpatialAudioObjectCoding,
            44 => AudioObjectType::LowDelayMpegSurround,
            45 => AudioObjectType::SpatialAudioObjectCodingDialogueEnhancement,
            46 => AudioObjectType::AudioSync,
            _ => AudioObjectType::Unknown(value),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u8)]
pub enum SamplingFrequencyIndex {
    Freq96000 = 0x0,
    Freq88200 = 0x1,
    Freq64000 = 0x2,
    Freq48000 = 0x3,
    Freq44100 = 0x4,
    Freq32000 = 0x5,
    Freq24000 = 0x6,
    Freq22050 = 0x7,
    Freq16000 = 0x8,
    Freq12000 = 0x9,
    Freq11025 = 0xa,
    Freq8000 = 0xb,
    Freq7350 = 0xc,
    Unknown(u8),
}

impl From<u8> for SamplingFrequencyIndex {
    fn from(value: u8) -> SamplingFrequencyIndex {
        match value {
            0x0 => SamplingFrequencyIndex::Freq96000,
            0x1 => SamplingFrequencyIndex::Freq88200,
            0x2 => SamplingFrequencyIndex::Freq64000,
            0x3 => SamplingFrequencyIndex::Freq48000,
            0x4 => SamplingFrequencyIndex::Freq44100,
            0x5 => SamplingFrequencyIndex::Freq32000,
            0x6 => SamplingFrequencyIndex::Freq24000,
            0x7 => SamplingFrequencyIndex::Freq22050,
            0x8 => SamplingFrequencyIndex::Freq16000,
            0x9 => SamplingFrequencyIndex::Freq12000,
            0xa => SamplingFrequencyIndex::Freq11025,
            0xb => SamplingFrequencyIndex::Freq8000,
            0xc => SamplingFrequencyIndex::Freq7350,
            _ => SamplingFrequencyIndex::Unknown(value),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u8)]
pub enum ChannelConfiguration {
    Unknown(u8),
    Mono = 0x1,
    Stereo = 0x2,
    Surround3_0 = 0x3,
    Surround4_0 = 0x4,
    Surround5_0 = 0x5,
    Surround5_1 = 0x6,
    Surround7_1 = 0x7,
}

impl From<u8> for ChannelConfiguration {
    fn from(value: u8) -> ChannelConfiguration {
        match value {
            0x1 => ChannelConfiguration::Mono,
            0x2 => ChannelConfiguration::Stereo,
            0x3 => ChannelConfiguration::Surround3_0,
            0x4 => ChannelConfiguration::Surround4_0,
            0x5 => ChannelConfiguration::Surround5_0,
            0x6 => ChannelConfiguration::Surround5_1,
            0x7 => ChannelConfiguration::Surround7_1,
            _ => ChannelConfiguration::Unknown(value),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Parameters {
    pub bitrate: u32,
    pub profile: AudioObjectType,
    pub freq_index: SamplingFrequencyIndex,
    pub channel_config: ChannelConfiguration,
}
