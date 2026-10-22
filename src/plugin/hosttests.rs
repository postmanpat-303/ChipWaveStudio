//! CLAP host behaviour around plugin editors (claphost branch): editor
//! edits reach the playing instances, request_flush, state capture, seeding
//! and the editor scale. Uses the Test Synth fixture's simulated editor
//! (`cw_test_gui_edit`, see plugins/cw-test-synth).
use super::tests::{fixture, patch, TEST_ID};
use super::*;

/// the fixture's "editor" sets parameter `id` of the instance behind `h`
pub fn gui_edit(h: *const std::ffi::c_void, id: u32, value: f64) {
    assert!(!h.is_null(), "not a CLAP instance");
    unsafe {
        let lib = libloading::Library::new(fixture()).unwrap();
        let f: libloading::Symbol<unsafe extern "C" fn(*const std::ffi::c_void, u32, f64)> =
            lib.get(b"cw_test_gui_edit").unwrap();
        f(h, id, value);
    }
}

pub fn new_inst() -> Box<dyn Hosted> {
    Box::new(clap::ClapInst::new(&fixture(), TEST_ID, 48000.0).unwrap())
}

/// a rack with instrument 0 (the fixture) on channels 0 and 1
pub fn two_channel_rack() -> Rack {
    let mut rack = Rack::new();
    let p = patch();
    rack.insert(0, 0, &p, new_inst());
    rack.insert(0, 1, &p, new_inst());
    assert_eq!(rack.primary(0), Some(0));
    rack
}

#[test]
fn editor_edits_on_the_primary_reach_the_playing_instances() {
    let mut rack = two_channel_rack();
    // notes held on both channels keep the instances awake
    rack.note(0, 0, 60, 1.0);
    rack.note(0, 1, 64, 1.0);
    let h = rack.slots[0].inst.test_handle();
    gui_edit(h, 1, 0.9);
    rack.chunk(0, 64);
    assert_eq!(
        rack.slots[0].inst.value(1),
        Some(0.9),
        "the editor's own instance"
    );
    assert_eq!(
        rack.slots[1].inst.value(1),
        Some(0.5),
        "queued for the sibling"
    );
    rack.chunk(1, 64);
    assert_eq!(
        rack.slots[1].inst.value(1),
        Some(0.9),
        "the sibling playing channel 1 follows the editor"
    );
    let p = rack.poll_main();
    assert_eq!(
        p.events,
        vec![
            (0, OutEv::Begin(1)),
            (0, OutEv::Value(1, 0.9)),
            (0, OutEv::End(1))
        ]
    );
    assert_eq!(p.dirty, vec![0], "an editor edit makes the song dirty");
    assert!(
        rack.poll_main().events.is_empty(),
        "events are handed over once"
    );
}

#[test]
fn a_secondary_instance_never_echoes_into_the_primary() {
    let mut rack = two_channel_rack();
    rack.note(0, 0, 60, 1.0);
    rack.note(0, 1, 64, 1.0);
    let h = rack.slots[1].inst.test_handle();
    gui_edit(h, 2, 1.0);
    rack.chunk(1, 64);
    rack.chunk(0, 64);
    assert_eq!(rack.slots[0].inst.value(2), Some(0.0));
    assert!(rack.poll_main().events.is_empty());
}

#[test]
fn a_long_drag_is_merged_and_never_allocates() {
    let mut rack = two_channel_rack();
    rack.note(0, 0, 60, 1.0);
    let h = rack.slots[0].inst.test_handle();
    // warm up once
    gui_edit(h, 1, 0.1);
    rack.chunk(0, 64);
    let _ = rack.poll_main();
    for k in 0..300 {
        gui_edit(h, 1, k as f64 / 300.0);
        let n = crate::engine::alloc_count::count(|| rack.chunk(0, 64));
        assert_eq!(n, 0, "forwarding editor edits allocated");
    }
    let p = rack.poll_main();
    assert_eq!(
        p.events.len(),
        3,
        "one gesture: {:?}",
        &p.events[..p.events.len().min(6)]
    );
    assert_eq!(rack.slots[1].inst.value(1), Some(0.5), "still queued");
    rack.chunk(1, 64);
    let v = rack.slots[1].inst.value(1).unwrap();
    assert!((v - 299.0 / 300.0).abs() < 1e-9, "{v}");
}

