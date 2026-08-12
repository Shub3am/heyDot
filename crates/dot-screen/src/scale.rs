//! Picks the pixel size a screenshot is captured at.
//! Must not touch ScreenCaptureKit; it is plain arithmetic so it can be tested without a screen.

pub(crate) fn fit_within_long_edge(width: u32, height: u32, max_long_edge: u32) -> (u32, u32) {
    let long_edge = width.max(height);
    if long_edge <= max_long_edge {
        return (width, height);
    }
    let shrink = f64::from(max_long_edge) / f64::from(long_edge);
    let shrink_edge = |edge: u32| ((f64::from(edge) * shrink).round() as u32).max(1);
    (shrink_edge(width), shrink_edge(height))
}

#[cfg(test)]
mod tests {
    use super::fit_within_long_edge;

    #[test]
    fn a_retina_macbook_air_display_shrinks_to_1600_wide() {
        assert_eq!(fit_within_long_edge(3420, 2214, 1600), (1600, 1036));
    }

    #[test]
    fn a_retina_macbook_pro_display_shrinks_to_1600_wide() {
        assert_eq!(fit_within_long_edge(3024, 1964, 1600), (1600, 1039));
    }

    #[test]
    fn a_portrait_display_shrinks_to_1600_tall() {
        assert_eq!(fit_within_long_edge(1080, 1920, 1600), (900, 1600));
    }

    #[test]
    fn a_display_smaller_than_the_limit_is_never_upscaled() {
        assert_eq!(fit_within_long_edge(1512, 982, 1600), (1512, 982));
    }

    #[test]
    fn a_display_exactly_at_the_limit_is_unchanged() {
        assert_eq!(fit_within_long_edge(1600, 900, 1600), (1600, 900));
    }

    #[test]
    fn an_extreme_aspect_never_rounds_an_edge_to_zero() {
        assert_eq!(fit_within_long_edge(20000, 1, 1600), (1600, 1));
    }
}
