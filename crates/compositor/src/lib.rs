//! Compositor layout engine: the geometry the ZenvX session uses to split the
//! screen into an ~80% app zone and a reserved ~20% terminal zone, and to tile
//! foreign app surfaces inside the app zone.
//!
//! This is the pure, testable core. A Smithay-based Wayland event loop (built in
//! a graphical session) feeds client surfaces through `tile` and configures them
//! with the resulting rectangles; the math lives here so it can be verified
//! headlessly.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Layout {
    pub app_zone: Rect,
    pub term_zone: Rect,
}

/// Fraction of screen height reserved for the terminal pane.
pub const TERM_PERCENT: i32 = 20;

/// Split a screen into the app zone (top ~80%) and terminal zone (bottom ~20%).
pub fn layout(screen_w: i32, screen_h: i32) -> Layout {
    let term_h = screen_h * TERM_PERCENT / 100;
    let app_h = screen_h - term_h;
    Layout {
        app_zone: Rect { x: 0, y: 0, w: screen_w, h: app_h },
        term_zone: Rect { x: 0, y: app_h, w: screen_w, h: term_h },
    }
}

/// Tile `n` surfaces side-by-side within `zone` (last one absorbs the remainder).
pub fn tile(zone: &Rect, n: usize) -> Vec<Rect> {
    if n == 0 {
        return vec![];
    }
    let col_w = zone.w / n as i32;
    (0..n)
        .map(|i| {
            let x = zone.x + i as i32 * col_w;
            let w = if i == n - 1 { zone.x + zone.w - x } else { col_w };
            Rect { x, y: zone.y, w, h: zone.h }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zones_split_80_20_and_cover_screen() {
        let l = layout(1920, 1080);
        assert_eq!(l.app_zone, Rect { x: 0, y: 0, w: 1920, h: 864 });
        assert_eq!(l.term_zone, Rect { x: 0, y: 864, w: 1920, h: 216 });
        // zones are disjoint and cover the full height
        assert_eq!(l.app_zone.h + l.term_zone.h, 1080);
        assert_eq!(l.term_zone.y, l.app_zone.h);
    }

    #[test]
    fn tiling_fills_zone_without_gaps() {
        let l = layout(1000, 1000); // app zone 1000x800
        let rects = tile(&l.app_zone, 3);
        assert_eq!(rects.len(), 3);
        assert_eq!(rects[0].x, 0);
        // surfaces are contiguous and the last reaches the right edge
        assert_eq!(rects[1].x, rects[0].x + rects[0].w);
        assert_eq!(rects[2].x + rects[2].w, l.app_zone.w);
        // all stay inside the app zone height (terminal stays reserved)
        assert!(rects.iter().all(|r| r.h == l.app_zone.h && r.y == 0));
    }

    #[test]
    fn no_surfaces_yields_empty() {
        assert!(tile(&layout(800, 600).app_zone, 0).is_empty());
    }
}
