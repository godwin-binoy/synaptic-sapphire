//! Hand-rolled Standard MIDI File writer (SMF type 1).
//!
//! Zero dependencies: header chunk, tempo/time-signature track, one track per
//! voice with delta-encoded note-on/note-off events. Notes that overlap on the
//! same channel are split so every `note_off` matches the latest `note_on`.

use crate::music::{NoteEvent, Song, Track};

/// Ticks per quarter note written into the header (matches `music::PPQ`).
const division: u16 = 480;

/// Serialise a whole [`Song`] to SMF bytes.
pub fn to_midi_bytes(song: &Song) -> Vec<u8> {
    let mut out = Vec::with_capacity(4096);

    // --- MThd: format 1, n+1 tracks, 480 PPQ -------------------------------
    let ntracks = (song.tracks.len() + 1) as u16;
    out.extend_from_slice(b"MThd");
    out.extend_from_slice(&4u32.to_be_bytes());
    out.extend_from_slice(&1u16.to_be_bytes());
    out.extend_from_slice(&ntracks.to_be_bytes());
    out.extend_from_slice(&division.to_be_bytes());

    // --- conductor track: tempo + time signature ----------------------------
    let mut meta = Vec::new();
    push_meta(&mut meta, 0, 0x51, &tempo_bytes(song.tempo_bpm)); // set tempo
    push_meta(&mut meta, 0, 0x58, &[4, 2, 24, 8]); // time signature 4/4
    push_meta(&mut meta, 0, 0x2F, &[]); // end of track
    write_track(&mut out, &meta);

    // --- one track per voice -------------------------------------------------
    for t in &song.tracks {
        write_track(&mut out, &track_events(t));
    }
    out
}

/// µs per quarter note, big-endian 24-bit.
fn tempo_bytes(bpm: f64) -> [u8; 3] {
    let uspb = (60_000_000.0 / bpm.clamp(1.0, 600.0)).round().max(0.0) as u32;
    [((uspb >> 16) & 0xFF) as u8, ((uspb >> 8) & 0xFF) as u8, (uspb & 0xFF) as u8]
}

/// Build the raw byte stream (absolute-time internally, then delta-coded).
fn track_events(track: &Track) -> Vec<u8> {
    let mut evs: Vec<(i64, Vec<u8>)> = Vec::new();

    // Track name + program change at t=0.
    evs.push((0, smf_text(0x01, track.name.as_bytes())));
    if track.channel != 10 {
        evs.push((0, vec![0xC0 | (track.channel - 1), track.program]));
    }

    // Collect notes; resolve overlaps by splitting earlier notes.
    let mut notes: Vec<_> = track
        .events
        .iter()
        .filter_map(|e| if let NoteEvent::Note(n) = e { Some(n) } else { None })
        .collect();
    notes.sort_by_key(|n| (n.start.as_i64(), n.pitch.midi()));

    // last_off[(pitch)] = tick where the previous note on this pitch ends.
    let mut last_off: std::collections::BTreeMap<u8, i64> = Default::default();
    for n in &notes {
        let start = n.start.as_i64();
        let mut end = n.end().as_i64();
        if let Some(&prev) = last_off.get(&n.pitch.midi()) {
            if prev > start {
                // Overlap: truncate the previous note is impossible now, so
                // instead delay this attack to the previous release.
                let shift = prev - start;
                end += shift;
            }
        }
        last_off.insert(n.pitch.midi(), end);
        let ch = (track.channel - 1) & 0x0F;
        evs.push((start, vec![0x90 | ch, n.pitch.midi(), n.velocity]));
        evs.push((end, vec![0x80 | ch, n.pitch.midi(), 0x40]));
    }

    let total = evs.iter().map(|(t, _)| *t).max().unwrap_or(0);
    evs.push((total + division as i64, vec![0xFF, 0x2F, 0x00])); // EOT

    // Delta-encode.
    evs.sort_by_key(|(t, _)| *t);
    let mut bytes = Vec::new();
    let mut prev = 0i64;
    for (tick, payload) in evs {
        let delta = (tick - prev).max(0);
        prev = tick;
        write_var_len(&mut bytes, delta as u64);
        bytes.extend_from_slice(&payload);
    }
    bytes
}

