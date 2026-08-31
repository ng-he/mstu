use std::io::Seek;

use byteorder::{BigEndian, ReadBytesExt};

use crate::media::nalu::Nalu;
use crate::mp4::*;
use crate::types::FixedU16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Avc1 {
    pub data_reference_index: u16,
    pub width: u16,
    pub height: u16,
    pub horizontal_resolution: FixedU16,
    pub vertical_resolution: FixedU16,
    pub frame_count: u16,
    pub depth: u16,
    pub avcc: AvcC,
}

impl Avc1 {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        reader.read_u32::<BigEndian>()?; // reserved
        reader.read_u16::<BigEndian>()?; // reserved
        let data_reference_index = reader.read_u16::<BigEndian>()?;

        reader.read_u32::<BigEndian>()?; // pre-defined, reserved
        reader.read_u64::<BigEndian>()?; // pre-defined
        reader.read_u32::<BigEndian>()?; // pre-defined
        let width = reader.read_u16::<BigEndian>()?;
        let height = reader.read_u16::<BigEndian>()?;
        let horizontal_resolution = FixedU16::from_raw(reader.read_u32::<BigEndian>()?);
        let vertical_resolution = FixedU16::from_raw(reader.read_u32::<BigEndian>()?);
        reader.read_u32::<BigEndian>()?; // reserved
        let frame_count = reader.read_u16::<BigEndian>()?;
        atom::read_skip_bytes(reader, 32)?; // compressorname
        let depth = reader.read_u16::<BigEndian>()?;
        reader.read_i16::<BigEndian>()?; // pre-defined

        let end = start + size;
        loop {
            let current = reader.stream_position()?;
            if current >= end {
                return Err(Error::AtomNotFound(atom::constant_name::AVCC));
            }
            let header = atom::read_header(reader)?;
            let atom::Header { name, size: s } = header;
            if s > size {
                return Err(Error::InvalidData(
                    "avc1 box contains a box with a larger size than it",
                ));
            }
            if name == atom::constant_name::AVCC {
                let avcc = AvcC::read(reader, s)?;

                atom::read_skip_bytes_to(reader, start + size)?;

                return Ok(Avc1 {
                    data_reference_index,
                    width,
                    height,
                    horizontal_resolution,
                    vertical_resolution,
                    frame_count,
                    depth,
                    avcc,
                });
            } else {
                atom::read_skip_bytes_to(reader, current + s)?;
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvcC {
    pub configuration_version: u8,
    pub avc_profile_indication: u8,
    pub profile_compatibility: u8,
    pub avc_level_indication: u8,
    pub length_size_minus_one: u8,
    pub sequence_parameter_sets: Vec<Nalu>,
    pub picture_parameter_sets: Vec<Nalu>,
}

impl AvcC {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        let configuration_version = reader.read_u8()?;
        let avc_profile_indication = reader.read_u8()?;
        let profile_compatibility = reader.read_u8()?;
        let avc_level_indication = reader.read_u8()?;
        let length_size_minus_one = reader.read_u8()? & 0x3;

        let num_of_spss = reader.read_u8()? & 0x1F;
        let mut sequence_parameter_sets = Vec::with_capacity(num_of_spss as usize);
        for _ in 0..num_of_spss {
            let nalu = atom::read_nalu(reader)?;
            sequence_parameter_sets.push(nalu);
        }

        let num_of_ppss = reader.read_u8()?;
        let mut picture_parameter_sets = Vec::with_capacity(num_of_ppss as usize);
        for _ in 0..num_of_ppss {
            let nalu = atom::read_nalu(reader)?;
            picture_parameter_sets.push(nalu);
        }

        atom::read_skip_bytes_to(reader, start + size)?;

        Ok(AvcC {
            configuration_version,
            avc_profile_indication,
            profile_compatibility,
            avc_level_indication,
            length_size_minus_one,
            sequence_parameter_sets,
            picture_parameter_sets,
        })
    }
}
