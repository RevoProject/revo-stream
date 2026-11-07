//! Embedded asset source: icons (lucide-style SVGs) and the brand logo are
//! compiled into the binary so the native shell needs no filesystem assets.

use anyhow::Result;
use gpui::{AssetSource, SharedString};
use std::borrow::Cow;
use std::path::Path;

/// (path, bytes) for every embedded asset; `include_bytes!` keeps them in
/// the binary so the shell needs no filesystem asset directory.
const ASSETS: &[(&str, &[u8])] = &[
    (
        "brand/favicon.png",
        include_bytes!("../assets/brand/favicon.png"),
    ),
    ("brand/mark.svg", include_bytes!("../assets/brand/mark.svg")),
    (
        "icons/spinner.svg",
        include_bytes!("../assets/icons/spinner.svg"),
    ),
    (
        "icons/close.svg",
        include_bytes!("../assets/icons/close.svg"),
    ),
    (
        "icons/pencil.svg",
        include_bytes!("../assets/icons/pencil.svg"),
    ),
    ("icons/copy.svg", include_bytes!("../assets/icons/copy.svg")),
    (
        "icons/trash.svg",
        include_bytes!("../assets/icons/trash.svg"),
    ),
    (
        "icons/layers.svg",
        include_bytes!("../assets/icons/layers.svg"),
    ),
    ("icons/info.svg", include_bytes!("../assets/icons/info.svg")),
    ("icons/lock.svg", include_bytes!("../assets/icons/lock.svg")),
    (
        "icons/unlock.svg",
        include_bytes!("../assets/icons/unlock.svg"),
    ),
    (
        "icons/volume.svg",
        include_bytes!("../assets/icons/volume.svg"),
    ),
    (
        "icons/volume-x.svg",
        include_bytes!("../assets/icons/volume-x.svg"),
    ),
    (
        "icons/refresh.svg",
        include_bytes!("../assets/icons/refresh.svg"),
    ),
    (
        "icons/folder.svg",
        include_bytes!("../assets/icons/folder.svg"),
    ),
    (
        "icons/file-text.svg",
        include_bytes!("../assets/icons/file-text.svg"),
    ),
    ("icons/user.svg", include_bytes!("../assets/icons/user.svg")),
    ("icons/eye.svg", include_bytes!("../assets/icons/eye.svg")),
    (
        "icons/warning.svg",
        include_bytes!("../assets/icons/warning.svg"),
    ),
    (
        "icons/error.svg",
        include_bytes!("../assets/icons/error.svg"),
    ),
    (
        "icons/interact.svg",
        include_bytes!("../assets/icons/interact.svg"),
    ),
    (
        "icons/expand.svg",
        include_bytes!("../assets/icons/expand.svg"),
    ),
    (
        "icons/source.svg",
        include_bytes!("../assets/icons/source.svg"),
    ),
    (
        "icons/sliders.svg",
        include_bytes!("../assets/icons/sliders.svg"),
    ),
    ("icons/dots.svg", include_bytes!("../assets/icons/dots.svg")),
    (
        "icons/settings.svg",
        include_bytes!("../assets/icons/settings.svg"),
    ),
    (
        "icons/power.svg",
        include_bytes!("../assets/icons/power.svg"),
    ),
    (
        "icons/record.svg",
        include_bytes!("../assets/icons/record.svg"),
    ),
    ("icons/stop.svg", include_bytes!("../assets/icons/stop.svg")),
    (
        "icons/monitor.svg",
        include_bytes!("../assets/icons/monitor.svg"),
    ),
    (
        "fonts/space-grotesk-400.ttf",
        include_bytes!("../assets/fonts/space-grotesk-400.ttf"),
    ),
    (
        "fonts/space-grotesk-600.ttf",
        include_bytes!("../assets/fonts/space-grotesk-600.ttf"),
    ),
    (
        "fonts/space-grotesk-700.ttf",
        include_bytes!("../assets/fonts/space-grotesk-700.ttf"),
    ),
];

/// [`AssetSource`] over `include_bytes!`-embedded icon and logo files.
pub struct EmbeddedAssets;

impl AssetSource for EmbeddedAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>, anyhow::Error> {
        for (asset_path, bytes) in ASSETS {
            if *asset_path == path {
                return Ok(Some(Cow::Borrowed(*bytes)));
            }
        }
        Ok(None)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>, anyhow::Error> {
        let prefix = Path::new(path);
        Ok(ASSETS
            .iter()
            .filter(|(asset_path, _)| Path::new(asset_path).starts_with(prefix))
            .map(|(asset_path, _)| SharedString::from(asset_path.to_string()))
            .collect())
    }
}
