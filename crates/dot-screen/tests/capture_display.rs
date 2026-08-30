// Needs a real screen and Screen Recording permission for the terminal, so it is ignored in CI.
// Every display this runs on is at least 1600 px on its long edge (any Retina Mac, any 1080p screen).

use std::time::Instant;

use dot_screen::capture_display_under_cursor;

#[test]
#[ignore = "needs a screen and Screen Recording permission"]
fn captures_the_display_under_the_cursor_at_1600_px_on_its_long_edge() {
    for attempt in 1..=5 {
        let started = Instant::now();
        let jpeg = capture_display_under_cursor(1600).unwrap();
        let elapsed = started.elapsed();

        let decoded = image::load_from_memory_with_format(&jpeg, image::ImageFormat::Jpeg).unwrap();
        println!(
            "capture {attempt}: {}x{}, {} KB, {elapsed:?}",
            decoded.width(),
            decoded.height(),
            jpeg.len() / 1024
        );
        assert_eq!(decoded.width().max(decoded.height()), 1600);
    }
}
