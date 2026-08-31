use crate::Nalu;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Profile {
    Unknown(u8, bool),
    ConstrainedBaseline,
    Baseline,
    Main,
    Extended,
    High,
}

impl From<(u8, u8)> for Profile {
    fn from(value: (u8, u8)) -> Profile {
        let profile: u8 = value.0;
        let constrained = value.1 & 0x40 != 0;
        match (profile, constrained) {
            (66, true) => Profile::ConstrainedBaseline,
            (66, false) => Profile::Baseline,
            (77, _) => Profile::Main,
            (88, _) => Profile::Extended,
            (100, _) => Profile::High,
            _ => Profile::Unknown(profile, constrained),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NaluType {
    Unknown(u8),
    Slice = 1,
    Idr = 5,
    Sei = 6,
    Sps = 7,
    Pps = 8,
    Aud = 9,
}

impl From<u8> for NaluType {
    fn from(v: u8) -> Self {
        match v {
            1 => Self::Slice,
            5 => Self::Idr,
            6 => Self::Sei,
            7 => Self::Sps,
            8 => Self::Pps,
            9 => Self::Aud,
            _ => Self::Unknown(v),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Parameters {
    pub profile: Profile,
    pub sps: Vec<Nalu>,
    pub pps: Vec<Nalu>,
}

pub mod rtp {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(u8)]
    pub enum PacketizationType {
        Unknown(u8),
        StapA = 24,
        FuA = 28,
    }

    impl From<u8> for PacketizationType {
        fn from(v: u8) -> Self {
            match v {
                24 => Self::StapA,
                28 => Self::FuA,
                _ => Self::Unknown(v),
            }
        }
    }
}
