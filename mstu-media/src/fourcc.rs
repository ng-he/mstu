use core::fmt;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid fourcc length")]
    InvalidLength,
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FourCC([u8; 4]);

impl FourCC {
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).unwrap_or("????")
    }

    pub const fn from(value: [u8; 4]) -> Self {
        Self([value[0], value[1], value[2], value[3]])
    }
}

impl TryFrom<&[u8]> for FourCC {
    type Error = Error;

    fn try_from(value: &[u8]) -> Result<Self> {
        if value.len() != 4 {
            return Err(Error::InvalidLength);
        }

        Ok(Self([value[0], value[1], value[2], value[3]]))
    }
}

impl fmt::Display for FourCC {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())?;
        Ok(())
    }
}
