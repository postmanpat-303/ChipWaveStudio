use super::*;

fn render(song: &Song, secs: f32) -> Vec<f32> {
    let mut e = crate::engine::Engine::new(48000.0);
    e.start(song, 0, 0, false, 0);
    let mut out = vec![0.0f32; (secs * 48000.0) as usize * 2];
    for c in out.chunks_mut(512) {
        e.render(song, c);
    }
    out
}

fn max_diff(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f32::max)
}

fn peak(a: &[f32]) -> f32 {
    a.iter().fold(0.0f32, |m, x| m.max(x.abs()))
}

fn timeline_song(i: usize) -> Song {
    let mut s = crate::fileio::demo_song_n(i);
    let (tl, _) = crate::timeline::from_order(&s);
    s.timeline = Some(Box::new(tl));
    s
}

#[test]
fn reorder_keeps_playback_tracker_and_timeline() {
    for timeline in [false, true] {
        let mut song = if timeline {
            timeline_song(0)
        } else {
            crate::fileio::demo_song_n(0)
        };
        // a channel-targeted lane and distinct mixer settings follow the move
        song.mixer.channels[0].pan = -0.6;
        song.mixer.channels[1].volume = 0.5;
        song.automation.push(crate::automation::Lane::new(
            Target::ChanVolume(1),
            "v".into(),
        ));
        let a = render(&song, 6.0);
        assert!(peak(&a) > 0.01, "demo is silent");
        let n = song.channels;
        let name0 = song.channel_names[0].clone();
        assert!(move_channel(&mut song, 0, n - 1));
        assert!(move_channel(&mut song, 1, 0));
        assert_eq!(song.channel_names[n - 1], name0);
        assert_eq!(song.mixer.channels[n - 1].pan, -0.6);
        assert_eq!(
            song.automation.last().unwrap().target,
            Target::ChanVolume(1).id()
        );
        assert_eq!(song.mixer.channels[1].volume, 0.5);
        if let Some(tl) = &song.timeline {
            assert_eq!(tl.tracks.len(), n);
            assert!(tl.clips.iter().all(|c| (c.track as usize) < n));
        }
        let b = render(&song, 6.0);
        // only the summing order of the channels differs
        let d = max_diff(&a, &b);
        assert!(
            d < 1e-4,
            "timeline {timeline}: reorder changed playback by {d}"
        );
    }
}

#[test]
fn insert_channel_is_silent_and_respects_the_limit() {
    for timeline in [false, true] {
        let mut song = if timeline {
            timeline_song(1)
        } else {
            crate::fileio::demo_song_n(1)
        };
        let a = render(&song, 4.0);
        let n = song.channels;
        assert!(insert_channel(&mut song, 1, "Bass 2"));
        assert_eq!(song.channels, n + 1);
        assert_eq!(song.channel_names[1], "Bass 2");
        assert!(song
            .patterns
            .iter()
            .all(|p| p.cells.iter().all(|r| r.len() == n + 1)));
        let b = render(&song, 4.0);
        assert!(max_diff(&a, &b) < 1e-4);
        while song.channels < MAX_CHANNELS {
            let at = song.channels;
            assert!(insert_channel(&mut song, at, "x"));
        }
        assert!(!insert_channel(&mut song, 0, "full"));
        assert_eq!(song.channels, MAX_CHANNELS);
    }
}

#[test]
fn assign_instrument_rewrites_the_tracks_notes() {
    let mut song = crate::fileio::demo_song_n(0);
    let changed = assign_track_instrument(&mut song, 0, 2);
    assert!(changed > 0);
    for p in &song.patterns {
        for r in &p.cells {
            if (1..=120).contains(&r[0].note) {
                assert_eq!(r[0].instr, 2);
            }
        }
    }
    let mut song = timeline_song(0);
    let changed = assign_track_instrument(&mut song, 0, 2);
    assert!(changed > 0);
    assert_eq!(song.timeline.as_ref().unwrap().tracks[0].instr, 2);
}

/// The Song / Master track (tempo, master and return lanes, after the
/// channels) survives a reorder or an insert and keeps its lanes, so the
/// music still plays the same.
#[test]
fn reorder_and_insert_keep_the_song_master_track() {
    use crate::timeline::TrackKind;
    let mut s = crate::fileio::demo_song_n(0);
    crate::timeline::enter_timeline_mode(&mut s); // song lanes -> tracks
    let lanes = |s: &Song| {
        let tl = s.timeline.as_deref().unwrap();
        let i = tl.tracks.iter().position(|t| t.kind == TrackKind::Master);
        i.map(|i| (i, tl.tracks[i].auto.len()))
    };
    let (mi, ml) = lanes(&s).expect("demo has a Song / Master track");
    assert_eq!(mi, s.channels);
    assert!(ml > 0, "the demo's return lane lives on Song / Master");
    let a = render(&s, 4.0);
    let mut m = s.clone();
    assert!(move_channel(&mut m, 0, 2));
    assert_eq!(lanes(&m), Some((m.channels, ml)));
    assert!(max_diff(&a, &render(&m, 4.0)) < 1e-4);
    assert!(insert_channel(&mut m, 1, "New"));
    assert_eq!(lanes(&m), Some((m.channels, ml)));
    assert_eq!(m.timeline.as_deref().unwrap().tracks.len(), m.channels + 1);
}
