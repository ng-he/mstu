use crate::mp4::*;

use byteorder::{BigEndian, ReadBytesExt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stsd {
    pub version: u8,
    pub flags: u32,

    pub avc1: Option<atom::Avc1>,
    pub hev1: Option<atom::Hev1>,
    pub vp09: Option<atom::Vp09>,
    pub mp4a: Option<atom::Mp4a>,
    pub tx3g: Option<atom::Tx3g>,
}

impl Stsd {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        let full_header = atom::read_full_header(reader)?;

        reader.read_u32::<BigEndian>()?; // XXX entry_count

        let mut avc1 = None;
        let mut hev1 = None;
        let mut vp09 = None;
        let mut mp4a = None;
        let mut tx3g = None;

        let header = atom::read_header(reader)?;
        let atom::Header { name, size: s } = header;
        if s > size {
            return Err(Error::InvalidData(
                "stsd box contains a box with a larger size than it",
            ));
        }

        match name {
            atom::constant_name::AVC1 => {
                avc1 = Some(atom::Avc1::read(reader, s)?);
            }
            atom::constant_name::HEV1 => {
                hev1 = Some(atom::Hev1::read(reader, s)?);
            }
            atom::constant_name::VP09 => {
                vp09 = Some(atom::Vp09::read(reader, s)?);
            }
            atom::constant_name::MP4A => {
                mp4a = Some(atom::Mp4a::read(reader, s)?);
            }
            atom::constant_name::TX3G => {
                tx3g = Some(atom::Tx3g::read(reader, s)?);
            }
            _ => {}
        }

        atom::read_skip_bytes_to(reader, start + size)?;

        Ok(Stsd {
            version: full_header.version,
            flags: full_header.flags,
            avc1,
            hev1,
            vp09,
            mp4a,
            tx3g,
        })
    }
}
