//! Offline additive/FM-lite synth tuned for *fast, good-enough* previews.
//!
//! Per-voice timbres (chosen by MIDI channel):
//! * ch 10 → drums: sine-sweep kick, band-noise snare/hats, noise-burst crash.
//! * ch 3  → bass: triangle with lowpass-ish harmonic rolloff + fast decay.
//! * ch 5  → Rhodes-like chords: FM pair (carrier + 2× modulator) with a soft
//!   bell transient and medium release.
//! * ch 1  → melody: detuned dual saws through a one-pole lowpass.
//!
//! Everything renders into a single f32 stereo buffer at 44.1 kHz, gets a
//! gentle master lowpass (the "lofi" veil), a touch of vinyl-style noise is
//! intentionally NOT added here (that belongs to the future render engine).

use crate::music::{NoteOn, Song, Ticks};
use crate::preview::notes_by_channel;

pub const SAMPLE_RATE: usize = 44_100;
const PPQ: i64 = 480;

/// A rendered preview: interleaved stereo f32 samples in -1..1 plus metadata.
#[derive(Debug, Clone)]
pub struct RenderedAudio {
    pub sample_rate: usize,
    /// Interleaved [L, R, L, R, ...].
    pub samples: Vec<f32>,
    pub seconds: f64,
}

impl RenderedAudio {
    /// Peak absolute sample value.
    pub fn peak(&self) -> f32 {
        self.samples.iter().fold(0f32, |m, s| m.max(s.abs()))
    }

    /// RMS loudness estimate.
    pub fn rms(&self) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.samples.iter().map(|s| (*s as f64) * (*s as f64)).sum();
        (sum / self.samples.len() as f64).sqrt() as f32
    }
}

/// Render the whole song to a preview buffer.
pub fn render_song(song: &Song) -> RenderedAudio {
    let channels = notes_by_channel(song);

    // Total length: song end + 2 s tail for releases.
    let last_tick = channels
        .iter()
        .flat_map(|(_, ns)| ns.iter().map(|n| n.end().as_i64()))
        .max()
        .unwrap_or(0)
        .max(song.total_ticks.as_i64());
    let seconds = last_tick as f64 / (PPQ as f64 * song.tempo_bpm / 60.0) + 2.0;
    let n_frames = (seconds * SAMPLE_RATE as f64) as usize;

    let mut buf = vec![0f32; n_frames * 2];

    for (ch, notes) in &channels {
        for n in notes {
            render_note(*ch, n, song.tempo_bpm, &mut buf);
        }
    }

    // Master chain: one-pole lowpass ~7.5 kHz (veil) + soft clip.
    let alpha = lp_alpha(7500.0);
    let (mut sl, mut sr) = (0f32, 0f32);
    for frame in buf.chunks_exact_mut(2) {
        sl += alpha * (frame[0] - sl);
        sr += alpha * (frame[1] - sr);
        frame[0] = soft_clip(sl);
        frame[1] = soft_clip(sr);
    }

    RenderedAudio { sample_rate: SAMPLE_RATE, samples: buf, seconds }
}

/// tick → sample index (rounded).
fn tick_to_frame(tick: i64, bpm: f64) -> usize {
    let sec = tick as f64 * 60.0 / (bpm * PPQ as f64);
    (sec * SAMPLE_RATE as f64).round().max(0.0) as usize
}

fn tick_dur_to_frames(dur: Ticks, bpm: f64) -> usize {
    (dur.as_i64() as f64 * 60.0 / (bpm * PPQ as f64) * SAMPLE_RATE as f64).round().max(1.0) as usize
}

/// Dispatch one note to its voice model.
fn render_note(channel: u8, n: &NoteOn, bpm: f64, buf: &mut [f32]) {
    let start = tick_to_frame(n.start.as_i64(), bpm);
    let dur = tick_dur_to_frames(n.duration, bpm);
    if start >= buf.len() / 2 {
        return;
    }
    let vel = n.velocity as f32 / 127.0;
    match channel {
        10 => render_drum(n.pitch.midi(), vel, start, buf),
        3 => render_bass(n.pitch.freq_hz(), vel, start, dur, buf),
        5 => render_rhodes(n.pitch.freq_hz(), vel, start, dur, buf),
        _ => render_lead(n.pitch.freq_hz(), vel, start, dur, buf),
    }
}

/// Add `gain * wave(i)` into the stereo pair from `start`.
fn add_voice(buf: &mut [f32], start: usize, frames: usize, gain: f32, pan: f32, mut wave: impl FnMut(usize) -> f32) {
    let gl = gain * (1.0 - pan * 0.5).min(1.0);
    let gr = gain * (1.0 + pan * 0.5).min(1.0) * 0.9;
    let total = buf.len() / 2;
    let end = (start + frames).min(total);
    for i in start..end {
        let s = wave(i - start);
        buf[i * 2] += s * gl;
        buf[i * 2 + 1] += s * gr;
    }
}

