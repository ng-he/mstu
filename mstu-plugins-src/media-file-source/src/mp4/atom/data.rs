use std::{
    io::{Read, Seek},
    vec,
};

use byteorder::{BigEndian, ReadBytesExt};

use crate::mp4::{Result, atom, error::Error};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataType {
    Binary = 0x000000,
    Text = 0x000001,
    Image = 0x00000D,
    TempoCpil = 0x000015,
}

impl TryFrom<u32> for DataType {
    type Error = Error;
    fn try_from(value: u32) -> Result<DataType> {
        match value {
            0x000000 => Ok(DataType::Binary),
            0x000001 => Ok(DataType::Text),
            0x00000D => Ok(DataType::Image),
            0x000015 => Ok(DataType::TempoCpil),
            _ => Err(Error::InvalidData("invalid data type")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MetadataKey {
    Title,
    Year,
    Poster,
    Summary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Data {
    pub data: Vec<u8>,
    pub data_type: DataType,
}

impl Data {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        let data_type = DataType::try_from(reader.read_u32::<BigEndian>()?)?;

        reader.read_u32::<BigEndian>()?; // reserved = 0

        let current = reader.stream_position()?;
        let mut data = vec![0u8; (start + size - current) as usize];
        reader.read_exact(&mut data)?;

        Ok(Data { data, data_type })
    }
}
