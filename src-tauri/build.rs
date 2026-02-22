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

    tauri_build::build()
}
