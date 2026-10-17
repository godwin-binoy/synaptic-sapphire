//! `midigen` — generate lo-fi MIDI songs and instant audio previews.
//!
//! Examples:
//! ```text
//! midgen generate --seed 42 --bars 32 --out song.mid
//! midgen preview --seed 42 --out song.wav          # hear it immediately
//! midgen generate --config my.toml                 # full config file
//! ```

use clap::{Parser, Subcommand};
use midi_gen::config::GenerateConfig;
use midi_gen::pipeline::generate_song;
use midi_gen::preview::render_song;
use std::io::Write;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "midigen", version, about = "Lo-fi MIDI generator + preview engine")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Generate a .mid file from seed/config knobs.
    Generate {
        /// Deterministic seed (same seed + knobs → same song).
        #[arg(long, default_value_t = 1)]
        seed: u64,
        /// Target length in bars (rounded up to the arrangement).
        #[arg(long, default_value_t = 32)]
        bars: u32,
        /// Tempo in BPM.
        #[arg(long, default_value_t = 72.0)]
        bpm: f64,
        /// Key bias: lofi_minor | all_keys | bright | dark
        #[arg(long, default_value = "lofi_minor")]
        key_bias: String,
        /// Chord flavour: lofi_jazz | neo_soul | ambient | bluesy | mixed
        #[arg(long, default_value = "lofi_jazz")]
        chords_style: String,
        /// Swing strength 0..1.
        #[arg(long, default_value_t = 0.15)]
        swing: f64,
        /// Melody density scale 0..2.
        #[arg(long, default_value_t = 1.0)]
        density: f64,
        /// Groove intensity 0..2.
        #[arg(long, default_value_t = 1.0)]
        groove: f64,
        /// Load a TOML/JSON config file (CLI flags override it).
        #[arg(long)]
        config: Option<PathBuf>,
        /// Output .mid path ("-" for stdout).
        #[arg(long, default_value = "song.mid")]
        out: PathBuf,
    },
    /// Render a quick stereo WAV preview of the generated song.
    Preview {
        #[arg(long, default_value_t = 1)]
        seed: u64,
        #[arg(long, default_value_t = 32)]
        bars: u32,
        #[arg(long, default_value_t = 72.0)]
        bpm: f64,
        #[arg(long, default_value = "lofi_minor")]
        key_bias: String,
        #[arg(long, default_value = "lofi_jazz")]
        chords_style: String,
        #[arg(long, default_value_t = 0.15)]
        swing: f64,
        #[arg(long, default_value_t = 1.0)]
        density: f64,
        #[arg(long, default_value_t = 1.0)]
        groove: f64,
        #[arg(long)]
        config: Option<PathBuf>,
        /// Output .wav path ("-" for stdout).
        #[arg(long, default_value = "preview.wav")]
        out: PathBuf,
        /// Also write the matching .mid next to the wav.
        #[arg(long)]
        also_mid: bool,
    },
}

/// Shared knob struct so both subcommands build the same config.
#[allow(clippy::too_many_arguments)]
fn build_cfg(seed: u64, bars: u32, bpm: f64, key_bias: &str, style: &str, swing: f64, density: f64, groove: f64, file: Option<&PathBuf>) -> anyhow::Result<GenerateConfig> {
    let mut cfg = match file {
        Some(p) => {
            let text = std::fs::read_to_string(p)
                .map_err(|e| anyhow::anyhow!("cannot read config {}: {e}", p.display()))?;
            serde_json::from_str(&text)
                .map_err(|e| anyhow::anyhow!("invalid JSON config {}: {e}", p.display()))?
        }
        None => GenerateConfig::default(),
    };
    cfg.seed = seed;
    cfg.arrangement.bars_hint = bars;
    cfg.tempo_bpm = bpm;
    cfg.key_bias = parse_key_bias(key_bias);
    cfg.chords.progression_style = parse_style(style);
    cfg.humanize.swing = swing;
    cfg.melody.density_scale = density;
    cfg.drums.groove_intensity = groove;
    Ok(cfg)
}

fn parse_key_bias(s: &str) -> midi_gen::config::KeyBias {
    use midi_gen::config::KeyBias::*;
    match s {
        "all_keys" => AllKeys,
        "bright" => Bright,
        "dark" => Dark,
        _ => LofiMinor,
    }
}

fn parse_style(s: &str) -> midi_gen::config::ProgressionStyle {
    use midi_gen::config::ProgressionStyle::*;
    match s {
        "neo_soul" => NeoSoul,
        "ambient" => Ambient,
        "bluesy" => Bluesy,
        "mixed" => Mixed,
        _ => LofiJazz,
    }
}

fn emit(path: &PathBuf, bytes: &[u8]) -> anyhow::Result<()> {
    if path.to_str() == Some("-") {
        std::io::stdout().write_all(bytes)?;
    } else {
        std::fs::write(path, bytes)?;
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Generate { seed, bars, bpm, key_bias, chords_style, swing, density, groove, config, out } => {
            let cfg = build_cfg(seed, bars, bpm, &key_bias, &chords_style, swing, density, groove, config.as_ref())?;
            let t0 = std::time::Instant::now();
            let song_out = generate_song(&cfg)?;
            let bytes = song_out.midi_bytes();
            emit(&out, &bytes)?;
            println!("{}  [{} ms]", song_out.blueprint_summary, t0.elapsed().as_millis());
            println!("wrote {} ({} bytes)", out.display(), bytes.len());
        }
        Cmd::Preview { seed, bars, bpm, key_bias, chords_style, swing, density, groove, config, out, also_mid } => {
            let cfg = build_cfg(seed, bars, bpm, &key_bias, &chords_style, swing, density, groove, config.as_ref())?;
            let gen = generate_song(&cfg)?;
            if also_mid {
                let mid = out.with_extension("mid");
                emit(&mid, &gen.midi_bytes())?;
                println!("wrote {}", mid.display());
            }
            let t0 = std::time::Instant::now();
            let audio = render_song(&gen.song);
            let bytes = midi_gen::preview::wav_bytes(&audio);
            emit(&out, &bytes)?;
            println!(
                "preview: {:.1}s @ {} Hz, peak {:.2}, rms {:.3}  [{} ms render]",
                audio.seconds,
                audio.sample_rate,
                audio.peak(),
                audio.rms(),
                t0.elapsed().as_millis()
            );
            println!("wrote {} ({} bytes)", out.display(), bytes.len());
        }
    }
    Ok(())
}
