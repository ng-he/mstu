use std::io::Seek;

use crate::mp4::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Minf {
    pub vmhd: Option<atom::Vmhd>,
    pub smhd: Option<atom::Smhd>,
    pub dinf: atom::Dinf,
    pub stbl: atom::Stbl,
}

impl Minf {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        let mut vmhd = None;
        let mut smhd = None;
        let mut dinf = None;
        let mut stbl = None;

        let mut current = reader.stream_position()?;
        let end = start + size;
        while current < end {
            let header = atom::read_header(reader)?;
            let atom::Header { name, size: s } = header;
            if s > size {
                return Err(Error::InvalidData(
                    "minf box contains a box with a larger size than it",
                ));
            }

            match name {
                atom::constant_name::VMHD => {
                    vmhd = Some(atom::Vmhd::read(reader, s)?);
                }
                atom::constant_name::SMHD => {
                    smhd = Some(atom::Smhd::read(reader, s)?);
                }
                atom::constant_name::DINF => {
                    dinf = Some(atom::Dinf::read(reader, s)?);
                }
                atom::constant_name::STBL => {
                    stbl = Some(atom::Stbl::read(reader, s)?);
                }
                _ => {
                    // XXX warn!()
                    atom::read_skip(reader, s)?;
                }
            }

            current = reader.stream_position()?;
        }

        if dinf.is_none() {
            return Err(Error::AtomNotFound(atom::constant_name::DINF));
        }
        if stbl.is_none() {
            return Err(Error::AtomNotFound(atom::constant_name::STBL));
        }

        atom::read_skip_bytes_to(reader, start + size)?;

        Ok(Minf {
            vmhd,
            smhd,
            dinf: dinf.unwrap(),
            stbl: stbl.unwrap(),
        })
    }
}
