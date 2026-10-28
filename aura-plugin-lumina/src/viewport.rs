//! Window-logical viewport geometry, intentionally independent of a renderer.
//! This is NOT framebuffer/DPI conversion, safe-area management or layout sizing.

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RootViewport {
    pub(crate) width: f32,
    pub(crate) height: f32,
}

impl RootViewport {
    /// Sample the current window-logical dimensions for *this* frame.
    /// A minimized/invalid nonpositive dimension contributes zero available space,
    /// never an imaginary fixed-size surface.
    pub(crate) fn from_window_size(width: i32, height: i32) -> Self {
        Self {
            width: width.max(0) as f32,
            height: height.max(0) as f32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RootViewport;

    #[test]
    fn exact_logical_dimensions_for_multiple_viewports() {
        for (w, h) in [(1920, 1080), (1280, 720), (800, 600), (360, 640), (640, 360)] {
            let viewport = RootViewport::from_window_size(w, h);
            assert_eq!((viewport.width, viewport.height), (w as f32, h as f32));
        }
    }

    #[test]
    fn every_frame_is_resampled_without_stale_dimensions() {
        let sizes = [(1920, 1080), (900, 500), (320, 240), (1600, 900)];
        let observed: Vec<_> = sizes
            .into_iter()
            .map(|(w, h)| {
                let v = RootViewport::from_window_size(w, h);
                (v.width, v.height)
            })
            .collect();
        assert_eq!(
            observed,
            vec![(1920.0, 1080.0), (900.0, 500.0), (320.0, 240.0), (1600.0, 900.0)]
        );
    }

    #[test]
    fn nonpositive_dimensions_do_not_create_fake_render_space() {
        assert_eq!(RootViewport::from_window_size(0, 0), RootViewport { width: 0.0, height: 0.0 });
        assert_eq!(RootViewport::from_window_size(-10, 720), RootViewport { width: 0.0, height: 720.0 });
        assert_eq!(RootViewport::from_window_size(640, -1), RootViewport { width: 640.0, height: 0.0 });
    }
}
