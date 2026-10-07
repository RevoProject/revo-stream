pub(crate) fn bridge_info() -> Result<crate::CefBridgeInfo, String> {
    // The optional sibling crate does not implement a linked CEF rendering backend.
    Ok(crate::CefBridgeInfo {
        compiled: false,
        major: 0,
        minor: 0,
        patch: 0,
        commit: 0,
    })
}

pub(crate) fn dock_render_frame(url: String, width: u32, height: u32) -> Result<String, String> {
    use base64::Engine as _;
    let png = crate::sources::browser::capture_png(
        &url,
        width,
        height,
        &std::sync::atomic::AtomicBool::new(false),
    )?;
    Ok(base64::engine::general_purpose::STANDARD.encode(png))
}
