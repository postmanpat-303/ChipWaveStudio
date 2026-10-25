//! Follow actions (docs/ROUND_ROBIN.md): a Clip Arranger clip that, after
//! N loops, moves on by itself: "after 4 loops: 75 % Next, else Again".
//!
//! Like the rest of Round Robin it is a pure function of the clip's seed
//! and the decision number (counted per track from the launch), so a
//! captured run on the timeline is exactly what the launcher plays. Off by
//! default and not written to files while off.
use super::rr::SALT_FOLLOW;
use super::*;

/// What a clip does when its loops are up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Act {
    /// the clip in the next scene row on this track (wraps to the first)
    #[default]
    Next,
    /// play this clip again
    Again,
    /// any other clip on this track
    Other,
    /// the track's first clip
    First,
    /// the track's last clip
    Last,
    /// any clip on this track, this one included
    Any,
    /// stop the track
    Stop,
}

impl Act {
    pub const ALL: [Act; 7] = [
        Act::Next,
        Act::Again,
        Act::Other,
        Act::First,
        Act::Last,
        Act::Any,
        Act::Stop,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Act::Next => "Next",
            Act::Again => "Again",
            Act::Other => "Other",
            Act::First => "First",
            Act::Last => "Last",
            Act::Any => "Any",
            Act::Stop => "Stop",
        }
    }
    pub fn tip(self) -> &'static str {
        match self {
            Act::Next => {
                "the clip in the next scene row on this track (after the last row: the first)"
            }
            Act::Again => "this clip again",
            Act::Other => "any other clip on this track, picked at random",
            Act::First => "the first clip on this track",
            Act::Last => "the last clip on this track",
            Act::Any => "any clip on this track (this one too), picked at random",
            Act::Stop => "the track stops",
        }
    }
}

/// A clip's follow action: after `after` loops, `a` with `chance` %, else `b`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Follow {
    pub after: u16,
    pub a: Act,
    pub b: Act,
    /// % for `a` (0..=100)
    pub chance: u8,
}

impl Default for Follow {
    fn default() -> Self {
        Follow {
            after: 4,
            a: Act::Next,
            b: Act::Again,
            chance: 100,
        }
    }
}

impl Follow {
    /// one line for tiles and tooltips: "after 4 loops: 75 % Next, else Again"
    pub fn describe(&self) -> String {
        let n = self.after.max(1);
        let loops = if n == 1 { "loop" } else { "loops" };
        if self.chance >= 100 || self.a == self.b {
            format!("after {n} {loops}: {}", self.a.label())
        } else if self.chance == 0 {
            format!("after {n} {loops}: {}", self.b.label())
        } else {
            format!(
                "after {n} {loops}: {} % {}, else {}",
                self.chance,
                self.a.label(),
                self.b.label()
            )
        }
    }
}

/// decision `k` of a clip with seed `seed`: the action, and a draw for the
/// random targets. Pure.
pub fn decide(f: &Follow, seed: u32, k: u32) -> (Act, u32) {
    let mut r = ClipRng::new(seed ^ SALT_FOLLOW, k);
    let u = r.next_u32() % 100;
    let act = if u < f.chance.min(100) as u32 {
        f.a
    } else {
        f.b
    };
    (act, r.next_u32())
}

