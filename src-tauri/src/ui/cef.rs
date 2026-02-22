pub(crate) fn bridge_info() -> Result<crate::CefBridgeInfo, String> {
	#[cfg(feature = "cef")]
	unsafe {
		return Ok(crate::CefBridgeInfo {
			compiled: true,
			major: revo_cef::ffi::cef_version_info(0),
			minor: revo_cef::ffi::cef_version_info(1),
			patch: revo_cef::ffi::cef_version_info(2),
			commit: revo_cef::ffi::cef_version_info(3),
		});
	}

	#[cfg(not(feature = "cef"))]
	{
		Ok(crate::CefBridgeInfo {
			compiled: false,
			major: 0,
			minor: 0,
			patch: 0,
			commit: 0,
		})
	}
}

pub(crate) fn dock_render_frame(url: String, width: u32, height: u32) -> Result<String, String> {
	use base64::Engine as _;

	let w = width.clamp(64, 4096);
	let h = height.clamp(64, 4096);

	#[cfg(feature = "cef")]
	{
		let hash = url
			.bytes()
			.fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
		let c1 = ((hash >> 16) & 0xFF) as u8;
		let c2 = ((hash >> 8) & 0xFF) as u8;
		let c3 = (hash & 0xFF) as u8;

		let img: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> =
			image::ImageBuffer::from_fn(w, h, |x, y| {
				let band = (((x / 24) + (y / 24)) % 2) as u8;
				let (r, g, b) = if band == 0 {
					(c1.saturating_add(25), c2.saturating_add(25), c3.saturating_add(25))
				} else {
					(c1 / 2, c2 / 2, c3 / 2)
				};
				image::Rgba([r, g, b, 255])
			});

		let mut png = Vec::new();
		image::codecs::png::PngEncoder::new(&mut png)
			.write_image(img.as_raw(), w, h, image::ColorType::Rgba8.into())
			.map_err(|e| format!("failed to encode frame: {e}"))?;

		return Ok(base64::engine::general_purpose::STANDARD.encode(png));
	}

	#[cfg(not(feature = "cef"))]
	{
		let target = url.trim();
		let parsed = tauri::Url::parse(target).map_err(|e| format!("invalid url: {e}"))?;
		let scheme = parsed.scheme();
		if scheme != "http" && scheme != "https" {
			return Err("Only http/https URLs are supported".to_string());
		}

		let screenshot_path = std::env::temp_dir().join(format!("revo-dock-{}.png", uuid::Uuid::new_v4()));
		let screenshot_arg = format!("--screenshot={}", screenshot_path.display());
		let size_arg = format!("--window-size={},{}", w, h);
		let url_arg = parsed.as_str().to_string();

		let chrome_bins = [
			"chromium",
			"/usr/bin/chromium",
			"/snap/bin/chromium",
			"chromium-browser",
			"/usr/bin/chromium-browser",
			"brave-browser",
			"/usr/bin/brave-browser",
			"brave",
			"/usr/bin/brave",
			"/snap/bin/brave",
			"google-chrome-stable",
			"/usr/bin/google-chrome-stable",
			"google-chrome",
			"/usr/bin/google-chrome",
		];

		let headless_flags = ["--headless=new", "--headless"];

		let mut last_err: Option<String> = None;
		let mut started = false;

		for bin in chrome_bins {
			for headless_flag in headless_flags {
				let status = std::process::Command::new(bin)
					.arg(headless_flag)
					.arg("--disable-gpu")
					.arg("--hide-scrollbars")
					.arg("--mute-audio")
					.arg("--no-sandbox")
					.arg("--run-all-compositor-stages-before-draw")
					.arg("--virtual-time-budget=2500")
					.arg(&size_arg)
					.arg(&screenshot_arg)
					.arg(&url_arg)
					.status();

				match status {
					Ok(exit) => {
						started = true;
						if exit.success() {
							let png = std::fs::read(&screenshot_path)
								.map_err(|e| format!("failed to read chromium screenshot: {e}"))?;
							let _ = std::fs::remove_file(&screenshot_path);
							return Ok(base64::engine::general_purpose::STANDARD.encode(png));
						}
						last_err = Some(format!("{bin} ({headless_flag}) exited with status {exit}"));
					}
					Err(e) => {
						if e.kind() != std::io::ErrorKind::NotFound {
							last_err = Some(format!("{bin} ({headless_flag}) failed: {e}"));
						}
					}
				}
			}
		}

		let _ = std::fs::remove_file(&screenshot_path);

		if !started {
			return Err(
				"No Chromium binary found (tried chromium/brave/chrome in PATH, /usr/bin, and /snap/bin)"
					.to_string(),
			);
		}

		Err(last_err.unwrap_or_else(|| "Chromium screenshot failed".to_string()))
	}
}
