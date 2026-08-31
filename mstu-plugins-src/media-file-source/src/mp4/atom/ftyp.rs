use std::io::Seek;

use byteorder::{BigEndian, ReadBytesExt};
use smallvec::SmallVec;

use crate::media::FourCC;
use crate::mp4::*;

#[derive(Debug, Clone)]
pub struct Ftyp {
    pub major_brand: FourCC,
    pub minor_version: u32,
    pub compatible_brands: SmallVec<[FourCC; 4]>,
}

impl Ftyp {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        if size < 16 || size % 4 != 0 {
            return Err(Error::InvalidData("ftyp size too small or not aligned"));
        }

        let brand_count = (size - 16) / 4; // header + major + minor
        let major = reader.read_u32::<BigEndian>()?;
        let minor = reader.read_u32::<BigEndian>()?;

        let mut brands = SmallVec::<[FourCC; 4]>::new();
        for _ in 0..brand_count {
            let b = reader.read_u32::<BigEndian>()?;
            brands.push(FourCC::from(b.to_be_bytes()));
        }

        atom::read_skip_bytes_to(reader, start + size)?;
        Ok(Ftyp {
            major_brand: FourCC::from(major.to_be_bytes()),
            minor_version: minor,
            compatible_brands: brands,
        })
    }
}
