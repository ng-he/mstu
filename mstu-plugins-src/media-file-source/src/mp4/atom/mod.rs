use std::io::{Read, Seek, SeekFrom};

use crate::media::{FourCC, nalu::Nalu};
use crate::mp4::{Result, error::Error};
use byteorder::{BigEndian, ReadBytesExt};

pub mod avc1;
pub mod co64;
pub mod ctts;
pub mod data;
pub mod dinf;
pub mod edts;
pub mod elst;
pub mod ftyp;
pub mod hdlr;
pub mod hev1;
pub mod ilst;
pub mod mdhd;
pub mod mdia;
pub mod mehd;
pub mod meta;
pub mod minf;
pub mod moov;
pub mod mp4a;
pub mod mvex;
pub mod mvhd;
pub mod smhd;
pub mod stbl;
pub mod stco;
pub mod stsc;
pub mod stsd;
pub mod stss;
pub mod stsz;
pub mod stts;
pub mod tkhd;
pub mod trak;
pub mod trex;
pub mod tx3g;
pub mod udta;
pub mod vmhd;
pub mod vp09;

pub use avc1::Avc1;
pub use co64::Co64;
pub use ctts::Ctts;
pub use data::Data;
pub use dinf::Dinf;
pub use edts::Edts;
pub use elst::Elst;
pub use ftyp::Ftyp;
pub use hdlr::Hdlr;
pub use hev1::Hev1;
pub use ilst::Ilst;
pub use mdhd::Mdhd;
pub use mdia::Mdia;
pub use mehd::Mehd;
pub use meta::Meta;
pub use minf::Minf;
pub use moov::Moov;
pub use mp4a::Mp4a;
pub use mvex::Mvex;
pub use mvhd::Mvhd;
pub use smhd::Smhd;
pub use stbl::Stbl;
pub use stco::Stco;
pub use stsc::Stsc;
pub use stsd::Stsd;
pub use stss::Stss;
pub use stsz::Stsz;
pub use stts::Stts;
pub use tkhd::Tkhd;
pub use trak::Trak;
pub use trex::Trex;
pub use tx3g::Tx3g;
pub use udta::Udta;
pub use vmhd::Vmhd;
pub use vp09::Vp09;

pub const HEADER_SIZE: u64 = 8;
pub const HEADER_EXT_SIZE: u64 = 4;

pub mod constant_name {
    use crate::media::FourCC;

    // Root / generic
    pub const FTYP: FourCC = FourCC::from(*b"ftyp");
    pub const FREE: FourCC = FourCC::from(*b"free");
    pub const MDAT: FourCC = FourCC::from(*b"mdat");
    pub const EMSG: FourCC = FourCC::from(*b"emsg");

    // Container atoms
    pub const MOOV: FourCC = FourCC::from(*b"moov");
    pub const TRAK: FourCC = FourCC::from(*b"trak");
    pub const MDIA: FourCC = FourCC::from(*b"mdia");
    pub const MINF: FourCC = FourCC::from(*b"minf");
    pub const STBL: FourCC = FourCC::from(*b"stbl");
    pub const DINF: FourCC = FourCC::from(*b"dinf");
    pub const EDTS: FourCC = FourCC::from(*b"edts");
    pub const MOOF: FourCC = FourCC::from(*b"moof");
    pub const TRAF: FourCC = FourCC::from(*b"traf");
    pub const MVEX: FourCC = FourCC::from(*b"mvex");
    pub const UDTA: FourCC = FourCC::from(*b"udta");
    pub const META: FourCC = FourCC::from(*b"meta");
    pub const ILST: FourCC = FourCC::from(*b"ilst");

    // Media header atoms
    pub const MVHD: FourCC = FourCC::from(*b"mvhd");
    pub const TKHD: FourCC = FourCC::from(*b"tkhd");
    pub const MDHD: FourCC = FourCC::from(*b"mdhd");
    pub const HDLR: FourCC = FourCC::from(*b"hdlr");
    pub const VMHD: FourCC = FourCC::from(*b"vmhd");
    pub const SMHD: FourCC = FourCC::from(*b"smhd");
    pub const MDIR: FourCC = FourCC::from(*b"mdir");

    // Sample table atoms
    pub const STSD: FourCC = FourCC::from(*b"stsd");
    pub const STTS: FourCC = FourCC::from(*b"stts");
    pub const STSC: FourCC = FourCC::from(*b"stsc");
    pub const STSZ: FourCC = FourCC::from(*b"stsz");
    pub const STSS: FourCC = FourCC::from(*b"stss");
    pub const STCO: FourCC = FourCC::from(*b"stco");
    pub const CO64: FourCC = FourCC::from(*b"co64");
    pub const CTTS: FourCC = FourCC::from(*b"ctts");

    // Data reference atoms
    pub const DREF: FourCC = FourCC::from(*b"dref");
    pub const URL_: FourCC = FourCC::from(*b"url ");

    // Edit list atoms
    pub const ELST: FourCC = FourCC::from(*b"elst");