/// ADSR-ish envelope with exponential release; t in frames.
fn env(t: usize, attack_f: usize, hold_f: usize, release_f: usize, sustain: f32) -> f32 {
    if t < attack_f {
        (t as f32 / attack_f.max(1) as f32).powi(2)
    } else if t < attack_f + hold_f {
        sustain
    } else if t < attack_f + hold_f + release_f {
        let p = (t - attack_f - hold_f) as f32 / release_f as f32;
        sustain * (-3.0 * p).exp()
    } else {
        0.0
    }
}

fn render_bass(freq: f32, vel: f32, start: usize, dur: usize, buf: &mut [f32]) {
    let frames = dur + (0.25 * SAMPLE_RATE as f32) as usize;
    let att = (0.008 * SAMPLE_RATE as f32) as usize;
    let rel = (0.15 * SAMPLE_RATE as f32) as usize;
    let phase = 0f64;
    let mut ph = phase as f32;
    let mut lp = 0f32;
    let k = lp_alpha(freq.min(1200.0));
    add_voice(buf, start, frames, vel * 0.5, 0.0, move |i| {
        let t = i as f64 / SAMPLE_RATE as f64;
        let raw = (std::f64::consts::PI * 2.0 * freq as f64 * t).sin() * 0.7
            + (std::f64::consts::PI * 4.0 * freq as f64 * t).sin() * 0.25;
        lp += k * (raw as f32 - lp);
        let e = env(i, att, dur.saturating_sub(att), rel, 0.8);
        ph = (ph + freq / SAMPLE_RATE as f32) % 1.0; // keep closure state honest
        lp * e
    });
}

fn render_rhodes(freq: f32, vel: f32, start: usize, dur: usize, buf: &mut [f32]) {
    // FM: carrier + modulator at 2× ratio decaying fast → tine+bell character.
    let frames = dur + (0.6 * SAMPLE_RATE as f32) as usize;
    let att = (0.004 * SAMPLE_RATE as f32) as usize;
    let rel = (0.5 * SAMPLE_RATE as f32) as usize;
    let fmod = freq * 2.0;
    add_voice(buf, start, frames, vel * 0.34, 0.0, move |i| {
        let t = i as f64 / SAMPLE_RATE as f64;
        let mod_env = (-6.0 * t).exp();
        let idx = 2.2 * mod_env as f32;
        let car = (std::f64::consts::TAU * freq as f64 * t).sin() as f32;
        let mo = (std::f64::consts::TAU * fmod as f64 * t).sin() as f32;
        let s = (car + idx * mo).sin();
        s * env(i, att, dur.saturating_sub(att), rel, 0.6)
    });
}

fn render_lead(freq: f32, vel: f32, start: usize, dur: usize, buf: &mut [f32]) {
    let frames = dur + (0.3 * SAMPLE_RATE as f32) as usize;
    let att = (0.02 * SAMPLE_RATE as f32) as usize;
    let rel = (0.25 * SAMPLE_RATE as f32) as usize;
    let mut lp = 0f32;
    let k = lp_alpha(freq * 4.0 + 1500.0);
    add_voice(buf, start, frames, vel * 0.3, 0.15, move |i| {
        let t = i as f64 / SAMPLE_RATE as f64;
        // Two slightly detuned hard-ish saws (band-limited via Fourier sum).
        let mut s = 0f32;
        for h in 1..8u32 {
            let hf = freq * h as f32;
            let amp = 1.0 / h as f32;
            s += amp * (std::f64::consts::TAU * hf as f64 * t).sin() as f32;
            s += amp * 0.9 * (std::f64::consts::TAU * (hf * 1.005) as f64 * t).sin() as f32;
        }
        lp += k * (s - lp);
        lp * env(i, att, dur.saturating_sub(att), rel, 0.75)
    });
}

