use std::{fmt, io::Cursor};

use symphonia::{
    core::{
        audio::{AudioBufferRef, SampleBuffer},
        codecs::{DecoderOptions, CODEC_TYPE_NULL},
        errors::Error as SymphoniaError,
        formats::FormatOptions,
        io::MediaSourceStream,
        meta::MetadataOptions,
        probe::Hint,
    },
    default::{get_codecs, get_probe},
};

#[derive(Clone, Debug)]
pub struct DecodedPcm {
    pub sample_rate: u32,
    pub channels: usize,
    pub interleaved_samples: Vec<f32>,
}

impl DecodedPcm {
    pub fn frame_count(&self) -> usize {
        self.interleaved_samples.len() / self.channels
    }
}

#[derive(Debug)]
pub enum DecodeError {
    Probe(String),
    MissingTrack,
    UnsupportedTrack,
    Decode(String),
    Empty,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Probe(err) => write!(f, "failed to probe audio: {err}"),
            Self::MissingTrack => write!(f, "no audio track found"),
            Self::UnsupportedTrack => write!(f, "audio track is missing required parameters"),
            Self::Decode(err) => write!(f, "failed to decode audio: {err}"),
            Self::Empty => write!(f, "decoded audio did not contain samples"),
        }
    }
}

impl std::error::Error for DecodeError {}

pub fn decode_audio(bytes: &[u8], hint_ext: &str) -> Result<DecodedPcm, DecodeError> {
    let cursor = Cursor::new(bytes.to_vec());
    let media_source = MediaSourceStream::new(Box::new(cursor), Default::default());
    let mut hint = Hint::new();
    hint.with_extension(hint_ext);

    let probed = get_probe()
        .format(
            &hint,
            media_source,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|err| DecodeError::Probe(err.to_string()))?;

    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|track| track.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or(DecodeError::MissingTrack)?;

    let track_id = track.id;

    let mut decoder = get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|err| DecodeError::Decode(err.to_string()))?;

    let mut sample_rate = track.codec_params.sample_rate.unwrap_or_default();
    let mut samples = Vec::new();
    let mut sample_buffer = None;

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(err))
                if err.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break
            }
            Err(SymphoniaError::ResetRequired) => {
                return Err(DecodeError::Decode("decoder reset required".to_string()));
            }
            Err(err) => return Err(DecodeError::Decode(err.to_string())),
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(err) => return Err(DecodeError::Decode(err.to_string())),
        };

        sample_rate = decoded.spec().rate;
        append_as_stereo(&decoded, &mut sample_buffer, &mut samples);
    }

    if samples.is_empty() {
        return Err(DecodeError::Empty);
    }

    Ok(DecodedPcm {
        sample_rate,
        channels: 2,
        interleaved_samples: samples,
    })
}

fn append_as_stereo(
    decoded: &AudioBufferRef<'_>,
    sample_buffer: &mut Option<SampleBuffer<f32>>,
    output: &mut Vec<f32>,
) {
    let spec = *decoded.spec();
    let duration = decoded.capacity() as u64;
    let buffer = sample_buffer.get_or_insert_with(|| SampleBuffer::<f32>::new(duration, spec));
    if buffer.capacity() < decoded.capacity() {
        *buffer = SampleBuffer::<f32>::new(duration, spec);
    }
    buffer.copy_interleaved_ref(decoded.clone());

    let channels = spec.channels.count();
    for frame in buffer.samples().chunks(channels) {
        let left = frame.first().copied().unwrap_or(0.0);
        let right = frame.get(1).copied().unwrap_or(left);
        output.push(left);
        output.push(right);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_MP3: &[u8] = include_bytes!("../resources/sample.mp3");
    const SAMPLE_WAV: &[u8] = include_bytes!("../resources/sample.wav");
    const SAMPLE_AIFF: &[u8] = include_bytes!("../resources/sample.aiff");
    const SAMPLE_FLAC: &[u8] = include_bytes!("../resources/sample.flac");
    const SAMPLE_M4A: &[u8] = include_bytes!("../resources/sample.m4a");

    fn assert_decodes(bytes: &[u8], ext: &str, label: &str) {
        let decoded = decode_audio(bytes, ext).unwrap_or_else(|err| {
            panic!("{label} should decode: {err}");
        });
        assert_eq!(decoded.channels, 2, "{label} should be stereo");
        assert!(decoded.sample_rate > 0, "{label} should have non-zero rate");
        assert_eq!(
            decoded.interleaved_samples.len() % 2,
            0,
            "{label} samples must be paired",
        );
        assert!(
            !decoded.interleaved_samples.is_empty(),
            "{label} must produce samples",
        );
    }

    #[test]
    fn test_decode_returns_stereo_interleaved_f32_for_mp3_sample() {
        assert_decodes(SAMPLE_MP3, "mp3", "MP3");
    }

    #[test]
    fn test_decode_returns_stereo_interleaved_f32_for_wav_sample() {
        assert_decodes(SAMPLE_WAV, "wav", "WAV");
    }

    #[test]
    fn test_decode_returns_stereo_interleaved_f32_for_aiff_sample() {
        assert_decodes(SAMPLE_AIFF, "aiff", "AIFF");
    }

    #[test]
    fn test_decode_returns_stereo_interleaved_f32_for_flac_sample() {
        assert_decodes(SAMPLE_FLAC, "flac", "FLAC");
    }

    #[test]
    fn test_decode_returns_stereo_interleaved_f32_for_alac_sample() {
        assert_decodes(SAMPLE_M4A, "m4a", "ALAC");
    }

    #[test]
    fn test_decode_returns_error_for_truncated_input() {
        let result = decode_audio(&SAMPLE_MP3[..16], "mp3");
        assert!(matches!(
            result,
            Err(DecodeError::Probe(_) | DecodeError::Decode(_) | DecodeError::Empty)
        ));
    }
}
