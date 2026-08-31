use crate::Nalu;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NaluType {
    Unknown(u8),
    Vps = 32,
    Sps = 33,
    Pps = 34,
    Aud = 35,

    PrefixSei = 39,
    SuffixSei = 40,
}

impl From<u8> for NaluType {
    fn from(v: u8) -> Self {
        match v {
            32 => Self::Vps,
            33 => Self::Sps,
            34 => Self::Pps,
            35 => Self::Aud,
            39 => Self::PrefixSei,
            40 => Self::SuffixSei,
            _ => Self::Unknown(v),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Parameters {
    pub sps: Vec<Nalu>,
    pub pps: Vec<Nalu>,
    pub vps: Vec<Nalu>,
}

pub mod rtp {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(u8)]
    pub enum PacketizationType {
        Unknown(u8),
        Ap = 48,
        Fu = 49,
    }

    impl From<u8> for PacketizationType {
        fn from(v: u8) -> Self {
            match v {
                48 => Self::Ap,
                49 => Self::Fu,
                _ => Self::Unknown(v),
            }
        }
    }
}
