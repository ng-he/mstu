use std::{
    io::{Read, Seek, SeekFrom},
    time::Duration,
    vec,
};

use bytes::Bytes;
use mstu_media::{
    Codec,
    codec::{
        audio::aac,
        subtitle::ttxt,
        video::{h264, h265, vp9},
    },
    nalu,
};

use crate::media::FourCC;
use crate::file::SampleInfo;
use crate::mp4::{
    Result, Sample,
    atom::{self},
    error::Error,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackType {
    Video,
    Audio,
    Subtitle,
}

const HANDLER_TYPE_VIDEO_FOURCC: FourCC = FourCC::from([b'v', b'i', b'd', b'e']);
const HANDLER_TYPE_AUDIO_FOURCC: FourCC = FourCC::from([b's', b'o', b'u', b'n']);
const HANDLER_TYPE_SUBTITLE_FOURCC: FourCC = FourCC::from([b's', b'b', b't', b'l']);

impl TryFrom<&FourCC> for TrackType {
    type Error = Error;
    fn try_from(fourcc: &FourCC) -> Result<TrackType> {
        match *fourcc {
            HANDLER_TYPE_VIDEO_FOURCC => Ok(TrackType::Video),
            HANDLER_TYPE_AUDIO_FOURCC => Ok(TrackType::Audio),
            HANDLER_TYPE_SUBTITLE_FOURCC => Ok(TrackType::Subtitle),
            _ => Err(Error::InvalidData("unsupported handler type")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Track {
    pub trak: atom::Trak,
}

impl Track {
    pub fn from(trak: &atom::Trak) -> Self {
        let trak = trak.clone();
        Self { trak }
    }

    pub fn track_id(&self) -> u32 {
        self.trak.tkhd.track_id
    }

    pub fn track_type(&self) -> Result<TrackType> {
        TrackType::try_from(&self.trak.mdia.hdlr.handler_type)
    }

    /// NALU length prefix size, None for non h26x tracks.
    pub(crate) fn nalu_length_size(&self) -> Option<usize> {
        let stsd = &self.trak.mdia.minf.stbl.stsd;

        if let Some(avc1) = &stsd.avc1 {
            Some(avc1.avcc.length_size_minus_one as usize + 1)
        } else if let Some(hev1) = &stsd.hev1 {
            Some(hev1.hvcc.length_size_minus_one as usize + 1)
        } else {
            None
        }
    }

    pub fn media_format(&self) -> Result<Codec> {
        if let Some(avc1) = &self.trak.mdia.minf.stbl.stsd.avc1 {
            Ok(Codec::H264(h264::Parameters {
                profile: h264::Profile::from((
                    avc1.avcc.avc_profile_indication,
                    avc1.avcc.profile_compatibility,
                )),
                sps: avc1.avcc.sequence_parameter_sets.clone(),
                pps: avc1.avcc.picture_parameter_sets.clone(),
            }))
        } else if let Some(hev1) = &self.trak.mdia.minf.stbl.stsd.hev1 {
            Ok(Codec::H265(h265::Parameters {
                sps: hev1.hvcc.nalus(h265::NaluType::Sps),
                pps: hev1.hvcc.nalus(h265::NaluType::Pps),
                vps: hev1.hvcc.nalus(h265::NaluType::Vps),
            }))
        } else if let Some(vp09) = &self.trak.mdia.minf.stbl.stsd.vp09 {
            Ok(Codec::VP9(vp9::Parameters {
                width: vp09.width,
                height: vp09.height,
            }))
        } else if let Some(mp4a) = &self.trak.mdia.minf.stbl.stsd.mp4a {
            let bitrate = mp4a.esds.es_desc.dec_config.avg_bitrate;
            let profile =
                aac::AudioObjectType::from(mp4a.esds.es_desc.dec_config.dec_specific.profile);
            let freq_index = aac::SamplingFrequencyIndex::from(
                mp4a.esds.es_desc.dec_config.dec_specific.freq_index,
            );
            let channel_config = aac::ChannelConfiguration::from(
                mp4a.esds.es_desc.dec_config.dec_specific.chan_conf,
            );

            Ok(Codec::AAC(aac::Parameters {
                bitrate,
                profile,
                freq_index,
                channel_config,
            }))
        } else if self.trak.mdia.minf.stbl.stsd.tx3g.is_some() {
            Ok(Codec::TTXT(ttxt::Parameters {}))
        } else {
            Err(Error::InvalidData("unsupported media format"))
        }
    }

    pub fn sample_entry(&self) -> Result<FourCC> {
        if self.trak.mdia.minf.stbl.stsd.avc1.is_some() {
            Ok(atom::constant_name::AVC1)
        } else if self.trak.mdia.minf.stbl.stsd.hev1.is_some() {
            Ok(atom::constant_name::HEV1)
        } else if self.trak.mdia.minf.stbl.stsd.vp09.is_some() {
            Ok(atom::constant_name::VP09)
        } else if self.trak.mdia.minf.stbl.stsd.mp4a.is_some() {
            Ok(atom::constant_name::MP4A)
        } else if self.trak.mdia.minf.stbl.stsd.tx3g.is_some() {
            Ok(atom::constant_name::TX3G)
        } else {
            Err(Error::InvalidData("unsupported sample entry atom"))
        }
    }

    pub fn width(&self) -> u16 {
        if let Some(ref avc1) = self.trak.mdia.minf.stbl.stsd.avc1 {
            avc1.width
        } else {
            self.trak.tkhd.width.integer()
        }
    }

    pub fn height(&self) -> u16 {
        if let Some(ref avc1) = self.trak.mdia.minf.stbl.stsd.avc1 {
            avc1.height
        } else {
            self.trak.tkhd.height.integer()
        }
    }

    pub fn frame_rate(&self) -> f64 {
        let dur = self.duration();
        if dur.is_zero() {
            0.0
        } else {
            self.sample_count() as f64 / dur.as_secs_f64()
        }
    }

    pub fn language(&self) -> &str {
        &self.trak.mdia.mdhd.language
    }

    pub fn timescale(&self) -> u32 {
        self.trak.mdia.mdhd.timescale
    }

    pub fn duration(&self) -> Duration {
        Duration::from_micros(
            self.trak.mdia.mdhd.duration * 1_000_000 / self.trak.mdia.mdhd.timescale as u64,
        )
    }

    pub fn sample_freq_index(&self) -> Result<aac::SamplingFrequencyIndex> {
        if let Some(ref mp4a) = self.trak.mdia.minf.stbl.stsd.mp4a {
            Ok(aac::SamplingFrequencyIndex::from(
                mp4a.esds.es_desc.dec_config.dec_specific.freq_index,
            ))
        } else {
            Err(Error::AtomInStblNotFound(
                self.track_id(),
                atom::constant_name::MP4A,
            ))
        }
    }

    pub fn channel_layout(&self) -> Result<aac::ChannelConfiguration> {
        if let Some(ref mp4a) = self.trak.mdia.minf.stbl.stsd.mp4a {
            Ok(aac::ChannelConfiguration::from(
                mp4a.esds.es_desc.dec_config.dec_specific.chan_conf,
            ))
        } else {
            Err(Error::AtomInStblNotFound(
                self.track_id(),
                atom::constant_name::MP4A,
            ))
        }
    }

    pub fn bitrate(&self) -> u32 {
        if let Some(ref mp4a) = self.trak.mdia.minf.stbl.stsd.mp4a {
            mp4a.esds.es_desc.dec_config.avg_bitrate
        } else {
            let dur = self.duration();
            if dur.is_zero() {
                0
            } else {
                let bitrate = self.total_sample_size() as f64 * 8.0 / dur.as_secs_f64();
                bitrate as u32
            }
        }
    }

    pub fn sample_count(&self) -> u32 {
        self.trak.mdia.minf.stbl.stsz.sample_count
    }

    pub fn video_profile(&self) -> Result<h264::Profile> {
        if let Some(ref avc1) = self.trak.mdia.minf.stbl.stsd.avc1 {
            Ok(h264::Profile::from((
                avc1.avcc.avc_profile_indication,
                avc1.avcc.profile_compatibility,
            )))
        } else {
            Err(Error::AtomInStblNotFound(
                self.track_id(),
                atom::constant_name::AVC1,
            ))
        }
    }

    pub fn sequence_parameter_set(&self) -> Result<&[u8]> {
        if let Some(ref avc1) = self.trak.mdia.minf.stbl.stsd.avc1 {
            match avc1.avcc.sequence_parameter_sets.get(0) {
                Some(nal) => Ok(nal.as_bytes()),
                None => Err(Error::EntryInStblNotFound(
                    self.track_id(),
                    atom::constant_name::AVCC,
                    0,
                )),
            }
        } else {
            Err(Error::AtomInStblNotFound(
                self.track_id(),
                atom::constant_name::AVC1,
            ))
        }
    }

    pub fn picture_parameter_set(&self) -> Result<&[u8]> {
        if let Some(ref avc1) = self.trak.mdia.minf.stbl.stsd.avc1 {
            match avc1.avcc.picture_parameter_sets.get(0) {
                Some(nal) => Ok(nal.as_bytes()),
                None => Err(Error::EntryInStblNotFound(
                    self.track_id(),
                    atom::constant_name::AVCC,
                    0,
                )),
            }
        } else {
            Err(Error::AtomInStblNotFound(
                self.track_id(),
                atom::constant_name::AVC1,
            ))
        }
    }

    pub fn audio_profile(&self) -> Result<aac::AudioObjectType> {
        if let Some(ref mp4a) = self.trak.mdia.minf.stbl.stsd.mp4a {
            Ok(aac::AudioObjectType::from(
                mp4a.esds.es_desc.dec_config.dec_specific.profile,
            ))
        } else {
            Err(Error::AtomInStblNotFound(
                self.track_id(),
                atom::constant_name::MP4A,
            ))
        }
    }

    fn stsc_index(&self, sample_id: u32) -> Result<usize> {
        if self.trak.mdia.minf.stbl.stsc.entries.is_empty() {
            return Err(Error::InvalidData("no stsc entries"));
        }
        for (i, entry) in self.trak.mdia.minf.stbl.stsc.entries.iter().enumerate() {
            if sample_id < entry.first_sample {
                return if i == 0 {
                    Err(Error::InvalidData("sample not found"))
                } else {
                    Ok(i - 1)
                };
            }
        }
        Ok(self.trak.mdia.minf.stbl.stsc.entries.len() - 1)
    }

    fn chunk_offset(&self, chunk_id: u32) -> Result<u64> {
        if self.trak.mdia.minf.stbl.stco.is_none() && self.trak.mdia.minf.stbl.co64.is_none() {
            return Err(Error::InvalidData("must have either stco or co64 boxes"));
        }
        if let Some(ref stco) = self.trak.mdia.minf.stbl.stco {
            if let Some(offset) = stco.entries.get(chunk_id as usize - 1) {
                return Ok(*offset as u64);
            } else {
                return Err(Error::EntryInStblNotFound(
                    self.track_id(),
                    atom::constant_name::STCO,
                    chunk_id,
                ));
            }
        } else if let Some(ref co64) = self.trak.mdia.minf.stbl.co64 {
            if let Some(offset) = co64.entries.get(chunk_id as usize - 1) {
                return Ok(*offset);
            } else {
                return Err(Error::EntryInStblNotFound(
                    self.track_id(),
                    atom::constant_name::CO64,
                    chunk_id,
                ));
            }
        }
        Err(Error::Atom2NotFound(
            atom::constant_name::STCO,
            atom::constant_name::CO64,
        ))
    }

    fn ctts_index(&self, sample_id: u32) -> Result<(usize, u32)> {
        let ctts = self.trak.mdia.minf.stbl.ctts.as_ref().unwrap();
        let mut sample_count: u32 = 1;
        for (i, entry) in ctts.entries.iter().enumerate() {
            let next_sample_count =
                sample_count
                    .checked_add(entry.sample_count)
                    .ok_or(Error::InvalidData(
                        "attempt to sum ctts entries sample_count with overflow",
                    ))?;
            if sample_id < next_sample_count {
                return Ok((i, sample_count));
            }
            sample_count = next_sample_count;
        }

        Err(Error::EntryInStblNotFound(
            self.track_id(),
            atom::constant_name::CTTS,
            sample_id,
        ))
    }

    fn sample_size(&self, sample_id: u32) -> Result<u32> {
        let stsz = &self.trak.mdia.minf.stbl.stsz;
        if stsz.sample_size > 0 {
            return Ok(stsz.sample_size);
        }
        if let Some(size) = stsz.sample_sizes.get(sample_id as usize - 1) {
            Ok(*size)
        } else {
            Err(Error::EntryInStblNotFound(
                self.track_id(),
                atom::constant_name::STSZ,
                sample_id,
            ))
        }
    }

    fn total_sample_size(&self) -> u64 {
        let stsz = &self.trak.mdia.minf.stbl.stsz;
        if stsz.sample_size > 0 {
            stsz.sample_size as u64 * self.sample_count() as u64
        } else {
            let mut total_size = 0;
            for size in stsz.sample_sizes.iter() {
                total_size += *size as u64;
            }
            total_size
        }
    }

    pub fn sample_offset(&self, sample_id: u32) -> Result<u64> {
        let stsc_index = self.stsc_index(sample_id)?;

        let stsc = &self.trak.mdia.minf.stbl.stsc;
        let stsc_entry = stsc.entries.get(stsc_index).unwrap();

        let first_chunk = stsc_entry.first_chunk;
        let first_sample = stsc_entry.first_sample;
        let samples_per_chunk = stsc_entry.samples_per_chunk;

        let chunk_id = sample_id
            .checked_sub(first_sample)
            .map(|n| n / samples_per_chunk)
            .and_then(|n| n.checked_add(first_chunk))
            .ok_or(Error::InvalidData(
                "attempt to calculate stsc chunk_id with overflow",
            ))?;

        let chunk_offset = self.chunk_offset(chunk_id)?;

        let first_sample_in_chunk = sample_id - (sample_id - first_sample) % samples_per_chunk;

        let mut sample_offset = 0;
        for i in first_sample_in_chunk..sample_id {
            sample_offset += self.sample_size(i)?;
        }

        Ok(chunk_offset + sample_offset as u64)
    }

    /// Track duration in timescale units.
    pub fn media_duration(&self) -> u64 {
        self.trak.mdia.mdhd.duration
    }

    /// Sample covering `time`, in timescale units.
    pub fn sample_at_time(&self, time: u64) -> u32 {
        let stts = &self.trak.mdia.minf.stbl.stts;

        let mut sample_id: u32 = 1;
        let mut elapsed: u64 = 0;

        for entry in stts.entries.iter() {
            let span = entry.sample_count as u64 * entry.sample_delta as u64;

            if time < elapsed + span {
                let delta = (entry.sample_delta as u64).max(1);
                return sample_id + ((time - elapsed) / delta) as u32;
            }

            sample_id += entry.sample_count;
            elapsed += span;
        }

        sample_id
    }

    /// Nearest sync sample at or before `sample_id`, so a seek lands on a
    /// keyframe rather than mid-GOP.
    pub fn sync_sample_at_or_before(&self, sample_id: u32) -> u32 {
        let Some(ref stss) = self.trak.mdia.minf.stbl.stss else {
            return sample_id;
        };

        match stss.entries.binary_search(&sample_id) {
            Ok(_) => sample_id,
            Err(0) => 1,
            Err(index) => stss.entries[index - 1],
        }
    }

    fn sample_time(&self, sample_id: u32) -> Result<(u64, u32)> {
        let stts = &self.trak.mdia.minf.stbl.stts;

        let mut sample_count: u32 = 1;
        let mut elapsed = 0;

        for entry in stts.entries.iter() {
            let new_sample_count =
                sample_count
                    .checked_add(entry.sample_count)
                    .ok_or(Error::InvalidData(
                        "attempt to sum stts entries sample_count with overflow",
                    ))?;
            if sample_id < new_sample_count {
                let start_time =
                    (sample_id - sample_count) as u64 * entry.sample_delta as u64 + elapsed;
                return Ok((start_time, entry.sample_delta));
            }

            sample_count = new_sample_count;
            elapsed += entry.sample_count as u64 * entry.sample_delta as u64;
        }

        Err(Error::EntryInStblNotFound(
            self.track_id(),
            atom::constant_name::STTS,
            sample_id,
        ))
    }

    fn sample_rendering_offset(&self, sample_id: u32) -> i32 {
        if let Some(ref ctts) = self.trak.mdia.minf.stbl.ctts {
            if let Ok((ctts_index, _)) = self.ctts_index(sample_id) {
                let ctts_entry = ctts.entries.get(ctts_index).unwrap();
                return ctts_entry.sample_offset;
            }
        }
        0
    }

    fn is_sync_sample(&self, sample_id: u32) -> bool {
        if let Some(ref stss) = self.trak.mdia.minf.stbl.stss {
            stss.entries.binary_search(&sample_id).is_ok()
        } else {
            true
        }
    }

    /// What a sample is, straight from the index: no bytes are read.
    pub fn sample_info(&self, sample_id: u32) -> Result<Option<SampleInfo>> {
        let size = match self.sample_size(sample_id) {
            Ok(size) => size,
            Err(Error::EntryInStblNotFound(_, _, _)) => return Ok(None),
            Err(err) => return Err(err),
        };

        let (start_time, duration) = self.sample_time(sample_id)?;

        Ok(Some(SampleInfo {
            start_time,
            duration,
            rendering_offset: self.sample_rendering_offset(sample_id),
            is_sync: self.is_sync_sample(sample_id),
            size: size as usize,
        }))
    }

    /// Reads a sample straight into `into`, which must be its size.
    ///
    /// Only for 4-byte length prefixes, where Annex-B is the same size and is
    /// rewritten where it lies.
    pub fn read_sample_into<R: Read + Seek>(
        &self,
        reader: &mut R,
        sample_id: u32,
        into: &mut [u8],
    ) -> Result<()> {
        let sample_offset = self.sample_offset(sample_id)?;

        reader.seek(SeekFrom::Start(sample_offset))?;
        reader.read_exact(into)?;

        if self.nalu_length_size().is_some() && !nalu::avcc_to_annexb_in_place(into) {
            return Err(Error::InvalidData("malformed sample"));
        }

        Ok(())
    }

    pub fn read_sample<R: Read + Seek>(
        &self,
        reader: &mut R,
        sample_id: u32,
    ) -> Result<Option<Sample>> {
        let sample_offset = match self.sample_offset(sample_id) {
            Ok(offset) => offset,
            Err(Error::EntryInStblNotFound(_, _, _)) => return Ok(None),
            Err(err) => return Err(err),
        };
        let sample_size = match self.sample_size(sample_id) {
            Ok(size) => size,
            Err(Error::EntryInStblNotFound(_, _, _)) => return Ok(None),
            Err(err) => return Err(err),
        };

        let mut buffer = vec![0x0u8; sample_size as usize];
        reader.seek(SeekFrom::Start(sample_offset))?;
        reader.read_exact(&mut buffer)?;

        // h26x samples are stored as AVCC, the pipeline carries Annex-B.
        if let Some(length_size) = self.nalu_length_size() {
            buffer = nalu::avcc_to_annexb(buffer, length_size)
                .ok_or(Error::InvalidData("malformed sample"))?;
        }

        let (start_time, duration) = self.sample_time(sample_id).unwrap(); // XXX
        let rendering_offset = self.sample_rendering_offset(sample_id);
        let is_sync = self.is_sync_sample(sample_id);

        Ok(Some(Sample {
            start_time,
            duration,
            rendering_offset,
            is_sync,
            bytes: Bytes::from(buffer),
        }))
    }
}
