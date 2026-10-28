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


/// One-frame metrics collected from the OS-backed RaylibHandle.
/// Layout, mouse and raylib scissor consume LOGICAL pixels. Rendering/pixel
/// inspection uses framebuffer dimensions. Do not pre-scale scissor coordinates:
/// raylib's BeginScissorMode already applies its own configured DPI transform.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FrameMetrics {
    pub(crate) logical: RootViewport,
    pub(crate) render_width: i32,
    pub(crate) render_height: i32,
    pub(crate) scale_x: f32,
    pub(crate) scale_y: f32,
}
impl FrameMetrics {
    pub(crate) fn from_window(logical_w: i32, logical_h: i32, render_w: i32, render_h: i32) -> Self {
        let logical = RootViewport::from_window_size(logical_w, logical_h);
        let render_width = render_w.max(0);
        let render_height = render_h.max(0);
        let ratio = |physical: i32, l: f32| {
            if l > 0.0 && physical > 0 { physical as f32 / l } else { 0.0 }
        };
        Self {
            logical,
            render_width,
            render_height,
            scale_x: ratio(render_width, logical.width),
            scale_y: ratio(render_height, logical.height),
        }
    }

    /// Mapping exclusively for buffer readback and visual verification.
    /// Logical renderer positions MUST NOT be multiplied by this transform.
    pub(crate) fn logical_to_framebuffer(self, x: f32, y: f32) -> Option<(i32, i32)> {
        if !x.is_finite() || !y.is_finite() || x < 0.0 || y < 0.0
            || x >= self.logical.width || y >= self.logical.height
            || self.scale_x <= 0.0 || self.scale_y <= 0.0 { return None; }
        let px = (x * self.scale_x).floor() as i32;
        let py = (y * self.scale_y).floor() as i32;
        (px < self.render_width && py < self.render_height).then_some((px, py))
    }
}

#[cfg(test)]
mod tests {
    use super::{RootViewport, FrameMetrics};

    #[test]
    fn framebuffer_scale_handles_1x_15x_2x_and_asymmetric_ratios() {
        for (rw,rh,sx,sy) in [(800,600,1.0,1.0),(1200,900,1.5,1.5),
            (1600,1200,2.0,2.0),(1600,900,2.0,1.5)] {
            let m=FrameMetrics::from_window(800,600,rw,rh);
            assert_eq!((m.logical.width,m.logical.height),(800.0,600.0));
            assert_eq!((m.scale_x,m.scale_y),(sx,sy));
            assert_eq!(m.logical_to_framebuffer(100.0,100.0),
                Some((100.0_f32.mul_add(sx,0.0) as i32,100.0_f32.mul_add(sy,0.0) as i32)));
        }
    }

    #[test]
    fn logical_edges_and_invalid_metrics_fail_closed() {
        let m=FrameMetrics::from_window(400,200,800,400);
        assert_eq!(m.logical_to_framebuffer(399.9,199.9),Some((799,399)));
        for (x,y) in [(400.0,0.0),(0.0,200.0),(-1.0,0.0),(f32::NAN,5.0),(1.0,f32::INFINITY)] {
            assert_eq!(m.logical_to_framebuffer(x,y),None);
        }
        let zero=FrameMetrics::from_window(0,-4,400,800);
        assert_eq!(zero.logical_to_framebuffer(0.0,0.0),None);
        assert_eq!(zero.scale_x,0.0);
        let physical_zero=FrameMetrics::from_window(300,200,0,0);
        assert_eq!(physical_zero.logical_to_framebuffer(2.0,2.0),None);
    }

    #[test]
    fn resize_reads_frame_metrics_afresh_not_from_initial_resolution() {
        let seq=[(1920,1080,1920,1080),(960,540,1920,1080),
            (500,350,1000,700),(1280,720,1280,720)];
        let metrics: Vec<_>=seq.into_iter().map(|(w,h,rw,rh)|
            FrameMetrics::from_window(w,h,rw,rh)).collect();
        assert_eq!(metrics[0].scale_x,1.0);
        assert_eq!(metrics[1].scale_x,2.0);
        assert_eq!(metrics[2].logical.width,500.0);
        assert_eq!(metrics[3].logical_to_framebuffer(1200.0,700.0),Some((1200,700)));
    }


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