#[test]
fn request_flush_applies_editor_edits_on_an_idle_instance() {
    let mut rack = two_channel_rack();
    // never processed (no audio running): the host flushes on the main thread
    let h = rack.slots[0].inst.test_handle();
    gui_edit(h, 1, 0.8);
    assert_eq!(rack.slots[0].inst.value(1), Some(0.5), "waits for a flush");
    let p = rack.poll_main();
    assert_eq!(rack.slots[0].inst.value(1), Some(0.8), "flushed while idle");
    assert_eq!(rack.slots[1].inst.value(1), Some(0.8), "sibling follows");
    assert_eq!(p.events.len(), 3);
    assert_eq!(p.dirty, vec![0]);
}

#[test]
fn request_flush_wakes_a_sleeping_instance() {
    let mut rack = two_channel_rack();
    // processing started, no notes: the plugin went to sleep
    rack.chunk(0, 64);
    rack.chunk(1, 64);
    let h = rack.slots[0].inst.test_handle();
    gui_edit(h, 2, 0.6);
    let _ = rack.poll_main();
    rack.chunk(0, 64);
    assert_eq!(
        rack.slots[0].inst.value(2),
        Some(0.6),
        "the next block runs the plugin"
    );
    rack.chunk(1, 64);
    assert_eq!(rack.slots[1].inst.value(2), Some(0.6));
    assert_eq!(rack.poll_main().dirty, vec![0]);
}

/// a real instance that counts `load_state` calls
struct Counting {
    inner: Box<dyn Hosted>,
    loads: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl Hosted for Counting {
    fn note_on(&mut self, key: u8, vel: f32) {
        self.inner.note_on(key, vel)
    }
    fn note_off(&mut self, key: u8) {
        self.inner.note_off(key)
    }
    fn tuning(&mut self, key: u8, semis: f32) {
        self.inner.tuning(key, semis)
    }
    fn param(&mut self, id: u32, value: f64) {
        self.inner.param(id, value)
    }
    fn panic(&mut self) {
        self.inner.panic()
    }
    fn process(&mut self, n: usize, l: &mut [f32], r: &mut [f32]) {
        self.inner.process(n, l, r)
    }
    fn params(&self) -> Vec<ParamInfo> {
        self.inner.params()
    }
    fn value(&self, id: u32) -> Option<f64> {
        self.inner.value(id)
    }
    fn value_text(&self, id: u32, value: f64) -> Option<String> {
        self.inner.value_text(id, value)
    }
    fn save_state(&mut self) -> Option<Vec<u8>> {
        self.inner.save_state()
    }
    fn load_state(&mut self, data: &[u8]) -> bool {
        self.loads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.inner.load_state(data)
    }
    fn set_param_main(&mut self, id: u32, value: f64) {
        self.inner.set_param_main(id, value)
    }
    fn has_editor(&self) -> bool {
        false
    }
    fn open_editor(&mut self, _title: &str) -> Result<(), String> {
        Err("none".into())
    }
    fn close_editor(&mut self) {}
    fn editor_open(&self) -> bool {
        false
    }
    fn poll_main(&mut self) {
        self.inner.poll_main()
    }
    fn take_dirty(&self) -> bool {
        self.inner.take_dirty()
    }
    fn drain_out(&mut self, f: &mut dyn FnMut(OutEv)) {
        self.inner.drain_out(f)
    }
    fn test_handle(&self) -> *const std::ffi::c_void {
        self.inner.test_handle()
    }
    fn set_sample_rate(&mut self, sr: f32) {
        self.inner.set_sample_rate(sr)
    }
}

#[test]
fn capture_reloads_only_instances_whose_state_differs() {
    let loads = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut rack = Rack::new();
    let p = patch();
    rack.insert(0, 0, &p, new_inst());
    let sib = Counting {
        inner: new_inst(),
        loads: loads.clone(),
    };
    rack.insert(0, 1, &p, Box::new(sib));
    let mut song = super::tests::plugin_song();
    let n0 = loads.load(std::sync::atomic::Ordering::Relaxed);
    // unchanged: an autosave must not reload (and cut) the playing sibling
    rack.capture(&mut song, 0);
    rack.capture(&mut song, 0);
    assert_eq!(loads.load(std::sync::atomic::Ordering::Relaxed), n0);
    // a real change is copied over once
    rack.slots[0].inst.set_param_main(1, 0.3);
    rack.capture(&mut song, 0);
    assert_eq!(rack.slots[1].inst.value(1), Some(0.3));
    let n1 = loads.load(std::sync::atomic::Ordering::Relaxed);
    assert_eq!(n1, n0 + 1);
    rack.capture(&mut song, 0);
    assert_eq!(loads.load(std::sync::atomic::Ordering::Relaxed), n1);
}