fn render_drum(key: u8, vel: f32, start: usize, buf: &mut [f32]) {
    match key {
        36 => {
            // Kick: 150→45 Hz pitch drop over 90 ms.
            let frames = (0.28 * SAMPLE_RATE as f32) as usize;
            add_voice(buf, start, frames, vel * 0.9, 0.0, move |i| {
                let t = i as f64 / SAMPLE_RATE as f64;
                let f = 150.0 * (-t * 28.0).exp() + 45.0;
                (std::f64::consts::TAU * f * t).sin() * (-t * 14.0).exp()
            });
        }
        38 | 37 => {
            // Snare/stick: noise burst + 190 Hz body.
            let frames = (0.18 * SAMPLE_RATE as f32) as usize;
            let seed = start as u64;
            add_voice(buf, start, frames, vel * 0.55, -0.1, move |i| {
                let t = i as f64 / SAMPLE_RATE as f64;
                let nz = hash_noise(seed, i) * (-t * 22.0).exp();
                let body = (std::f64::consts::TAU * 190.0 * t).sin() as f32 * (-t * 30.0).exp();
                nz * 0.7 + body * 0.5
            });
        }
        44 => {
            // Closed hat: short bright noise.
            let frames = (0.05 * SAMPLE_RATE as f32) as usize;
            let seed = start as u64 ^ 0x9E37;
            add_voice(buf, start, frames, vel * 0.32, 0.25, move |i| {
                let t = i as f64 / SAMPLE_RATE as f64;
                hash_noise(seed, i) * (-t * 60.0).exp()
            });
        }
        46 => {
            // Open hat: longer, softer.
            let frames = (0.22 * SAMPLE_RATE as f32) as usize;
            let seed = start as u64 ^ 0x51ED;
            add_voice(buf, start, frames, vel * 0.3, 0.3, move |i| {
                let t = i as f64 / SAMPLE_RATE as f64;
                hash_noise(seed, i) * (-t * 12.0).exp()
            });
        }
        49 => {
            // Crash: bright noise wash.
            let frames = (0.9 * SAMPLE_RATE as f32) as usize;
            let seed = start as u64 ^ 0xC0FFEE;
            add_voice(buf, start, frames, vel * 0.35, -0.3, move |i| {
                let t = i as f64 / SAMPLE_RATE as f64;
                hash_noise(seed, i) * (-t * 4.0).exp()
            });
        }
        50 | 45 => {
            // Toms: pitched sine drops.
            let base = if key == 50 { 220.0 } else { 150.0 };
            let frames = (0.25 * SAMPLE_RATE as f32) as usize;
            add_voice(buf, start, frames, vel * 0.6, 0.1, move |i| {
                let t = i as f64 / SAMPLE_RATE as f64;
                let f = base * (-t * 10.0).exp() + 60.0;
                (std::f64::consts::TAU * f * t).sin() * (-t * 12.0).exp()
            });
        }
        70 => {
            // Shaker: tiny filtered ticks.
            let frames = (0.03 * SAMPLE_RATE as f32) as usize;
            let seed = start as u64 ^ 0x2D95;
            add_voice(buf, start, frames, vel * 0.18, 0.4, move |i| {
                let t = i as f64 / SAMPLE_RATE as f64;
                hash_noise(seed, i) * (-t * 90.0).exp()
            });
        }
        _ => {}
    }
}

/// Deterministic cheap white noise in -1..1 (per-seed, per-sample).
fn hash_noise(seed: u64, i: usize) -> f32 {
    let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (i as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    ((x as f64) / (u64::MAX as f64) * 2.0 - 1.0) as f32
}

/// One-pole lowpass coefficient for cutoff Hz at our sample rate.
fn lp_alpha(hz: f32) -> f32 {
    let dt = 1.0 / SAMPLE_RATE as f32;
    let rc = 1.0 / (std::f64::consts::TAU * hz.max(20.0) as f64) as f32;
    dt / (rc + dt)
}

/// tanh-flavoured soft clip that keeps headroom below unity.
fn soft_clip(x: f32) -> f32 {
    x.tanh()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GenerateConfig;
    use crate::pipeline::composer::generate_song;

    #[test]
    fn renders_nonzero_audio_of_expected_length() {
        let song = generate_song(&GenerateConfig::default()).unwrap().song;
        let audio = render_song(&song);
        assert!(audio.samples.len() % 2 == 0);
        assert!(audio.seconds > 10.0);
        assert!(audio.rms() > 0.001, "silent render");
        assert!(audio.peak() <= 1.0 + 1e-6);
    }

    #[test]
    fn deterministic_render() {
        let song = generate_song(&GenerateConfig::default()).unwrap().song;
        let a = render_song(&song);
        let b = render_song(&song);
        assert_eq!(a.samples, b.samples);
    }

    #[test]
    fn empty_song_renders_silence_without_panic() {
        let song = Song::new("empty", 70.0, 4, Ticks(480), vec![]);
        let audio = render_song(&song);
        assert_eq!(audio.rms(), 0.0);
    }

    #[test]
    fn tick_math_matches_tempo() {
        // One bar at 120 BPM = 1 second.
        assert_eq!(tick_to_frame(480, 120.0), 0);
        assert_eq!(tick_to_frame(960, 120.0), SAMPLE_RATE);
    }
}
