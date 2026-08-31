use std::io::Seek;

use crate::mp4::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trak {
    pub tkhd: atom::Tkhd,
    pub edts: Option<atom::Edts>,
    pub meta: Option<atom::Meta>,
    pub mdia: atom::Mdia,
}

impl Trak {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        let mut tkhd = None;
        let mut edts = None;
        let mut meta = None;
        let mut mdia = None;

        let mut current = reader.stream_position()?;
        let end = start + size;
        while current < end {
            let header = atom::read_header(reader)?;
            let atom::Header { name, size: s } = header;
            if s > size {
                return Err(Error::InvalidData(
                    "trak box contains a box with a larger size than it",
                ));
            }

            match name {
                atom::constant_name::TKHD => {
                    tkhd = Some(atom::Tkhd::read(reader, s)?);
                }
                atom::constant_name::EDTS => {
                    edts = Some(atom::Edts::read(reader, s)?);
                }
                atom::constant_name::META => {
                    meta = Some(atom::Meta::read(reader, s)?);
                }
                atom::constant_name::MDIA => {
                    mdia = Some(atom::Mdia::read(reader, s)?);
                }
                _ => {
                    // XXX warn!()
                    atom::read_skip(reader, s)?;
                }
            }

            current = reader.stream_position()?;
        }

        if tkhd.is_none() {
            return Err(Error::AtomNotFound(atom::constant_name::TKHD));
        }
        if mdia.is_none() {
            return Err(Error::AtomNotFound(atom::constant_name::MDIA));
        }

        atom::read_skip_bytes_to(reader, start + size)?;

        Ok(Trak {
            tkhd: tkhd.unwrap(),
            edts,
            meta,
            mdia: mdia.unwrap(),
        })
    }
}
