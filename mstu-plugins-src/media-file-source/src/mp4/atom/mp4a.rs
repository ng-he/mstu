use std::io::{Read, Seek};

use byteorder::{BigEndian, ReadBytesExt};

use crate::{
    mp4::{
        Result,
        atom::{self},
        error::Error,
    },
    types::FixedU16,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mp4a {
    pub data_reference_index: u16,
    pub channel_count: u16,
    pub sample_size: u16,
    pub sample_rate: FixedU16,
    pub esds: Esds,
}

impl Mp4a {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        reader.read_u32::<BigEndian>()?; // reserved
        reader.read_u16::<BigEndian>()?; // reserved
        let data_reference_index = reader.read_u16::<BigEndian>()?;
        let version = reader.read_u16::<BigEndian>()?;
        reader.read_u16::<BigEndian>()?; // reserved
        reader.read_u32::<BigEndian>()?; // reserved
        let channel_count = reader.read_u16::<BigEndian>()?;
        let sample_size = reader.read_u16::<BigEndian>()?;
        reader.read_u32::<BigEndian>()?; // pre-defined, reserved
        let sample_rate = FixedU16::from_raw(reader.read_u32::<BigEndian>()?);

        if version == 1 {
            // Skip QTFF
            reader.read_u64::<BigEndian>()?;
            reader.read_u64::<BigEndian>()?;
        }

        // Find esds in mp4a or wave
        let mut esds = None;
        let end = start + size;
        loop {
            let current = reader.stream_position()?;
            if current >= end {
                break;
            }
            let header = atom::read_header(reader)?;
            let atom::Header { name, size: s } = header;
            if s > size {
                return Err(Error::InvalidData(
                    "mp4a box contains a box with a larger size than it",
                ));
            }
            if name == atom::constant_name::ESDS {
                esds = Some(Esds::read(reader, s)?);
                break;
            } else if name == atom::constant_name::WAVE {
                // Typically contains frma, mp4a, esds, and a terminator atom
            } else {
                // Skip boxes
                let skip_to = current + s;
                atom::read_skip_bytes_to(reader, skip_to)?;
            }
        }

        if esds.is_none() {
            return Err(Error::AtomNotFound(atom::constant_name::ESDS));
        }

        atom::read_skip_bytes_to(reader, end)?;

        Ok(Mp4a {
            data_reference_index,
            channel_count,
            sample_size,
            sample_rate,
            esds: esds.unwrap(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Esds {
    pub version: u8,
    pub flags: u32,
    pub es_desc: EsDescriptor,
}

impl Esds {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        let full_header = atom::read_full_header(reader)?;

        let mut es_desc = None;

        let mut current = reader.stream_position()?;
        let end = start + size;
        while current < end {
            let (desc_tag, desc_size) = read_desc(reader)?;
            match desc_tag {
                0x03 => {
                    es_desc = Some(EsDescriptor::read(reader, desc_size)?);
                }
                _ => break,
            }
            current = reader.stream_position()?;
        }

        if es_desc.is_none() {
            return Err(Error::InvalidData("ESDescriptor not found"));
        }

        atom::read_skip_bytes_to(reader, start + size)?;

        Ok(Esds {
            version: full_header.version,
            flags: full_header.flags,
            es_desc: es_desc.unwrap(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EsDescriptor {
    pub es_id: u16,

    pub dec_config: DecoderConfigDescriptor,
    pub sl_config: SlConfigDescriptor,
}

impl EsDescriptor {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u32) -> Result<Self> {
        let start = reader.stream_position()?;

        let es_id = reader.read_u16::<BigEndian>()?;
        reader.read_u8()?; // XXX flags must be 0

        let mut dec_config = None;
        let mut sl_config = None;

        let mut current = reader.stream_position()?;
        let end = start + size as u64;
        while current < end {
            let (desc_tag, desc_size) = read_desc(reader)?;
            match desc_tag {
                0x04 => {
                    dec_config = Some(DecoderConfigDescriptor::read(reader, desc_size)?);
                }
                0x06 => {
                    sl_config = Some(SlConfigDescriptor::read(reader, desc_size)?);
                }
                _ => {
                    atom::read_skip_bytes(reader, desc_size as u64)?;
                }
            }
            current = reader.stream_position()?;
        }

        if dec_config.is_none() {
            return Err(Error::InvalidData("decoder config descriptor not found"));
        }

        if sl_config.is_none() {
            return Err(Error::InvalidData("sl config descriptor not found"));
        }

        Ok(EsDescriptor {
            es_id,
            dec_config: dec_config.unwrap(),
            sl_config: sl_config.unwrap(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecoderConfigDescriptor {
    pub object_type_indication: u8,
    pub stream_type: u8,
    pub up_stream: u8,
    pub buffer_size_db: u32,
    pub max_bitrate: u32,
    pub avg_bitrate: u32,

    pub dec_specific: DecoderSpecificDescriptor,
}

impl DecoderConfigDescriptor {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u32) -> Result<Self> {
        let start = reader.stream_position()?;

        let object_type_indication = reader.read_u8()?;
        let byte_a = reader.read_u8()?;
        let stream_type = (byte_a & 0xFC) >> 2;
        let up_stream = byte_a & 0x02;
        let buffer_size_db = reader.read_u24::<BigEndian>()?;
        let max_bitrate = reader.read_u32::<BigEndian>()?;
        let avg_bitrate = reader.read_u32::<BigEndian>()?;

        let mut dec_specific = None;

        let mut current = reader.stream_position()?;
        let end = start + size as u64;
        while current < end {
            let (desc_tag, desc_size) = read_desc(reader)?;
            match desc_tag {
                0x05 => {
                    dec_specific = Some(DecoderSpecificDescriptor::read(reader, desc_size)?);
                }
                _ => {
                    atom::read_skip_bytes(reader, desc_size as u64)?;
                }
            }
            current = reader.stream_position()?;
        }

        if dec_specific.is_none() {
            return Err(Error::InvalidData("decoder specific descriptor not found"));
        }

        Ok(DecoderConfigDescriptor {
            object_type_indication,
            stream_type,
            up_stream,
            buffer_size_db,
            max_bitrate,
            avg_bitrate,
            dec_specific: dec_specific.unwrap(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecoderSpecificDescriptor {
    pub profile: u8,
    pub freq_index: u8,
    pub chan_conf: u8,
}

impl DecoderSpecificDescriptor {
    pub fn read<R: Read + Seek>(reader: &mut R, _size: u32) -> Result<Self> {
        let byte_a = reader.read_u8()?;
        let byte_b = reader.read_u8()?;
        let profile = Self::get_audio_object_type(byte_a, byte_b);
        let freq_index;
        let chan_conf;
        if profile > 31 {
            freq_index = (byte_b >> 1) & 0x0F;
            chan_conf = Self::get_chan_conf(reader, byte_b, freq_index, true)?;
        } else {
            freq_index = ((byte_a & 0x07) << 1) + (byte_b >> 7);
            chan_conf = Self::get_chan_conf(reader, byte_b, freq_index, false)?;
        }

        Ok(DecoderSpecificDescriptor {
            profile,
            freq_index,
            chan_conf,
        })
    }

    fn get_audio_object_type(byte_a: u8, byte_b: u8) -> u8 {
        let mut profile = byte_a >> 3;
        if profile == 31 {
            profile = 32 + ((byte_a & 7) | (byte_b >> 5));
        }

        profile
    }

    fn get_chan_conf<R: Read + Seek>(
        reader: &mut R,
        byte_b: u8,
        freq_index: u8,
        extended_profile: bool,
    ) -> Result<u8> {
        let chan_conf;
        if freq_index == 15 {
            // Skip the 24 bit sample rate
            let sample_rate = reader.read_u24::<BigEndian>()?;
            chan_conf = ((sample_rate >> 4) & 0x0F) as u8;
        } else if extended_profile {
            let byte_c = reader.read_u8()?;
            chan_conf = (byte_b & 1) | (byte_c & 0xE0);
        } else {
            chan_conf = (byte_b >> 3) & 0x0F;
        }

        Ok(chan_conf)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlConfigDescriptor {}

impl SlConfigDescriptor {
    pub fn read<R: Read + Seek>(reader: &mut R, _size: u32) -> Result<Self> {
        reader.read_u8()?; // pre-defined

        Ok(SlConfigDescriptor {})
    }
}

fn read_desc<R: Read>(reader: &mut R) -> Result<(u8, u32)> {
    let tag = reader.read_u8()?;

    let mut size: u32 = 0;
    for _ in 0..4 {
        let b = reader.read_u8()?;
        size = (size << 7) | (b & 0x7F) as u32;
        if b & 0x80 == 0 {
            break;
        }
    }

    Ok((tag, size))
}
