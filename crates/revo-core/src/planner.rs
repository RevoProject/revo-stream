//! Pure scene-canvas geometry shared by the Graphic Planner and source
//! output operations. Functions here must not touch libobs, Tauri, or GPUI.

/// Canvas dimensions used when no live or configured resolution resolves.
pub const DEFAULT_SCENE_RESOLUTION: (u32, u32) = (1920, 1080);

/// Aspect ratio (width / height) of [`DEFAULT_SCENE_RESOLUTION`]; the
/// preview frames use it when no configured resolution resolves.
pub const DEFAULT_SCENE_RESOLUTION_RATIO: f32 =
    DEFAULT_SCENE_RESOLUTION.0 as f32 / DEFAULT_SCENE_RESOLUTION.1 as f32;

/// Largest uniform scale that fits `base` inside `canvas`, never upscaling.
pub fn fit_scale(
    base_width: u32,
    base_height: u32,
    canvas_width: u32,
    canvas_height: u32,
) -> Option<f32> {
    if base_width == 0 || base_height == 0 || canvas_width == 0 || canvas_height == 0 {
        return None;
    }
    Some(
        ((canvas_width as f32 / base_width as f32).min(canvas_height as f32 / base_height as f32))
            .min(1.0),
    )
}

/// Independent horizontal and vertical scales that stretch `base` to fill
/// `canvas`, ignoring aspect ratio.
pub fn stretch_scale(
    base_width: u32,
    base_height: u32,
    canvas_width: u32,
    canvas_height: u32,
) -> Option<(f32, f32)> {
    if base_width == 0 || base_height == 0 || canvas_width == 0 || canvas_height == 0 {
        return None;
    }
    Some((
        canvas_width as f32 / base_width as f32,
        canvas_height as f32 / base_height as f32,
    ))
}

/// Top-left position centering an item of `item_width` x `item_height` inside
/// `canvas`. May be negative when the item exceeds the canvas.
pub fn centered_position(
    canvas_width: u32,
    canvas_height: u32,
    item_width: f32,
    item_height: f32,
) -> (f32, f32) {
    (
        (canvas_width as f32 - item_width) * 0.5,
        (canvas_height as f32 - item_height) * 0.5,
    )
}

/// Parses a `WIDTHxHEIGHT` string such as `1920x1080`, tolerating surrounding
/// and inner whitespace, rejecting zero or missing dimensions.
pub fn parse_resolution(raw: &str) -> Option<(u32, u32)> {
    let (width, height) = raw.trim().split_once('x')?;
    let width = width.trim().parse::<u32>().ok()?;
    let height = height.trim().parse::<u32>().ok()?;
    (width > 0 && height > 0).then_some((width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_scale_matches_current_behavior() {
        assert_eq!(fit_scale(3840, 2160, 1920, 1080), Some(0.5));
        assert_eq!(fit_scale(2560, 1080, 1920, 1080), Some(0.75));
        assert_eq!(fit_scale(640, 480, 1920, 1080), Some(1.0));
        assert_eq!(fit_scale(0, 480, 1920, 1080), None);
        assert_eq!(fit_scale(640, 0, 1920, 1080), None);
        assert_eq!(fit_scale(640, 480, 0, 1080), None);
    }

    #[test]
    fn centered_position_locks_existing_geometry() {
        let scale = fit_scale(1920, 1440, 1920, 1080).unwrap();
        assert_eq!(
            centered_position(1920, 1080, 1920.0 * scale, 1440.0 * scale),
            (240.0, 0.0)
        );
        assert_eq!(
            centered_position(1920, 1080, 3840.0, 2160.0),
            (-960.0, -540.0)
        );
    }

    #[test]
    fn stretch_scale_matches_current_behavior() {
        assert_eq!(stretch_scale(640, 480, 1920, 1080), Some((3.0, 2.25)));
        assert_eq!(stretch_scale(0, 480, 1920, 1080), None);
        assert_eq!(stretch_scale(640, 0, 1920, 1080), None);
        assert_eq!(stretch_scale(640, 480, 0, 1080), None);
    }

    #[test]
    fn parse_resolution_matches_current_behavior() {
        assert_eq!(parse_resolution("1920x1080"), Some((1920, 1080)));
        assert_eq!(parse_resolution(" 1280 x 720 "), Some((1280, 720)));
        assert_eq!(parse_resolution("0x1080"), None);
        assert_eq!(parse_resolution("1920"), None);
        assert_eq!(parse_resolution("axb"), None);
        assert_eq!(parse_resolution("1x2x3"), None);
        assert_eq!(parse_resolution(""), None);
        assert_eq!(parse_resolution("x"), None);
    }
}
