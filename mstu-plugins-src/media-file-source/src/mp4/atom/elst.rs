use std::io::Seek;

use byteorder::{BigEndian, ReadBytesExt};

use crate::mp4::{
    atom::{HEADER_EXT_SIZE, HEADER_SIZE},
    *,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Elst {
    pub version: u8,
    pub flags: u32,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub segment_duration: u64,
    pub media_time: u64,
    pub media_rate: u16,
    pub media_rate_fraction: u16,
}

impl Elst {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        let full_header = atom::read_full_header(reader)?;

        let header_size = HEADER_SIZE + HEADER_EXT_SIZE;
        let entry_count = reader.read_u32::<BigEndian>()?;
        let other_size = size_of::<i32>(); // entry_count
        let entry_size = {
            let mut entry_size = 0;
            entry_size += if full_header.version == 1 {
                size_of::<u64>() + size_of::<i64>() // segment_duration + media_time
            } else {
                size_of::<u32>() + size_of::<i32>() // segment_duration + media_time
            };
            entry_size += size_of::<i16>() + size_of::<i16>(); // media_rate_integer + media_rate_fraction
            entry_size
        };
        if u64::from(entry_count)
            > size
                .saturating_sub(header_size)
                .saturating_sub(other_size as u64)
                / entry_size as u64
        {
            return Err(Error::InvalidData(
                "elst entry_count indicates more entries than could fit in the box",
            ));
        }
        let mut entries = Vec::with_capacity(entry_count as usize);
        for _ in 0..entry_count {
            let (segment_duration, media_time) = if full_header.version == 1 {
                (
                    reader.read_u64::<BigEndian>()?,
                    reader.read_u64::<BigEndian>()?,
                )
            } else {
                (
                    reader.read_u32::<BigEndian>()? as u64,
                    reader.read_u32::<BigEndian>()? as u64,
                )
            };

            let entry = Entry {
                segment_duration,
                media_time,
                media_rate: reader.read_u16::<BigEndian>()?,
                media_rate_fraction: reader.read_u16::<BigEndian>()?,
            };
            entries.push(entry);
        }

        atom::read_skip_bytes_to(reader, start + size)?;

        Ok(Elst {
            version: full_header.version,
            flags: full_header.flags,
            entries,
        })
    }
}
