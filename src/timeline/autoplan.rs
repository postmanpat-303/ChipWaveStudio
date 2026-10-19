//! Track and clip automation compiled for the engine (M5).
//!
//! The UI thread turns `Track::auto` and `Clip::auto` into a flat, immutable
//! [`AutoPlan`]: one layer per enabled lane, its points copied, its target
//! parsed and given a slot. The engine evaluates the layers every 32 frames
//! without allocating. The innermost lane wins: a clip lane over the track
//! lane over the song lane (`Song::automation`), per target.
use std::hash::{Hash, Hasher};

use super::{Clip, Tick, Timeline, ROW};
use crate::automation::{value_at, Lane, Point, Target};
use crate::model::{Song, MAX_CHANNELS};

/// One enabled lane on a track or a clip.
#[derive(Clone, Debug, PartialEq)]
pub struct AutoLayer {
    /// index into `AutoPlan::targets`
    pub slot: u32,
    /// a clip lane (positions relative to the clip's source)
    pub clip: bool,
    /// clip span on the timeline (track lanes: the whole song)
    pub start: Tick,
    pub end: Tick,
    pub offset: Tick,
    /// clip loop length (0 = no loop)
    pub loop_len: Tick,
    /// points in `AutoPlan::points[p0..p1]`
    pub p0: u32,
    pub p1: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AutoPlan {
    pub key: u64,
    /// the distinct targets the layers drive
    pub targets: Vec<Target>,
    /// track layers first (track order), then clip layers by clip start:
    /// a later layer for the same slot wins
    pub layers: Vec<AutoLayer>,
    pub points: Vec<Point>,
}

/// can a track or clip lane drive this target (tempo is song-wide)
pub fn track_target(t: Target) -> bool {
    !matches!(t, Target::Tempo)
}

impl AutoLayer {
    /// Normalised value at timeline tick `t` (fractional), None when a clip
    /// layer is not playing there. No allocation.
    pub fn value(&self, points: &[Point], t: f64) -> Option<f32> {
        let pts = &points[self.p0 as usize..self.p1 as usize];
        if !self.clip {
            return value_at(pts, (t / ROW as f64) as f32);
        }
        if t < self.start as f64 || t >= self.end as f64 {
            return None;
        }
        let mut s = self.offset as f64 + (t - self.start as f64);
        if self.loop_len > 0 {
            s = s.rem_euclid(self.loop_len as f64);
        }
        value_at(pts, (s / ROW as f64) as f32)
    }
}

impl AutoPlan {
    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    /// The innermost raw value (no smoothing) of slot `k` at tick `t`.
    pub fn value(&self, k: usize, t: f64) -> Option<f32> {
        let mut v = None;
        for l in self.layers.iter().filter(|l| l.slot as usize == k) {
            if let Some(x) = l.value(&self.points, t) {
                v = Some(x);
            }
        }
        v
    }

    /// slot of `t`
    pub fn slot(&self, t: Target) -> Option<usize> {
        self.targets.iter().position(|x| *x == t)
    }
}

fn hash_lane(l: &Lane, h: &mut impl Hasher) {
    (l.target.as_str(), l.enabled, l.points.len()).hash(h);
    for p in &l.points {
        (
            p.pos.to_bits(),
            p.value.to_bits(),
            p.shape as u8,
            p.curve.to_bits(),
        )
            .hash(h);
    }
}

fn clip_has_auto(c: &Clip) -> bool {
    !c.auto.is_empty()
}

/// Cheap key of everything [`build_auto`] reads.
pub fn auto_key(song: &Song, tl: &Timeline) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    song.channels.hash(&mut h);
    for (i, t) in tl.tracks.iter().enumerate() {
        if !t.auto.is_empty() {
            i.hash(&mut h);
            for l in &t.auto {
                hash_lane(l, &mut h);
            }
        }
    }
    for c in tl.clips.iter().filter(|c| clip_has_auto(c)) {
        (c.id, c.track, c.start, c.len, c.offset, c.loop_len, c.muted).hash(&mut h);
        for l in &c.auto {
            hash_lane(l, &mut h);
        }
    }
    // FX parameter names resolve against the instruments' devices
    for i in &song.instruments {
        i.fx.len().hash(&mut h);
        for d in &i.fx {
            (d.kind as u8).hash(&mut h);
        }
    }
    h.finish()
}