/// The clip a track plays after `act` from clip `cur` in scene row
/// `scene`: `slots` are the (id, scene) of the track's launcher clips.
/// None: the track stops. Allocation-free (the engine calls it).
pub fn target<I>(act: Act, cur: u32, scene: u16, slots: I, draw: u32) -> Option<u32>
where
    I: Iterator<Item = (u32, u16)> + Clone,
{
    let min = |it: I| it.min_by_key(|s| s.1).map(|s| s.0);
    match act {
        Act::Stop => None,
        Act::Again => Some(cur),
        Act::Next => slots
            .clone()
            .filter(|s| s.1 > scene)
            .min_by_key(|s| s.1)
            .map(|s| s.0)
            .or_else(|| min(slots))
            .or(Some(cur)),
        Act::First => min(slots).or(Some(cur)),
        Act::Last => slots.max_by_key(|s| s.1).map(|s| s.0).or(Some(cur)),
        Act::Other => {
            let n = slots.clone().filter(|s| s.0 != cur).count();
            if n == 0 {
                return Some(cur);
            }
            slots
                .filter(|s| s.0 != cur)
                .nth(draw as usize % n)
                .map(|s| s.0)
        }
        Act::Any => {
            let n = slots.clone().count();
            if n == 0 {
                return Some(cur);
            }
            slots.clone().nth(draw as usize % n).map(|s| s.0)
        }
    }
}

/// What track `track` plays from launching clip `start`, for `limit`
/// ticks: (clip id, ticks) runs, the engine's own decisions in order.
pub fn run(plan: &[LaunchNotes], start: u32, limit: Tick) -> Vec<(u32, Tick)> {
    let mut out: Vec<(u32, Tick)> = vec![];
    let Some(track) = plan.iter().find(|l| l.id == start).map(|l| l.track) else {
        return out;
    };
    let (mut cur, mut t, mut k) = (start, 0, 0u32);
    while t < limit {
        let Some(ln) = plan.iter().find(|l| l.id == cur) else {
            break;
        };
        let span = ln
            .follow
            .map(|f| ln.loop_len * f.after.max(1) as Tick)
            .filter(|s| *s > 0);
        let dur = span.unwrap_or(limit - t).min(limit - t);
        match out.last_mut() {
            Some(l) if l.0 == cur => l.1 += dur,
            _ => out.push((cur, dur)),
        }
        t += dur;
        let Some(f) = ln.follow.filter(|_| t < limit) else {
            break;
        };
        let (act, draw) = decide(&f, ln.seed, k);
        k += 1;
        let slots = plan
            .iter()
            .filter(|x| x.track == track)
            .map(|x| (x.id, x.scene));
        match target(act, cur, ln.scene, slots, draw) {
            Some(id) => cur = id,
            None => break,
        }
    }
    out
}

/// Capture to timeline: what each track of scene row `scene` plays when
/// the scene is launched, for `limit` ticks, as timeline clips from `at`
/// (linked to the launcher clips' notes, with their seeds, so chance and
/// variations play the same takes). Returns the new clips' ids.
pub fn capture(tl: &mut Timeline, scene: usize, at: Tick, limit: Tick) -> Vec<u32> {
    let plan = launch_plan(tl);
    let starts: Vec<u32> = tl
        .launcher
        .iter()
        .filter(|l| l.scene as usize == scene && l.clip.notes().is_some() && l.clip.len > 0)
        .map(|l| l.clip.id)
        .collect();
    let mut ids = vec![];
    for s in starts {
        let mut t = at;
        for (id, dur) in run(&plan, s, limit) {
            let Some(src) = tl.launch_clip(id).map(|l| l.clip.clone()) else {
                break;
            };
            // one clip that loops the launcher clip, when it can; else one
            // clip per pass
            let whole = src
                .loop_len
                .filter(|l| *l > 0)
                .is_none_or(|l| src.len % l == 0);
            let pieces: Vec<(Tick, Tick)> = if whole {
                vec![(t, dur)]
            } else {
                (0..dur.div_euclid(src.len.max(1)).max(1))
                    .map(|i| (t + i * src.len, src.len.min(dur - i * src.len)))
                    .collect()
            };
            for (start, len) in pieces {
                let nid = tl.new_id();
                let mut c = src.clone();
                c.id = nid;
                c.start = start;
                c.len = len;
                if whole && src.loop_len.is_none() && len > src.len {
                    c.loop_len = Some(src.len);
                }
                tl.clips.push(c);
                ids.push(nid);
            }
            t += dur;
        }
    }
    tl.sort();
    ids
}
