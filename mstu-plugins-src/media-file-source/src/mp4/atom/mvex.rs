use std::io::Seek;

use crate::mp4::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mvex {
    pub mehd: Option<atom::Mehd>,
    pub trex: atom::Trex,
}

impl Mvex {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        let mut mehd = None;
        let mut trex = None;

        let mut current = reader.stream_position()?;
        let end = start + size;
        while current < end {
            let header = atom::read_header(reader)?;
            let atom::Header { name, size: s } = header;
            if s > size {
                return Err(Error::InvalidData(
                    "mvex box contains a box with a larger size than it",
                ));
            }

            match name {
                atom::constant_name::MEHD => {
                    mehd = Some(atom::Mehd::read(reader, s)?);
                }
                atom::constant_name::TREX => {
                    trex = Some(atom::Trex::read(reader, s)?);
                }
                _ => {
                    // XXX warn!()
                    atom::read_skip(reader, s)?;
                }
            }

            current = reader.stream_position()?;
        }

        if trex.is_none() {
            return Err(Error::AtomNotFound(atom::constant_name::TREX));
        }

        atom::read_skip_bytes_to(reader, start + size)?;

        Ok(Mvex {
            mehd,
            trex: trex.unwrap(),
        })
    }
}
