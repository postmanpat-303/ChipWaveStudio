fn main() {
    // Pack scripts set CW_BUILD (for example "acidbass abc1234") so the window
    // title and About can say "test build (...)" . Empty or unset is a release.
    println!("cargo:rerun-if-env-changed=CW_BUILD");
    if let Ok(v) = std::env::var("CW_BUILD") {
        let v = v.trim();
        if !v.is_empty() {
            println!("cargo:rustc-env=CW_BUILD={v}");
        }
    }
    println!("cargo:rerun-if-changed=assets/icon/chipwave.rc");
    println!("cargo:rerun-if-changed=assets/icon/chipwave_studio.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // Explorer / taskbar icon (mingw windres or MSVC rc.exe)
        embed_resource::compile("assets/icon/chipwave.rc", embed_resource::NONE)
            .manifest_optional()
            .unwrap();
    }
}
