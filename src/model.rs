//! Song data model: patterns of cells, an order list, instruments.
use serde::{Deserialize, Serialize};

use crate::synth::Patch;

pub const NOTE_OFF: u8 = 255;
pub const ROWS: usize = 64;
pub const MAX_CHANNELS: usize = 8;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    /// 0 = empty, 1..=120 = note (1 = C-0), 255 = note off
    pub note: u8,
    /// 0 = none, 1..=255 instrument slot
    pub instr: u8,
    /// 0 = none, 1..=65 => volume 0x00..0x40 (+1)
    pub vol: u8,
    pub fx: u8,
    pub fxp: u8,
}

impl Cell {
    pub fn is_empty(&self) -> bool {
        *self == Cell::default()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pattern {
    pub name: String,
    pub rows: usize,
    /// cells[row][channel]
    pub cells: Vec<Vec<Cell>>,
}

impl Pattern {
    pub fn new(name: &str, rows: usize, channels: usize) -> Self {
        Self {
            name: name.into(),
            rows,
            cells: vec![vec![Cell::default(); channels]; rows],
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum InstrumentKind {
    Synth(Patch),
    /// mono f32 sample data at `rate`
    Sample {
        file: String,
        rate: u32,
        #[serde(skip)]
        data: Vec<f32>,
        base_note: u8,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Instrument {
    pub name: String,
    pub volume: f32,
    pub kind: InstrumentKind,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Song {
    pub title: String,
    pub bpm: f32,
    /// ticks per row
    pub speed: u32,
    pub lines_per_beat: u32,
    pub channels: usize,
    pub channel_names: Vec<String>,
    pub patterns: Vec<Pattern>,
    pub order: Vec<usize>,
    /// index 0 => instrument slot 1
    pub instruments: Vec<Instrument>,
}

impl Song {
    pub fn empty() -> Self {
        Self {
            title: "Untitled".into(),
            bpm: 125.0,
            speed: 6,
            lines_per_beat: 4,
            channels: 4,
            channel_names: (1..=4).map(|i| format!("Ch {i}")).collect(),
            patterns: vec![Pattern::new("Block 00", ROWS, 4)],
            order: vec![0],
            instruments: vec![],
        }
    }

    /// seconds per row: 4 rows per beat at the given BPM (speed only subdivides ticks)
    pub fn row_seconds(&self) -> f64 {
        60.0 / (self.bpm as f64 * self.lines_per_beat.max(1) as f64)
    }

    pub fn length_seconds(&self) -> f64 {
        let rows: usize = self
            .order
            .iter()
            .filter_map(|&p| self.patterns.get(p))
            .map(|p| p.rows)
            .sum();
        rows as f64 * self.row_seconds()
    }
}

pub fn note_name(n: u8) -> String {
    match n {
        0 => "---".into(),
        NOTE_OFF => "===".into(),
        n => {
            const NAMES: [&str; 12] = [
                "C-", "C#", "D-", "D#", "E-", "F-", "F#", "G-", "G#", "A-", "A#", "B-",
            ];
            let i = (n - 1) as usize;
            format!("{}{}", NAMES[i % 12], i / 12)
        }
    }
}

pub fn parse_note(s: &str) -> Option<u8> {
    let s = s.trim();
    if s == "---" || s.is_empty() {
        return Some(0);
    }
    if s == "===" || s == "^^^" || s == "OFF" {
        return Some(NOTE_OFF);
    }
    let b = s.as_bytes();
    if b.len() != 3 {
        return None;
    }
    let base = match b[0] {
        b'C' => 0,
        b'D' => 2,
        b'E' => 4,
        b'F' => 5,
        b'G' => 7,
        b'A' => 9,
        b'B' => 11,
        _ => return None,
    };
    let sharp = if b[1] == b'#' { 1 } else { 0 };
    let oct = (b[2] as char).to_digit(10)? as u8;
    Some(oct * 12 + base + sharp + 1)
}

/// MIDI-like frequency for our note numbering (C-4 = note 49 = 261.63 Hz)
pub fn note_freq(n: u8) -> f32 {
    let midi = n as f32 - 1.0 + 12.0; // C-0 => midi 12
    440.0 * 2f32.powf((midi - 69.0) / 12.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn note_roundtrip() {
        for n in 1..=96u8 {
            assert_eq!(parse_note(&note_name(n)), Some(n));
        }
        assert!((note_freq(parse_note("A-4").unwrap()) - 440.0).abs() < 0.01);
    }
}
