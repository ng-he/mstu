use std::io::Seek;

use byteorder::{BigEndian, ReadBytesExt};

use crate::mp4::{
    atom::{HEADER_EXT_SIZE, HEADER_SIZE},
    *,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ctts {
    pub version: u8,
    pub flags: u32,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub sample_count: u32,
    pub sample_offset: i32,
}

impl Ctts {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        let full_header = atom::read_full_header(reader)?;

        let header_size = HEADER_SIZE + HEADER_EXT_SIZE;
        let entry_count = reader.read_u32::<BigEndian>()?;
        let entry_size = size_of::<u32>() + size_of::<i32>(); // sample_count + sample_offset
        // (sample_offset might be a u32, but the size is the same.)
        let other_size = size_of::<i32>(); // entry_count
        if u64::from(entry_count)
            > size
                .saturating_sub(header_size)
                .saturating_sub(other_size as u64)
                / entry_size as u64
        {
            return Err(Error::InvalidData(
                "ctts entry_count indicates more entries than could fit in the box",
            ));
        }
        let mut entries = Vec::with_capacity(entry_count as usize);
        for _ in 0..entry_count {
            let entry = Entry {
                sample_count: reader.read_u32::<BigEndian>()?,
                sample_offset: reader.read_i32::<BigEndian>()?,
            };
            entries.push(entry);
        }

        atom::read_skip_bytes_to(reader, start + size)?;

        Ok(Ctts {
            version: full_header.version,
            flags: full_header.flags,
            entries,
        })
    }
}
