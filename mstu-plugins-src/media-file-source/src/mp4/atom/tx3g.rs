use crate::mp4::*;

use byteorder::{BigEndian, ReadBytesExt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tx3g {
    pub data_reference_index: u16,
    pub display_flags: u32,
    pub horizontal_justification: i8,
    pub vertical_justification: i8,
    pub bg_color_rgba: RgbaColor,
    pub box_record: [i16; 4],
    pub style_record: [u8; 12],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Tx3g {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        reader.read_u32::<BigEndian>()?; // reserved
        reader.read_u16::<BigEndian>()?; // reserved
        let data_reference_index = reader.read_u16::<BigEndian>()?;

        let display_flags = reader.read_u32::<BigEndian>()?;
        let horizontal_justification = reader.read_i8()?;
        let vertical_justification = reader.read_i8()?;
        let bg_color_rgba = RgbaColor {
            r: reader.read_u8()?,
            g: reader.read_u8()?,
            b: reader.read_u8()?,
            a: reader.read_u8()?,
        };
        let box_record: [i16; 4] = [
            reader.read_i16::<BigEndian>()?,
            reader.read_i16::<BigEndian>()?,
            reader.read_i16::<BigEndian>()?,
            reader.read_i16::<BigEndian>()?,
        ];
        let style_record: [u8; 12] = [
            reader.read_u8()?,
            reader.read_u8()?,
            reader.read_u8()?,
            reader.read_u8()?,
            reader.read_u8()?,
            reader.read_u8()?,
            reader.read_u8()?,
            reader.read_u8()?,
            reader.read_u8()?,
            reader.read_u8()?,
            reader.read_u8()?,
            reader.read_u8()?,
        ];

        atom::read_skip_bytes_to(reader, start + size)?;

        Ok(Tx3g {
            data_reference_index,
            display_flags,
            horizontal_justification,
            vertical_justification,
            bg_color_rgba,
            box_record,
            style_record,
        })
    }
}
