use std::io::Seek;

use byteorder::{BigEndian, ReadBytesExt};
use mstu_media::codec::video::h265;

use crate::media::nalu::Nalu;
use crate::mp4::*;
use crate::types::FixedU16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hev1 {
    pub data_reference_index: u16,
    pub width: u16,
    pub height: u16,
    pub horizontal_resolution: FixedU16,
    pub vertical_resolution: FixedU16,
    pub frame_count: u16,
    pub depth: u16,
    pub hvcc: HvcC,
}

impl Hev1 {
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

        let header = atom::read_header(reader)?;
        let atom::Header { name, size: s } = header;
        if s > size {
            return Err(Error::InvalidData(
                "hev1 box contains a box with a larger size than it",
            ));
        }
        if name == atom::constant_name::HVCC {
            let hvcc = HvcC::read(reader, s)?;

            atom::read_skip_bytes_to(reader, start + size)?;

            Ok(Hev1 {
                data_reference_index,
                width,
                height,
                horizontal_resolution,
                vertical_resolution,
                frame_count,
                depth,
                hvcc,
            })
        } else {
            Err(Error::AtomNotFound(atom::constant_name::HVCC))
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct HvcC {
    pub configuration_version: u8,
    pub general_profile_space: u8,
    pub general_tier_flag: bool,
    pub general_profile_idc: u8,
    pub general_profile_compatibility_flags: u32,
    pub general_constraint_indicator_flag: u64,
    pub general_level_idc: u8,
    pub min_spatial_segmentation_idc: u16,
    pub parallelism_type: u8,
    pub chroma_format_idc: u8,
    pub bit_depth_luma_minus8: u8,
    pub bit_depth_chroma_minus8: u8,
    pub avg_frame_rate: u16,
    pub constant_frame_rate: u8,
    pub num_temporal_layers: u8,
    pub temporal_id_nested: bool,
    pub length_size_minus_one: u8,
    pub arrays: Vec<HvcCArray>,
}

impl HvcC {
    pub fn read<R: Read + Seek>(reader: &mut R, _size: u64) -> Result<Self> {
        let configuration_version = reader.read_u8()?;
        let params = reader.read_u8()?;
        let general_profile_space = params & 0b11000000 >> 6;
        let general_tier_flag = (params & 0b00100000 >> 5) > 0;
        let general_profile_idc = params & 0b00011111;

        let general_profile_compatibility_flags = reader.read_u32::<BigEndian>()?;
        let general_constraint_indicator_flag = reader.read_u48::<BigEndian>()?;
        let general_level_idc = reader.read_u8()?;
        let min_spatial_segmentation_idc = reader.read_u16::<BigEndian>()? & 0x0FFF;
        let parallelism_type = reader.read_u8()? & 0b11;
        let chroma_format_idc = reader.read_u8()? & 0b11;
        let bit_depth_luma_minus8 = reader.read_u8()? & 0b111;
        let bit_depth_chroma_minus8 = reader.read_u8()? & 0b111;
        let avg_frame_rate = reader.read_u16::<BigEndian>()?;

        let params = reader.read_u8()?;
        let constant_frame_rate = params & 0b11000000 >> 6;
        let num_temporal_layers = params & 0b00111000 >> 3;
        let temporal_id_nested = (params & 0b00000100 >> 2) > 0;
        let length_size_minus_one = params & 0b000011;

        let num_of_arrays = reader.read_u8()?;

        let mut arrays = Vec::with_capacity(num_of_arrays as _);
        for _ in 0..num_of_arrays {
            let params = reader.read_u8()?;
            let num_nalus = reader.read_u16::<BigEndian>()?;
            let mut nalus = Vec::with_capacity(num_nalus as usize);

            for _ in 0..num_nalus {
                let size = reader.read_u16::<BigEndian>()?;
                let mut data = vec![0; size as usize];

                reader.read_exact(&mut data)?;

                nalus.push(Nalu::from(data.as_slice()))
            }

            arrays.push(HvcCArray {
                completeness: (params & 0b10000000) > 0,
                nalu_type: h265::NaluType::from(params & 0b111111),
                nalus,
            });
        }

        Ok(HvcC {
            configuration_version,
            general_profile_space,
            general_tier_flag,
            general_profile_idc,
            general_profile_compatibility_flags,
            general_constraint_indicator_flag,
            general_level_idc,
            min_spatial_segmentation_idc,
            parallelism_type,
            chroma_format_idc,
            bit_depth_luma_minus8,
            bit_depth_chroma_minus8,
            avg_frame_rate,
            constant_frame_rate,
            num_temporal_layers,
            temporal_id_nested,
            length_size_minus_one,
            arrays,
        })
    }

    pub fn nalus(&self, t: h265::NaluType) -> Vec<Nalu> {
        self.arrays
            .iter()
            .filter(|arr| arr.nalu_type == t)
            .flat_map(|arr| arr.nalus.clone())
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HvcCArray {
    pub completeness: bool,
    pub nalu_type: h265::NaluType,
    pub nalus: Vec<Nalu>,
}
