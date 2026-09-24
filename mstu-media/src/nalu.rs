use bytes::Bytes;

pub const START_CODE: [u8; 4] = [0, 0, 0, 1];

/// Rewrites a 4-byte-length-prefixed sample into Annex-B where it lies.
///
/// Same size either way, so this needs no buffer of its own. False on a
/// malformed sample.
pub fn avcc_to_annexb_in_place(data: &mut [u8]) -> bool {
    let mut offset = 0;

    while offset < data.len() {
        let end = offset + START_CODE.len();

        if end > data.len() {
            return false;
        }

        let Ok(prefix) = data[offset..end].try_into() else {
            return false;
        };

        let size = u32::from_be_bytes(prefix) as usize;

        if end + size > data.len() {
            return false;
        }

        data[offset..end].copy_from_slice(&START_CODE);
        offset = end + size;
    }

    true
}

/// Converts AVCC (length prefixed) to Annex-B (start code prefixed).
///
/// `length_size` 4 is rewritten in place, shorter prefixes need a copy.
/// Returns None on a malformed sample.
pub fn avcc_to_annexb(mut data: Vec<u8>, length_size: usize) -> Option<Vec<u8>> {
    if length_size == 0 || length_size > 4 {
        return None;
    }

    if length_size == START_CODE.len() {
        let mut offset = 0;

        while offset < data.len() {
            let end = offset + START_CODE.len();
            if end > data.len() {
                return None;
            }

            let size = u32::from_be_bytes(data[offset..end].try_into().ok()?) as usize;
            if end + size > data.len() {
                return None;
            }

            data[offset..end].copy_from_slice(&START_CODE);
            offset = end + size;
        }

        return Some(data);
    }

    let mut annexb = Vec::with_capacity(data.len());
    let mut offset = 0;

    while offset < data.len() {
        let end = offset + length_size;
        if end > data.len() {
            return None;
        }

        let size = data[offset..end]
            .iter()
            .fold(0usize, |size, byte| (size << 8) | *byte as usize);

        if end + size > data.len() {
            return None;
        }

        annexb.extend_from_slice(&START_CODE);
        annexb.extend_from_slice(&data[end..end + size]);

        offset = end + size;
    }

    Some(annexb)
}

/// Raw Network Abstraction Layer Unit (NALU).
/// This type stores only the raw NAL payload itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nalu(Bytes);

impl From<Vec<u8>> for Nalu {
    fn from(bytes: Vec<u8>) -> Self {
        Self(Bytes::from(bytes))
    }
}

impl From<&[u8]> for Nalu {
    fn from(bytes: &[u8]) -> Self {
        Self(Bytes::copy_from_slice(bytes))
    }
}

impl From<Bytes> for Nalu {
    fn from(bytes: Bytes) -> Self {
        Self(bytes)
    }
}

impl Nalu {
    pub fn size(&self) -> usize {
        self.0.len()
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn into_bytes(self) -> Bytes {
        self.0
    }

    pub fn from_annexb(data: &[u8]) -> Vec<Self> {
        let mut result = Vec::new();
        let mut start = None;
        let mut i = 0;

        while i < data.len() {
            let sc_len = if data[i..].starts_with(&[0, 0, 0, 1]) {
                Some(4)
            } else if data[i..].starts_with(&[0, 0, 1]) {
                Some(3)
            } else {
                None
            };

            if let Some(len) = sc_len {
                if let Some(s) = start {
                    if s < i {
                        result.push(Self::from(&data[s..i]));
                    }
                }

                start = Some(i + len);
                i += len;
            } else {
                i += 1;
            }
        }

        if let Some(s) = start {
            if s < data.len() {
                result.push(Self::from(&data[s..]));
            }
        }

        result
    }
}
