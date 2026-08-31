use crate::media::FourCC;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    InvalidData(&'static str),
    #[error("{0} atom not found")]
    AtomNotFound(FourCC),
    #[error("{0}, {1} atoms not found")]
    Atom2NotFound(FourCC, FourCC),
    #[error("{0} version {1} is not supported")]
    UnsupportedAtomVersion(FourCC, u8),
    #[error("trak[{0}] not found")]
    TrakNotFound(u32),
    #[error("trak[{0}].{1} not found")]
    AtomInTrakNotFound(u32, FourCC),
    #[error("trak[{0}].stbl.{1} not found")]
    AtomInStblNotFound(u32, FourCC),
    #[error("trak[{0}].stbl.{1}.entry[{2}] not found")]
    EntryInStblNotFound(u32, FourCC, u32),
}
