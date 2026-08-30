//! Takes one screenshot of the display under the mouse cursor, without Hey Dot's own windows.
//! Must not pick the size limit or decide what happens without a screenshot; the caller does both.

use objc2_core_graphics::{CGEvent, CGPreflightScreenCaptureAccess, CGRequestScreenCaptureAccess};
use screencapturekit::cg::CGPoint;
use screencapturekit::error::SCError;
use screencapturekit::screenshot_manager::{CGImageExt, SCScreenshotManager};
use screencapturekit::shareable_content::{SCShareableContent, SCShareableContentInfo, SCWindow};
use screencapturekit::stream::configuration::SCStreamConfiguration;
use screencapturekit::stream::content_filter::SCContentFilter;

use crate::ScreenError;
use crate::jpeg::encode_jpeg;
use crate::scale::fit_within_long_edge;

/// Blocks for about 95 ms warm, 165 ms on the first capture. Run it off the async runtime.
pub fn capture_display_under_cursor(max_long_edge_px: u32) -> Result<Vec<u8>, ScreenError> {
    // ScreenCaptureKit reports a denied permission as an untyped "no shareable content" error.
    if !CGPreflightScreenCaptureAccess() {
        return Err(ScreenError::PermissionNotGranted);
    }
    let cursor_event = CGEvent::new(None);
    let cursor = CGEvent::location(cursor_event.as_deref());
    let content = SCShareableContent::get()?;
    // Display frames and the cursor are both in global points with a top-left origin.
    let display = content
        .displays()
        .into_iter()
        .find(|display| {
            display
                .frame()
                .contains_point(CGPoint::new(cursor.x, cursor.y))
        })
        .ok_or(ScreenError::NoDisplayUnderCursor)?;

    let own_pid = std::process::id() as i32;
    let windows = content.windows();
    let own_windows: Vec<&SCWindow> = windows
        .iter()
        .filter(|window| {
            window
                .owning_application()
                .is_some_and(|app| app.process_id() == own_pid)
        })
        .collect();
    let filter = SCContentFilter::create()
        .with_display(&display)
        .with_excluding_windows(&own_windows)
        .build();

    // SCDisplay's width and height are points; the pixel size needs the display's scale.
    let filter_info = SCShareableContentInfo::for_filter(&filter).ok_or_else(|| {
        SCError::NoShareableContent("no size information for the display".to_owned())
    })?;
    let scale = f64::from(filter_info.point_pixel_scale());
    let points = filter_info.content_rect().size;
    let (width, height) = fit_within_long_edge(
        (points.width * scale).round() as u32,
        (points.height * scale).round() as u32,
        max_long_edge_px,
    );
    let config = SCStreamConfiguration::new()
        .with_width(width)
        .with_height(height);
    let image = SCScreenshotManager::capture_image(&filter, &config)?;
    encode_jpeg(
        &image.rgba_data()?,
        image.width() as u32,
        image.height() as u32,
    )
}

/// Shows macOS' Screen Recording prompt the first time and lists Hey Dot in System Settings.
/// After the user has answered once, it does nothing.
pub fn request_screen_access() {
    CGRequestScreenCaptureAccess();
}
