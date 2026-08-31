use std::io::Seek;

use crate::mp4::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edts {
    pub elst: Option<atom::Elst>,
}

impl Edts {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        let mut edts = Self { elst: None };

        let header = atom::read_header(reader)?;
        let atom::Header { name, size: s } = header;
        if s > size {
            return Err(Error::InvalidData(
                "edts box contains a box with a larger size than it",
            ));
        }

        if name == atom::constant_name::EDTS {
            let elst = atom::Elst::read(reader, s)?;
            edts.elst = Some(elst);
        }

        atom::read_skip_bytes_to(reader, start + size)?;

        Ok(edts)
    }
}
