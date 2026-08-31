use std::io::Seek;

use byteorder::{BigEndian, ReadBytesExt};

use crate::mp4::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vp09 {
    pub version: u8,
    pub flags: u32,
    pub start_code: u16,
    pub data_reference_index: u16,
    pub reserved0: [u8; 16],
    pub width: u16,
    pub height: u16,
    pub horizontal_resolution: (u16, u16),
    pub vertical_resolution: (u16, u16),
    pub reserved1: [u8; 4],
    pub frame_count: u16,
    pub compressor_name: [u8; 32],
    pub depth: u16,
    pub end_code: u16,
    pub vpcc: Vpcc,
}

impl Vp09 {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        let full_header = atom::read_full_header(reader)?;

        let start_code: u16 = reader.read_u16::<BigEndian>()?;
        let data_reference_index: u16 = reader.read_u16::<BigEndian>()?;
        let reserved0: [u8; 16] = {
            let mut buf = [0u8; 16];
            reader.read_exact(&mut buf)?;
            buf
        };
        let width: u16 = reader.read_u16::<BigEndian>()?;
        let height: u16 = reader.read_u16::<BigEndian>()?;
        let horizontal_resolution: (u16, u16) = (
            reader.read_u16::<BigEndian>()?,
            reader.read_u16::<BigEndian>()?,
        );
        let vertical_resolution: (u16, u16) = (
            reader.read_u16::<BigEndian>()?,
            reader.read_u16::<BigEndian>()?,
        );
        let reserved1: [u8; 4] = {
            let mut buf = [0u8; 4];
            reader.read_exact(&mut buf)?;
            buf
        };
        let frame_count: u16 = reader.read_u16::<BigEndian>()?;
        let compressor_name: [u8; 32] = {
            let mut buf = [0u8; 32];
            reader.read_exact(&mut buf)?;
            buf
        };
        let depth: u16 = reader.read_u16::<BigEndian>()?;
        let end_code: u16 = reader.read_u16::<BigEndian>()?;

        let vpcc = {
            let header = atom::read_header(reader)?;
            if header.size > size {
                return Err(Error::InvalidData(
                    "vp09 box contains a box with a larger size than it",
                ));
            }
            Vpcc::read(reader, header.size)?
        };

        atom::read_skip_bytes_to(reader, start + size)?;

        Ok(Self {
            version: full_header.version,
            flags: full_header.flags,
            start_code,
            data_reference_index,
            reserved0,
            width,
            height,
            horizontal_resolution,
            vertical_resolution,
            reserved1,
            frame_count,
            compressor_name,
            depth,
            end_code,
            vpcc,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vpcc {
    pub version: u8,
    pub flags: u32,
    pub profile: u8,
    pub level: u8,
    pub bit_depth: u8,
    pub chroma_subsampling: u8,
    pub video_full_range_flag: bool,
    pub color_primaries: u8,
    pub transfer_characteristics: u8,
    pub matrix_coefficients: u8,
    pub codec_initialization_data_size: u16,
}

impl Vpcc {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        let full_header = atom::read_full_header(reader)?;

        let profile: u8 = reader.read_u8()?;
        let level: u8 = reader.read_u8()?;
        let (bit_depth, chroma_subsampling, video_full_range_flag) = {
            let b = reader.read_u8()?;
            (b >> 4, b << 4 >> 5, b & 0x01 == 1)
        };
        let transfer_characteristics: u8 = reader.read_u8()?;
        let matrix_coefficients: u8 = reader.read_u8()?;
        let codec_initialization_data_size: u16 = reader.read_u16::<BigEndian>()?;

        atom::read_skip_bytes_to(reader, start + size)?;

        Ok(Self {
            version: full_header.version,
            flags: full_header.flags,
            profile,
            level,
            bit_depth,
            chroma_subsampling,
            video_full_range_flag,
            color_primaries: 0,
            transfer_characteristics,
            matrix_coefficients,
            codec_initialization_data_size,
        })
    }
}
