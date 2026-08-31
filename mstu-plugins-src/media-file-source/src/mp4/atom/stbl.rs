use std::io::Seek;

use crate::mp4::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stbl {
    pub stsd: atom::Stsd,
    pub stts: atom::Stts,
    pub ctts: Option<atom::Ctts>,

    pub stss: Option<atom::Stss>,
    pub stsc: atom::Stsc,
    pub stsz: atom::Stsz,

    pub stco: Option<atom::Stco>,
    pub co64: Option<atom::Co64>,
}

impl Stbl {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        let mut stsd = None;
        let mut stts = None;
        let mut ctts = None;
        let mut stss = None;
        let mut stsc = None;
        let mut stsz = None;
        let mut stco = None;
        let mut co64 = None;

        let mut current = reader.stream_position()?;
        let end = start + size;
        while current < end {
            let header = atom::read_header(reader)?;
            let atom::Header { name, size: s } = header;
            if s > size {
                return Err(Error::InvalidData(
                    "stbl box contains a box with a larger size than it",
                ));
            }

            match name {
                atom::constant_name::STSD => {
                    stsd = Some(atom::Stsd::read(reader, s)?);
                }
                atom::constant_name::STTS => {
                    stts = Some(atom::Stts::read(reader, s)?);
                }
                atom::constant_name::CTTS => {
                    ctts = Some(atom::Ctts::read(reader, s)?);
                }
                atom::constant_name::STSS => {
                    stss = Some(atom::Stss::read(reader, s)?);
                }
                atom::constant_name::STSC => {
                    stsc = Some(atom::Stsc::read(reader, s)?);
                }
                atom::constant_name::STSZ => {
                    stsz = Some(atom::Stsz::read(reader, s)?);
                }
                atom::constant_name::STCO => {
                    stco = Some(atom::Stco::read(reader, s)?);
                }
                atom::constant_name::CO64 => {
                    co64 = Some(atom::Co64::read(reader, s)?);
                }
                _ => {
                    // XXX warn!()
                    atom::read_skip(reader, s)?;
                }
            }
            current = reader.stream_position()?;
        }

        if stsd.is_none() {
            return Err(Error::AtomNotFound(atom::constant_name::STSD));
        }
        if stts.is_none() {
            return Err(Error::AtomNotFound(atom::constant_name::STTS));
        }
        if stsc.is_none() {
            return Err(Error::AtomNotFound(atom::constant_name::STSC));
        }
        if stsz.is_none() {
            return Err(Error::AtomNotFound(atom::constant_name::STSZ));
        }
        if stco.is_none() && co64.is_none() {
            return Err(Error::Atom2NotFound(
                atom::constant_name::STCO,
                atom::constant_name::CO64,
            ));
        }

        atom::read_skip_bytes_to(reader, start + size)?;

        Ok(Stbl {
            stsd: stsd.unwrap(),
            stts: stts.unwrap(),
            ctts,
            stss,
            stsc: stsc.unwrap(),
            stsz: stsz.unwrap(),
            stco,
            co64,
        })
    }
}
