fn main() {
    println!("cargo:rerun-if-env-changed=REVO_OBS_LIB_DIR");
    if let Some(dir) = std::env::var_os("REVO_OBS_LIB_DIR") {
        println!(
            "cargo:rustc-link-search=native={}",
            std::path::PathBuf::from(dir).display()
        );
    } else if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        println!("cargo:rustc-link-search=native=/usr/lib");
    }
    println!("cargo:rustc-link-lib=obs");
}