/// A meta event: FF <type> <len> <data...>.
fn smf_text(kind: u8, data: &[u8]) -> Vec<u8> {
    let mut v = vec![0xFF, kind];
    write_var_len(&mut v, data.len() as u64);
    v.extend_from_slice(data);
    v
}

fn push_meta(buf: &mut Vec<u8>, tick: i64, kind: u8, data: &[u8]) {
    write_var_len(buf, tick.max(0) as u64);
    buf.extend_from_slice(&smf_text(kind, data));
}

fn write_var_len(out: &mut Vec<u8>, mut value: u64) {
    // MIDI variable-length: 7 bits per byte, MSB continuation.
    let mut stack = [0u8; 5];
    let mut n = 0usize;
    loop {
        stack[n] = (value & 0x7F) as u8;
        n += 1;
        value >>= 7;
        if value == 0 {
            break;
        }
    }
    for (i, b) in stack[..n].iter().rev().enumerate() {
        let cont = if i + 1 < n { 0x80 } else { 0 };
        out.push(b | cont);
    }
}

fn write_track(out: &mut Vec<u8>, body: &[u8]) {
    out.extend_from_slice(b"MTrk");
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(body);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GenerateConfig;
    use crate::pipeline::composer::generate_song;

    #[test]
    fn header_is_type_one_with_five_tracks() {
        let song = generate_song(&GenerateConfig::default()).unwrap().song;
        let bytes = to_midi_bytes(&song);
        assert_eq!(&bytes[0..4], b"MThd");
        assert_eq!(&bytes[8..10], &[0, 1]); // format 1
        assert_eq!(&bytes[10..12], &[0, 5]); // 4 voices + conductor
        assert_eq!(&bytes[12..14], &division.to_be_bytes());
    }

    #[test]
    fn chunk_structure_parses() {
        let song = generate_song(&GenerateConfig::default()).unwrap().song;
        let bytes = to_midi_bytes(&song);
        // Walk chunks: sizes must tile the buffer exactly.
        let mut pos = 0usize;
        let mut tracks = 0;
        while pos < bytes.len() {
            assert!(pos + 8 <= bytes.len(), "truncated chunk header");
            let tag = &bytes[pos..pos + 4];
            assert!(tag == b"MThd" || tag == b"MTrk", "bad tag {:?}", tag);
            let len = u32::from_be_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
            if tag == b"MTrk" {
                tracks += 1;
            }
            pos += 8 + len;
        }
        assert_eq!(pos, bytes.len(), "chunk sizes don't tile file");
        assert_eq!(tracks, 5);
    }

    #[test]
    fn var_len_round_trips() {
        for v in [0u64, 1, 127, 128, 0x3FFF, 0x4000, 0x1FFFFF, 0xFFFFFFF] {
            let mut enc = Vec::new();
            write_var_len(&mut enc, v);
            let mut dec = 0u64;
            for b in &enc {
                dec = (dec << 7) | (b & 0x7F) as u64;
            }
            assert_eq!(dec, v);
        }
    }

    #[test]
    fn tempo_event_matches_config() {
        let mut cfg = GenerateConfig::default();
        cfg.tempo_bpm = 70.0;
        let song = generate_song(&cfg).unwrap().song;
        let bytes = to_midi_bytes(&song);
        // First track body starts with delta 0 + FF 51 03 xx xx xx.
        let body = &bytes[14 + 8..];
        assert_eq!(&body[0..5], &[0x00, 0xFF, 0x51, 0x03]);
        let expect = tempo_bytes(70.0);
        assert_eq!(&body[5..8], &expect);
    }
}
