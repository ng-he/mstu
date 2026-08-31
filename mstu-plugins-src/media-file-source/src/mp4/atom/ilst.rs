use std::{
    collections::HashMap,
    io::{Read, Seek},
};

use crate::mp4::{
    Result,
    atom::{self, data::MetadataKey},
    error::Error,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ilst {
    pub items: HashMap<MetadataKey, IlstItem>,
}

impl Ilst {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        let mut items = HashMap::new();

        let mut current = reader.stream_position()?;
        let end = start + size;
        while current < end {
            let header = atom::read_header(reader)?;
            let atom::Header { name, size: s } = header;
            if s > size {
                return Err(Error::InvalidData(
                    "ilst box contains a box with a larger size than it",
                ));
            }

            match name {
                atom::constant_name::NAME => {
                    items.insert(MetadataKey::Title, IlstItem::read(reader, s)?);
                }
                atom::constant_name::DAY => {
                    items.insert(MetadataKey::Year, IlstItem::read(reader, s)?);
                }
                atom::constant_name::COVR => {
                    items.insert(MetadataKey::Poster, IlstItem::read(reader, s)?);
                }
                atom::constant_name::DESC => {
                    items.insert(MetadataKey::Summary, IlstItem::read(reader, s)?);
                }
                _ => {
                    // XXX warn!()
                    atom::read_skip(reader, s)?;
                }
            }

            current = reader.stream_position()?;
        }

        atom::read_skip_bytes_to(reader, start + size)?;

        Ok(Ilst { items })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IlstItem {
    pub data: atom::Data,
}

impl IlstItem {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        let mut data = None;

        let mut current = reader.stream_position()?;
        let end = start + size;
        while current < end {
            let header = atom::read_header(reader)?;
            let atom::Header { name, size: s } = header;
            if s > size {
                return Err(Error::InvalidData(
                    "ilst item box contains a box with a larger size than it",
                ));
            }

            match name {
                atom::constant_name::DATA => {
                    data = Some(atom::Data::read(reader, s)?);
                }
                _ => {
                    // XXX warn!()
                    atom::read_skip(reader, s)?;
                }
            }

            current = reader.stream_position()?;
        }

        if data.is_none() {
            return Err(Error::AtomNotFound(atom::constant_name::DATA));
        }

        atom::read_skip_bytes_to(reader, start + size)?;
        Ok(IlstItem {
            data: data.unwrap(),
        })
    }
}
