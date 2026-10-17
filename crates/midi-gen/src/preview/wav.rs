//! Minimal 16-bit PCM stereo WAV muxer.

use std::io::Write;

use super::synth::RenderedAudio;

/// Encode [`RenderedAudio`] as RIFF/WAVE bytes (PCM s16le, stereo).
pub fn wav_bytes(audio: &RenderedAudio) -> Vec<u8> {
    let sr = audio.sample_rate as u32;
    let bits: u16 = 16;
    let channels: u16 = 2;
    let byte_rate = sr * channels as u32 * bits as u32 / 8;
    let block_align: u16 = channels * bits / 8;
    let data_len = (audio.samples.len() * 2) as u32; // f32 → s16 conversion below

    let mut out = Vec::with_capacity(44 + audio.samples.len() * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");

    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sr.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&bits.to_le_bytes());

    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for &s in &audio.samples {
        // Clamp to [-1, 1] then scale to i16 range.
        let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// Write a `.wav` file to disk.
pub fn write_wav(audio: &RenderedAudio, path: &str) -> std::io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    f.write_all(&wav_bytes(audio))?
        ;
    f.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GenerateConfig;
    use crate::preview::render_song;
    use crate::pipeline::composer::generate_song;

    #[test]
    fn wav_header_is_valid() {
        let song = generate_song(&GenerateConfig::default()).unwrap().song;
        let audio = render_song(&song);
        let bytes = wav_bytes(&audio);
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(&bytes[12..16], b"fmt ");
        assert_eq!(&bytes[36..40], b"data");
        let declared = u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize;
        assert_eq!(declared + 44, bytes.len());
        let riff_size = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        assert_eq!(riff_size + 8, bytes.len());
    }

    #[test]
    fn writes_file_to_disk() {
        let song = generate_song(&GenerateConfig::default()).unwrap().song;
        let audio = render_song(&song);
        let path = std::env::temp_dir().join("midigen_preview_test.wav");
        write_wav(&audio, path.to_str().unwrap()).unwrap();
        let on_disk = std::fs::read(&path).unwrap();
        assert_eq!(on_disk.len(), wav_bytes(&audio).len());
        std::fs::remove_file(path).ok();
    }
}
