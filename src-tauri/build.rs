use std::{env, fs, path::PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let bundled_obs_browser_conf = manifest_dir
        .join("resources")
        .join("revo-root")
        .join("data")
        .join("conf")
        .join("obs-browser");
    if bundled_obs_browser_conf.exists() {
        if let Err(err) = fs::remove_dir_all(&bundled_obs_browser_conf) {
            eprintln!(
                "warning: failed to remove transient bundled obs-browser conf dir {}: {err}",
                bundled_obs_browser_conf.display()
            );
        }
    }

    println!("cargo:rerun-if-env-changed=REVO_OBS_LIB_DIR");
    if let Some(dir) = env::var_os("REVO_OBS_LIB_DIR") {
        println!("cargo:rustc-link-search=native={}", PathBuf::from(dir).display());
    } else if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        println!("cargo:rustc-link-search=native=/usr/lib");
    }
    println!("cargo:rustc-link-lib=obs");

    tauri_build::build()
}
