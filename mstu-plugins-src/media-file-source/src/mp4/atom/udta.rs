use std::io::Seek;

use crate::mp4::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Udta {
    pub meta: Option<atom::Meta>,
}

impl Udta {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        let mut meta = None;

        let mut current = reader.stream_position()?;
        let end = start + size;
        while current < end {
            let header = atom::read_header(reader)?;
            let atom::Header { name, size: s } = header;
            if s > size {
                return Err(Error::InvalidData(
                    "udta box contains a box with a larger size than it",
                ));
            }

            match name {
                atom::constant_name::META => {
                    meta = Some(atom::Meta::read(reader, s)?);
                }
                _ => {
                    // XXX warn!()
                    atom::read_skip(reader, s)?;
                }
            }

            current = reader.stream_position()?;
        }

        atom::read_skip_bytes_to(reader, start + size)?;
        Ok(Udta { meta })
    }
}