    // Fragmented MP4 atoms
    pub const MFHD: FourCC = FourCC::from(*b"mfhd");
    pub const TFHD: FourCC = FourCC::from(*b"tfhd");
    pub const TFDT: FourCC = FourCC::from(*b"tfdt");
    pub const TRUN: FourCC = FourCC::from(*b"trun");
    pub const TREX: FourCC = FourCC::from(*b"trex");
    pub const MEHD: FourCC = FourCC::from(*b"mehd");

    // Metadata atoms
    pub const DATA: FourCC = FourCC::from(*b"data");
    pub const NAME: FourCC = FourCC::from([0xA9, b'n', b'a', b'm']); // ©nam
    pub const DAY: FourCC = FourCC::from([0xA9, b'd', b'a', b'y']); // ©day
    pub const COVR: FourCC = FourCC::from(*b"covr");
    pub const DESC: FourCC = FourCC::from(*b"desc");

    // Sample entry / codec atoms
    pub const AVC1: FourCC = FourCC::from(*b"avc1");
    pub const HEV1: FourCC = FourCC::from(*b"hev1");
    pub const MP4A: FourCC = FourCC::from(*b"mp4a");
    pub const TX3G: FourCC = FourCC::from(*b"tx3g");
    pub const AVCC: FourCC = FourCC::from(*b"avcC");
    pub const HVCC: FourCC = FourCC::from(*b"hvcC");
    pub const ESDS: FourCC = FourCC::from(*b"esds");
    pub const VPCC: FourCC = FourCC::from(*b"vpcC");
    pub const VP09: FourCC = FourCC::from(*b"vp09");

    // Misc/container-ish
    pub const WIDE: FourCC = FourCC::from(*b"wide");
    pub const WAVE: FourCC = FourCC::from(*b"wave");
}

#[derive(Debug, Clone, Copy)]
pub struct Header {
    pub name: FourCC,
    pub size: u64,
}

impl Header {
    pub fn new(name: FourCC, size: u64) -> Self {
        Self { name, size }
    }
}

pub fn read_header<R: Read>(reader: &mut R) -> Result<Header> {
    // Create and read to buf.
    let mut buf = [0u8; 8]; // 8 bytes for atom header.
    reader.read_exact(&mut buf)?;

    // Get size.
    let s = buf[0..4].try_into().unwrap();
    let size = u32::from_be_bytes(s);

    // Get atom type string.
    let t = buf[4..8].try_into().unwrap();
    let typ = u32::from_be_bytes(t);

    // Get largesize if size is 1
    if size == 1 {
        reader.read_exact(&mut buf)?;
        let largesize = u64::from_be_bytes(buf);

        Ok(Header {
            name: FourCC::from(typ.to_be_bytes()),

            // Subtract the length of the serialized largesize, as callers assume `size - HEADER_SIZE` is the length
            // of the box data. Disallow `largesize < 16`, or else a largesize of 8 will result in a Header::size
            // of 0, incorrectly indicating that the box data extends to the end of the stream.
            size: match largesize {
                0 => 0,
                1..=15 => return Err(Error::InvalidData("64-bit box size too small")),
                16..=u64::MAX => largesize - 8,
            },
        })
    } else {
        Ok(Header {
            name: FourCC::from(typ.to_be_bytes()),
            size: size as u64,
        })
    }
}

pub struct FullHeader {
    pub version: u8,
    pub flags: u32,
}

pub fn read_full_header<R: Read>(reader: &mut R) -> Result<FullHeader> {
    let version = reader.read_u8()?;

    let mut flags_buf = [0u8; 4];
    reader.read_exact(&mut flags_buf[1..])?;

    let flags = u32::from_be_bytes(flags_buf);

    Ok(FullHeader { version, flags })

    // let version = reader.read_u8()?;
    // let flags = reader.read_u32::<BigEndian>()?;
    // Ok(FullHeader { version, flags })
}

pub fn read_position<R: Seek>(seeker: &mut R) -> Result<u64> {
    Ok(seeker.stream_position()? - HEADER_SIZE)
}

pub fn read_skip<S: Seek>(seeker: &mut S, size: u64) -> Result<()> {
    let start = seeker.stream_position()? - HEADER_SIZE;
    seeker.seek(SeekFrom::Start(start + size))?;
    Ok(())
}

pub fn read_skip_bytes<S: Seek>(seeker: &mut S, size: u64) -> Result<()> {
    seeker.seek(SeekFrom::Current(size as i64))?;
    Ok(())
}

pub fn read_skip_bytes_to<S: Seek>(seeker: &mut S, pos: u64) -> Result<()> {
    seeker.seek(SeekFrom::Start(pos))?;
    Ok(())
}

pub fn read_nalu<R: Read + Seek>(reader: &mut R) -> Result<Nalu> {
    let length = reader.read_u16::<BigEndian>()? as usize;
    let mut bytes = vec![0u8; length];
    reader.read_exact(&mut bytes)?;
    Ok(Nalu::from(bytes.as_slice()))
}
