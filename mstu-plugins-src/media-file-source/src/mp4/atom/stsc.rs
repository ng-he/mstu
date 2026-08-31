use std::io::Seek;

use byteorder::{BigEndian, ReadBytesExt};

use crate::mp4::{
    atom::{HEADER_EXT_SIZE, HEADER_SIZE},
    *,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stsc {
    pub version: u8,
    pub flags: u32,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub first_chunk: u32,
    pub samples_per_chunk: u32,
    pub sample_description_index: u32,
    pub first_sample: u32,
}

impl Stsc {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        let full_header = atom::read_full_header(reader)?;

        let header_size = HEADER_SIZE + HEADER_EXT_SIZE;
        let other_size = size_of::<u32>(); // entry_count
        let entry_size = size_of::<u32>() + size_of::<u32>() + size_of::<u32>(); // first_chunk + samples_per_chunk + sample_description_index
        let entry_count = reader.read_u32::<BigEndian>()?;
        if u64::from(entry_count)
            > size
                .saturating_sub(header_size)
                .saturating_sub(other_size as u64)
                / entry_size as u64
        {
            return Err(Error::InvalidData(
                "stsc entry_count indicates more entries than could fit in the box",
            ));
        }
        let mut entries = Vec::with_capacity(entry_count as usize);
        for _ in 0..entry_count {
            let entry = Entry {
                first_chunk: reader.read_u32::<BigEndian>()?,
                samples_per_chunk: reader.read_u32::<BigEndian>()?,
                sample_description_index: reader.read_u32::<BigEndian>()?,
                first_sample: 0,
            };
            entries.push(entry);
        }

        let mut sample_id = 1;
        for i in 0..entry_count {
            let (first_chunk, samples_per_chunk) = {
                let entry = entries.get_mut(i as usize).unwrap();
                entry.first_sample = sample_id;
                (entry.first_chunk, entry.samples_per_chunk)
            };
            if i < entry_count - 1 {
                let next_entry = entries.get(i as usize + 1).unwrap();
                sample_id = next_entry
                    .first_chunk
                    .checked_sub(first_chunk)
                    .and_then(|n| n.checked_mul(samples_per_chunk))
                    .and_then(|n| n.checked_add(sample_id))
                    .ok_or(Error::InvalidData(
                        "attempt to calculate stsc sample_id with overflow",
                    ))?;
            }
        }

        atom::read_skip_bytes_to(reader, start + size)?;

        Ok(Stsc {
            version: full_header.version,
            flags: full_header.flags,
            entries,
        })
    }
}
