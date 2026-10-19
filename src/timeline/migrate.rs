//! Migrate song-scope automation onto timeline tracks (design A §3.6).
//! Playback stays bit-exact: the same points move from `song.automation` onto
//! a track lane (or the Song/Master track); the engine still evaluates them.
use super::{Timeline, Track, TrackKind};
use crate::automation::{Lane, Target};
use crate::model::{Song, MAX_CHANNELS};

/// Ensure a Song/Master track exists at the end (after mixer channels).
pub fn ensure_master_track(tl: &mut Timeline, channels: usize) -> usize {
    let nch = channels.clamp(1, MAX_CHANNELS);
    if let Some(i) = tl
        .tracks
        .iter()
        .enumerate()
        .find(|(i, t)| *i >= nch && t.kind == TrackKind::Master)
        .map(|(i, _)| i)
    {
        return i;
    }
    while tl.tracks.len() < nch {
        let i = tl.tracks.len();
        tl.tracks.push(Track {
            name: format!("Ch {}", i + 1),
            ..Default::default()
        });
    }
    tl.tracks.push(Track {
        name: "Song / Master".into(),
        kind: TrackKind::Master,
        ..Default::default()
    });
    tl.tracks.len() - 1
}

fn is_master_target(t: Target) -> bool {
    matches!(
        t,
        Target::Master | Target::Tempo | Target::Ret(..) | Target::ModAmount(_)
    )
}

/// Which mixer track should own a song lane for `t` (not Master).
fn owner_mixer_track(song: &Song, tl: &Timeline, t: Target) -> usize {
    let nch = song.channels.clamp(1, MAX_CHANNELS);
    match t {
        Target::ChanVolume(c) | Target::ChanPan(c) | Target::Send(c, _) => c.min(nch - 1),
        Target::Cutoff(i)
        | Target::Resonance(i)
        | Target::Drive(i)
        | Target::Level(i)
        | Target::Fx(i, ..)
        | Target::Macro(i, _)
        | Target::Gen(i, _)
        | Target::Patch(i, _) => {
            let slot = (i + 1) as u8;
            if let Some(tr) = tl.tracks.iter().take(nch).position(|x| x.instr == slot) {
                return tr;
            }
            let mut counts = vec![0usize; nch];
            for p in &song.patterns {
                for row in &p.cells {
                    for (tr, c) in row.iter().enumerate().take(nch) {
                        if c.instr == slot {
                            counts[tr] += 1;
                        }
                    }
                }
            }
            counts
                .iter()
                .enumerate()
                .max_by_key(|(_, n)| *n)
                .filter(|(_, n)| **n > 0)
                .map(|(tr, _)| tr)
                .unwrap_or(0)
        }
        _ => 0,
    }
}

/// Move every song lane onto its track (or Song/Master). Returns how many
/// lanes moved. Idempotent; leaves unparseable lanes on the song.
pub fn migrate_song_lanes(song: &mut Song) -> usize {
    if song.timeline.is_none() || song.automation.is_empty() {
        return 0;
    }
    let nch = song.channels.clamp(1, MAX_CHANNELS);
    // (target_id, Target, Lane)
    let pending: Vec<(String, Target, Lane)> = song
        .automation
        .iter()
        .filter_map(|l| {
            let t = Target::parse_in(&l.target, song)?;
            Some((l.target.clone(), t, l.clone()))
        })
        .collect();
    if pending.is_empty() {
        return 0;
    }

    let mut moved_ids = Vec::new();
    let mut moved = 0usize;
    // owners with immutable borrows
    let owners: Vec<(usize, String, Lane)> = {
        let tl = song.timeline.as_deref().unwrap();
        pending
            .into_iter()
            .map(|(id, t, lane)| {
                let tr = if is_master_target(t) {
                    // master index after ensure — compute without mut
                    tl.tracks
                        .iter()
                        .enumerate()
                        .find(|(i, x)| *i >= nch && x.kind == TrackKind::Master)
                        .map(|(i, _)| i)
                        .unwrap_or(nch) // ensure will create it
                } else {
                    owner_mixer_track(song, tl, t)
                };
                (tr, id, lane)
            })
            .collect()
    };

    let tl = song.timeline.as_deref_mut().unwrap();
    let master = ensure_master_track(tl, nch);
    for (tr0, id, lane) in owners {
        let tr = if tr0 >= nch || tl.tracks.get(tr0).map(|t| t.kind) == Some(TrackKind::Master) {
            master
        } else {
            tr0
        };
        if let Some(track) = tl.tracks.get_mut(tr) {
            if !track.auto.iter().any(|l| l.target == lane.target) {
                track.auto.push(lane);
                moved += 1;
            }
            moved_ids.push(id);
        }
    }
    song.automation
        .retain(|l| !moved_ids.iter().any(|id| id == &l.target));
    moved
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automation::{Point, Shape};
    use crate::timeline::enter_timeline_mode;

    fn lane(t: Target, v: f32) -> Lane {
        let mut l = Lane::new(t, t.param_name().into());
        l.points.push(Point {
            pos: 0.0,
            value: v,
            shape: Shape::Linear,
            curve: 0.0,
        });
        l.points.push(Point {
            pos: 16.0,
            value: (v + 0.3).min(1.0),
            shape: Shape::Linear,
            curve: 0.0,
        });
        l
    }

    #[test]
    fn migrate_moves_mixer_and_tempo_lanes() {
        let mut song = crate::fileio::demo_song();
        song.automation.clear();
        song.automation.push(lane(Target::ChanVolume(1), 0.4));
        song.automation.push(lane(Target::Tempo, 0.5));
        enter_timeline_mode(&mut song); // migrates song lanes
        let n = migrate_song_lanes(&mut song); // idempotent
        assert_eq!(n, 0, "already migrated on enter");
        assert!(
            song.automation.is_empty(),
            "song lanes cleared: {:?}",
            song.automation
                .iter()
                .map(|l| &l.target)
                .collect::<Vec<_>>()
        );
        let tl = song.timeline.as_deref().unwrap();
        assert!(
            tl.tracks[1]
                .auto
                .iter()
                .any(|l| l.target.contains("volume")),
            "{:?}",
            tl.tracks[1].auto
        );
        let master = tl.tracks.iter().find(|t| t.kind == TrackKind::Master);
        assert!(master.is_some());
        assert!(
            master.unwrap().auto.iter().any(|l| l.target == "tempo"),
            "{:?}",
            master.unwrap().auto
        );
    }

    #[test]
    fn migrate_is_idempotent_and_keeps_points() {
        let mut song = crate::fileio::demo_song();
        song.automation.clear();
        let l = lane(Target::ChanPan(0), 0.2);
        let pts = l.points.clone();
        song.automation.push(l);
        // build timeline without the enter helper so we can test migrate alone
        let (tl, _) = crate::timeline::from_order(&song);
        song.timeline = Some(Box::new(tl));
        assert_eq!(migrate_song_lanes(&mut song), 1);
        assert_eq!(migrate_song_lanes(&mut song), 0);
        let got = &song.timeline.as_ref().unwrap().tracks[0].auto[0].points;
        assert_eq!(got.len(), pts.len());
        for (a, b) in got.iter().zip(&pts) {
            assert_eq!(a.pos.to_bits(), b.pos.to_bits());
            assert_eq!(a.value.to_bits(), b.value.to_bits());
        }
    }
}
