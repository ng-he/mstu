use std::{
    fmt,
    io::{Read, Seek},
};

use crate::mp4::{
    Result,
    atom::{self},
    error::Error,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moov {
    pub mvhd: atom::Mvhd,
    pub meta: Option<atom::Meta>,
    pub mvex: Option<atom::Mvex>,
    pub traks: Vec<atom::Trak>,
    pub udta: Option<atom::Udta>,
}

impl Moov {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        let mut mvhd = None;
        let mut meta = None;
        let mut udta = None;
        let mut mvex = None;
        let mut traks = Vec::new();

        let mut current = reader.stream_position()?;
        let end = start + size;
        while current < end {
            let header = atom::read_header(reader)?;
            let atom::Header { name, size: s } = header;
            if s > size {
                return Err(Error::InvalidData(
                    "moov box contains a box with a larger size than it",
                ));
            }

            match name {
                atom::constant_name::MVHD => {
                    mvhd = Some(atom::Mvhd::read(reader, s)?);
                }
                atom::constant_name::META => {
                    meta = Some(atom::Meta::read(reader, s)?);
                }
                atom::constant_name::MVEX => {
                    mvex = Some(atom::Mvex::read(reader, s)?);
                }
                atom::constant_name::TRAK => {
                    let trak = atom::Trak::read(reader, s)?;
                    traks.push(trak);
                }
                atom::constant_name::UDTA => {
                    udta = Some(atom::Udta::read(reader, s)?);
                }
                _ => {
                    // XXX warn!()
                    atom::read_skip(reader, s)?;
                }
            }

            current = reader.stream_position()?;
        }

        if mvhd.is_none() {
            return Err(Error::AtomNotFound(atom::constant_name::MVHD));
        }

        atom::read_skip_bytes_to(reader, start + size)?;
        Ok(Moov {
            mvhd: mvhd.unwrap(),
            meta: meta,
            mvex: mvex,
            udta: udta,
            traks: traks,
        })
    }
}

impl fmt::Display for Moov {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Movie Box (moov)")?;
        writeln!(f, "  Version: {}", self.mvhd.version)?;
        writeln!(f, "  Timescale: {}", self.mvhd.timescale)?;
        writeln!(f, "  Duration: {}", self.mvhd.duration)?;
        writeln!(f, "  Rate: {:.2}", self.mvhd.rate.as_f32())?;
        writeln!(f, "  Volume: {:.2}", self.mvhd.volume.as_f32())?;
        writeln!(f, "  Next Track ID: {}", self.mvhd.next_track_id)?;
        writeln!(f, "  Matrix: {}", self.mvhd.matrix)?;

        if let Some(meta) = &self.meta {
            writeln!(f, "  Metadata:")?;
            match meta {
                atom::Meta::Mdir { ilst } => {
                    writeln!(f, "    Handler: mdir")?;

                    if let Some(ilst) = ilst {
                        for (key, item) in &ilst.items {
                            writeln!(f, "    - {:?}: {:?}", key, item)?;
                        }
                    }
                }

                atom::Meta::Unknown { hdlr, data } => {
                    writeln!(f, "    Handler: {}", hdlr.handler_type)?;
                    writeln!(f, "    Name: {}", hdlr.name)?;

                    for (kind, payload) in data {
                        writeln!(f, "    - {} ({} bytes)", kind, payload.len())?;
                    }
                }
            }
        }

        Ok(())
    }
}
