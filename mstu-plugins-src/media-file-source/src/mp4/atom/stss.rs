use std::io::Seek;

use byteorder::{BigEndian, ReadBytesExt};

use crate::mp4::{
    atom::{HEADER_EXT_SIZE, HEADER_SIZE},
    *,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stss {
    pub version: u8,
    pub flags: u32,
    pub entries: Vec<u32>,
}

impl Stss {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        let full_header = atom::read_full_header(reader)?;

        let header_size = HEADER_SIZE + HEADER_EXT_SIZE;
        let other_size = size_of::<u32>(); // entry_count
        let entry_size = size_of::<u32>(); // sample_number
        let entry_count = reader.read_u32::<BigEndian>()?;
        if u64::from(entry_count)
            > size
                .saturating_sub(header_size)
                .saturating_sub(other_size as u64)
                / entry_size as u64
        {
            return Err(Error::InvalidData(
                "stss entry_count indicates more entries than could fit in the box",
            ));
        }
        let mut entries = Vec::with_capacity(entry_count as usize);
        for _i in 0..entry_count {
            let sample_number = reader.read_u32::<BigEndian>()?;
            entries.push(sample_number);
        }

        atom::read_skip_bytes_to(reader, start + size)?;

        Ok(Stss {
            version: full_header.version,
            flags: full_header.flags,
            entries,
        })
    }
}
