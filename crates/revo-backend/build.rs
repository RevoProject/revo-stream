fn main() {
    println!("cargo:rerun-if-env-changed=REVO_OBS_LIB_DIR");
    println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_OS");

    // On Windows the OBS bootstrap ships `obs.dll` without an MSVC import
    // library, and the `libobs` crate of `revo-lib` already carries the link
    // directives for that platform. Asking the linker for `obs.lib` here would
    // fail with LNK1181, so the explicit link stays on the platforms where the
    // runtime is a plain shared object we locate ourselves.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        return;
    }

    if let Some(dir) = std::env::var_os("REVO_OBS_LIB_DIR") {
        println!(
            "cargo:rustc-link-search=native={}",
            std::path::PathBuf::from(dir).display()
        );
    } else if target_os == "linux" {
        println!("cargo:rustc-link-search=native=/usr/lib");
    }
    println!("cargo:rustc-link-lib=obs");
}
