fn main() {
    println!("cargo:rerun-if-changed=assets/icon/chipwave.rc");
    println!("cargo:rerun-if-changed=assets/icon/chipwave_tracker.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // Explorer / taskbar icon (mingw windres or MSVC rc.exe)
        embed_resource::compile("assets/icon/chipwave.rc", embed_resource::NONE)
            .manifest_optional()
            .unwrap();
    }
}
