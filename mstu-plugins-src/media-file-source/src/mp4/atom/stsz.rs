use std::io::Seek;

use byteorder::{BigEndian, ReadBytesExt};

use crate::mp4::{
    atom::{HEADER_EXT_SIZE, HEADER_SIZE},
    *,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stsz {
    pub version: u8,
    pub flags: u32,
    pub sample_size: u32,
    pub sample_count: u32,
    pub sample_sizes: Vec<u32>,
}

impl Stsz {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        let full_header = atom::read_full_header(reader)?;

        let header_size = HEADER_SIZE + HEADER_EXT_SIZE;
        let other_size = size_of::<u32>() + size_of::<u32>(); // sample_size + sample_count
        let sample_size = reader.read_u32::<BigEndian>()?;
        let stsz_item_size = if sample_size == 0 {
            size_of::<u32>() // entry_size
        } else {
            0
        };
        let sample_count = reader.read_u32::<BigEndian>()?;
        let mut sample_sizes = Vec::new();
        if sample_size == 0 {
            if u64::from(sample_count)
                > size
                    .saturating_sub(header_size)
                    .saturating_sub(other_size as u64)
                    / stsz_item_size as u64
            {
                return Err(Error::InvalidData(
                    "stsz sample_count indicates more values than could fit in the box",
                ));
            }
            sample_sizes.reserve(sample_count as usize);
            for _ in 0..sample_count {
                let sample_number = reader.read_u32::<BigEndian>()?;
                sample_sizes.push(sample_number);
            }
        }

        atom::read_skip_bytes_to(reader, start + size)?;

        Ok(Stsz {
            version: full_header.version,
            flags: full_header.flags,
            sample_size,
            sample_count,
            sample_sizes,
        })
    }
}
