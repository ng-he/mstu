use crate::codec::{
    Codec::{AAC, H264, H265, TTXT, VP9},
    MediaType::{Audio, Subtitle, Video},
    audio::aac,
    subtitle::ttxt,
    video::{h264, h265, vp9},
};

pub mod audio;
pub mod subtitle;
pub mod video;

pub enum Codec {
    // Video
    H264(h264::Parameters),
    H265(h265::Parameters),
    VP9(vp9::Parameters),

    // Audio
    AAC(aac::Parameters),

    // Subtitle
    TTXT(ttxt::Parameters),
}

pub enum MediaType {
    Video,
    Audio,
    Subtitle,
}

impl Codec {
    pub fn media_type(&self) -> MediaType {
        match self {
            H264(_) => Video,
            H265(_) => Video,
            VP9(_) => Video,

            AAC(_) => Audio,

            TTXT(_) => Subtitle,
        }
    }
}