/// Compile the track and clip lanes of `tl` (UI thread).
pub fn build_auto(song: &Song, tl: &Timeline) -> AutoPlan {
    let nch = song.channels.clamp(1, MAX_CHANNELS);
    let mut p = AutoPlan {
        key: auto_key(song, tl),
        ..Default::default()
    };
    let add = |p: &mut AutoPlan, lane: &Lane, clip: Option<&Clip>| {
        if !lane.enabled || lane.points.is_empty() {
            return;
        }
        let Some(t) = Target::parse_in(&lane.target, song).filter(|t| track_target(*t)) else {
            return;
        };
        let slot = match p.slot(t) {
            Some(k) => k,
            None => {
                p.targets.push(t);
                p.targets.len() - 1
            }
        } as u32;
        let p0 = p.points.len() as u32;
        let mut pts = lane.points.clone();
        pts.sort_by(|a, b| {
            a.pos
                .partial_cmp(&b.pos)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        p.points.extend(pts);
        let p1 = p.points.len() as u32;
        p.layers.push(match clip {
            None => AutoLayer {
                slot,
                clip: false,
                start: Tick::MIN,
                end: Tick::MAX,
                offset: 0,
                loop_len: 0,
                p0,
                p1,
            },
            Some(c) => AutoLayer {
                slot,
                clip: true,
                start: c.start,
                end: c.end(),
                offset: c.offset,
                loop_len: c.loop_len.filter(|l| *l > 0).unwrap_or(0),
                p0,
                p1,
            },
        });
    };
    for t in tl.tracks.iter().take(nch) {
        for l in &t.auto {
            add(&mut p, l, None);
        }
    }
    let mut clips: Vec<&Clip> = tl
        .clips
        .iter()
        .filter(|c| clip_has_auto(c) && !c.muted && (c.track as usize) < nch && c.len > 0)
        .collect();
    // the clip that starts later wins where clips overlap
    clips.sort_by_key(|c| (c.start, c.track, c.id));
    for c in clips {
        for l in &c.auto {
            add(&mut p, l, Some(c));
        }
    }
    p
}

/// Track and clip automation as song lanes, for tracker mode: every target
/// is sampled at its breakpoints, clip edges and each row in between where
/// a curve or clip changes it, then straight runs are merged. Song lanes
/// with the same target are replaced by the combined (innermost) value.
pub fn bake_into_song(song: &mut Song, tl: &Timeline) {
    let plan = build_auto(song, tl);
    if plan.is_empty() {
        return;
    }
    // the song's end, or the last breakpoint if a track lane goes on
    let last = plan
        .points
        .iter()
        .map(|p| (p.pos.max(0.0) as f64 * ROW as f64).ceil() as Tick)
        .max()
        .unwrap_or(0);
    let end = super::play_end(tl).max(last).max(ROW);
    for (k, &t) in plan.targets.iter().enumerate() {
        let id = t.id();
        // the song lane (if any) is the outermost layer
        let song_lane = song
            .automation
            .iter()
            .position(|l| l.enabled && Target::parse_in(&l.target, song) == Some(t));
        let outer: Vec<Point> = song_lane
            .map(|i| song.automation[i].points.clone())
            .unwrap_or_default();
        let at = |tick: f64| -> Option<f32> {
            plan.value(k, tick)
                .or_else(|| value_at(&outer, (tick / ROW as f64) as f32))
        };
        let mut pts: Vec<Point> = vec![];
        let push = |pos: f32, v: f32, pts: &mut Vec<Point>| {
            if let Some(l) = pts.last() {
                if (l.pos - pos).abs() < 1e-4 {
                    return;
                }
            }
            pts.push(Point {
                pos,
                value: v,
                shape: crate::automation::Shape::Linear,
                curve: 0.0,
            });
        };
        // sample every row, plus just before each clip edge so a jump stays
        // a jump
        let mut edges: Vec<Tick> = plan
            .layers
            .iter()
            .filter(|l| l.slot as usize == k && l.clip)
            .flat_map(|l| [l.start, l.end])
            .collect();
        edges.sort_unstable();
        let mut e = 0;
        let mut r: Tick = 0;
        while r <= end {
            while e < edges.len() && edges[e] <= r {
                let x = edges[e] as f64;
                if let Some(v) = at(x - 1.0) {
                    push(((x - 1.0) / ROW as f64) as f32, v, &mut pts);
                }
                e += 1;
            }
            if let Some(v) = at(r as f64) {
                push((r / ROW) as f32, v, &mut pts);
            }
            r += ROW;
        }
        // drop points in the middle of straight runs
        let mut out: Vec<Point> = vec![];
        for p in pts {
            if out.len() >= 2 {
                let a = out[out.len() - 2];
                let b = out[out.len() - 1];
                let f = (b.pos - a.pos) / (p.pos - a.pos).max(1e-6);
                if (a.value + (p.value - a.value) * f - b.value).abs() < 1e-5 {
                    out.pop();
                }
            }
            out.push(p);
        }
        let mut lane = Lane::new(t, String::new());
        lane.target = id;
        lane.label = song_lane
            .map(|i| song.automation[i].label.clone())
            .unwrap_or_default();
        lane.points = out;
        match song_lane {
            Some(i) => song.automation[i] = lane,
            None => song.automation.push(lane),
        }
    }
}
