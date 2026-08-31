/// Unsigned 16.16 fixed-point number.
///
/// Layout:
/// ```text
/// [ integer:16 ][ fraction:16 ]
/// ```
///
/// Example:
/// ```text
/// 0x00010000 = 1.0
/// 0x00008000 = 0.5
/// 0x00018000 = 1.5
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedU16(u32);

impl FixedU16 {
    /// Initialize from raw `u32`.
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    /// Converts the fixed-point value into `f32`.
    pub fn as_f32(self) -> f32 {
        self.0 as f32 / 65536.0
    }

    /// Returns the integer component.
    pub fn integer(self) -> u16 {
        (self.0 >> 16) as u16
    }

    /// Returns the fractional component.
    ///
    /// This is the raw lower 16-bit fraction,
    /// not a normalized floating-point value.
    pub fn fraction(self) -> u16 {
        (self.0 & 0xFFFF) as u16
    }

    /// Returns the raw underlying representation.
    pub fn raw(self) -> u32 {
        self.0
    }
}

/// Unsigned 8.8 fixed-point number.
///
/// Layout:
/// ```text
/// [ integer:8 ][ fraction:8 ]
/// ```
///
/// Example:
/// ```text
/// 0x0100 = 1.0
/// 0x0080 = 0.5
/// 0x0180 = 1.5
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedU8(u16);

impl FixedU8 {
    /// Initialize from raw `u16`.
    pub const fn from_raw(raw: u16) -> Self {
        Self(raw)
    }

    /// Converts the fixed-point value into `f32`.
    pub fn as_f32(self) -> f32 {
        self.0 as f32 / 256.0
    }

    /// Returns the integer component.
    pub fn integer(self) -> u8 {
        (self.0 >> 8) as u8
    }

    /// Returns the fractional component.
    ///
    /// This is the raw lower 8-bit fraction,
    /// not a normalized floating-point value.
    pub fn fraction(self) -> u8 {
        (self.0 & 0xFF) as u8
    }

    /// Returns the raw underlying representation.
    pub fn raw(self) -> u16 {
        self.0
    }
}

/// Signed 8.8 fixed-point number.
///
/// Layout:
/// ```text
/// [ integer:8 ][ fraction:8 ]
/// ```
///
/// Example:
/// ```text
/// 0x0100 = 1.0
/// 0xFF00 = -1.0
/// 0x0080 = 0.5
/// 0xFF80 = -0.5
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedI8(i16);

impl FixedI8 {
    /// Initialize from raw `i16`.
    pub const fn from_raw(raw: i16) -> Self {
        Self(raw)
    }

    /// Converts the fixed-point value into `f32`.
    pub fn as_f32(self) -> f32 {
        self.0 as f32 / 256.0
    }

    /// Returns the integer component.
    pub fn integer(self) -> i8 {
        (self.0 >> 8) as i8
    }

    /// Returns the fractional component.
    ///
    /// This is the raw lower 8-bit fraction,
    /// not a normalized floating-point value.
    pub fn fraction(self) -> u8 {
        (self.0 & 0x00FF) as u8
    }

    /// Returns the raw underlying representation.
    pub fn raw(self) -> i16 {
        self.0
    }
}

/// Signed 2.30 fixed-point number.
///
/// Layout:
/// ```text
/// [ integer:2 ][ fraction:30 ]
/// ```
///
/// Example:
/// ```text
/// 0x40000000 = 1.0
/// 0x20000000 = 0.5
/// 0x80000000 = -2.0
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fixed2_30(i32);

impl Fixed2_30 {
    /// Initialize from raw `i32`.
    pub const fn from_raw(raw: i32) -> Self {
        Self(raw)
    }

    /// Converts the fixed-point value into `f32`.
    pub fn as_f32(self) -> f32 {
        self.0 as f32 / 1073741824.0
    }

    /// Returns the integer component.
    pub fn integer(self) -> i8 {
        (self.0 >> 30) as i8
    }

    /// Returns the fractional component.
    ///
    /// This is the raw lower 30-bit fraction,
    /// not a normalized floating-point value.
    pub fn fraction(self) -> u32 {
        (self.0 & 0x3FFF_FFFF) as u32
    }

    /// Returns the raw underlying representation.
    pub fn raw(self) -> i32 {
        self.0
    }
}

macro_rules! impl_lower_hex_fmt {
    ($t:ty) => {
        impl std::fmt::LowerHex for $t {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::LowerHex::fmt(&self.0, f)
            }
        }
    };
}

impl_lower_hex_fmt!(FixedU16);
impl_lower_hex_fmt!(FixedU8);
impl_lower_hex_fmt!(Fixed2_30);
impl_lower_hex_fmt!(FixedI8);
