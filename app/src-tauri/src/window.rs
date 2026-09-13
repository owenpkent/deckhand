// Window geometry: plain data in, data out, so placement and clamping
// are unit-tested without a real window or a real monitor
// (docs/ARCHITECTURE.md#the-surface). The impure edges (reading Tauri's
// monitor list, calling set_size/set_position) live in main.rs and stay
// thin wrappers around the functions here.
//
// All rects are physical pixels: that is what Tauri's Monitor and
// outer_position/outer_size report, and mixing logical and physical
// values in one clamp is exactly the kind of bug this module exists to
// keep out of a scaled multi-monitor setup.

/// The window's width. Height is derived from the row count
/// (`window_height`) and changes as sessions bind and unbind.
pub const WINDOW_W_LOGICAL: f64 = 360.0;

/// Header row: drag grip, Move, Quit.
pub const HEADER_H_LOGICAL: f64 = 56.0;

/// One session row. 48 is comfortably above the 44px accessibility
/// floor (docs/ACCESSIBILITY.md); the floor itself is a minimum, not a
/// target.
pub const ROW_H_LOGICAL: f64 = 48.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Rect { x, y, w, h }
    }

    pub fn right(&self) -> i32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }

    pub fn center_y(&self) -> i32 {
        self.y + self.h / 2
    }
}

/// True when `rect` shares at least one pixel with `area`. Used to test
/// a saved position against the monitors actually connected right now:
/// a removed monitor or a resolution change can leave a saved point
/// sitting in space that no longer exists.
pub fn intersects(rect: Rect, area: Rect) -> bool {
    rect.x < area.right() && rect.right() > area.x && rect.y < area.bottom() && rect.bottom() > area.y
}

/// The window's height for a given row count: the header plus one row
/// per session, at least one row (the "No sessions" placeholder when the
/// list is empty), capped at `max_h` so the window never grows past the
/// monitor's work area. The list itself scrolls beyond that cap.
pub fn window_height(row_count: usize, max_h: i32) -> i32 {
    let rows = row_count.max(1) as i32;
    let h = (HEADER_H_LOGICAL + rows as f64 * ROW_H_LOGICAL).round() as i32;
    h.min(max_h.max(HEADER_H_LOGICAL as i32 + ROW_H_LOGICAL as i32))
}

/// Recompute a rect for a new height, keeping the bottom edge fixed when
/// `bottom_anchored` (a window parked near the bottom of the screen
/// should grow upward as rows are added, not slide off the taskbar) and
/// the top edge fixed otherwise.
pub fn reflow_height(old: Rect, new_h: i32, bottom_anchored: bool) -> Rect {
    let y = if bottom_anchored { old.bottom() - new_h } else { old.y };
    Rect::new(old.x, y, old.w, new_h)
}

/// Whether `rect`'s centre sits in the bottom half of `area`: the
/// heuristic `reflow_height` and `cycle_position` use to decide which
/// edge a resize should anchor to. There is no explicit anchor stored
/// anywhere; the window's own position on screen is the anchor.
pub fn is_bottom_anchored(rect: Rect, area: Rect) -> bool {
    rect.center_y() > area.y + area.h / 2
}

/// Fit `rect` fully inside `area`: shrink first if `rect` is larger than
/// `area` in either dimension (the window is bigger than the whole
/// monitor, e.g. a huge session list on a small screen), then translate
/// so no edge crosses the area's bounds.
pub fn clamp_into(rect: Rect, area: Rect) -> Rect {
    let w = rect.w.min(area.w);
    let h = rect.h.min(area.h);
    let mut x = rect.x;
    let mut y = rect.y;
    if x < area.x {
        x = area.x;
    }
    if x + w > area.right() {
        x = area.right() - w;
    }
    if y < area.y {
        y = area.y;
    }
    if y + h > area.bottom() {
        y = area.bottom() - h;
    }
    Rect::new(x, y, w, h)
}

/// Validate a restored position against the monitors actually connected
/// at startup (PR review P1). When `saved` intersects one of `areas`, it
/// is clamped into that area (a resolution change may have shrunk it out
/// from under part of the window). When it intersects none of them (a
/// removed monitor, or a saved point from a machine that no longer has
/// that monitor), `fallback` is placed on the first available area
/// instead of trusting the stale point at all.
pub fn restore_or_fallback(saved: Rect, areas: &[Rect], fallback: Rect) -> Rect {
    if let Some(area) = areas.iter().find(|a| intersects(saved, **a)) {
        return clamp_into(saved, *area);
    }
    match areas.first() {
        Some(area) => clamp_into(fallback, *area),
        None => fallback,
    }
}

/// The work area a rect should be clamped into right now: whichever
/// monitor it already overlaps, else the first one available. Returns
/// `None` only when there are no monitors at all.
pub fn area_for(rect: Rect, areas: &[Rect]) -> Option<&Rect> {
    areas.iter().find(|a| intersects(rect, **a)).or_else(|| areas.first())
}

/// A visible default: near the top-left of the given area, with a small
/// margin so the window is never flush against the screen edge.
pub fn default_rect(area: Rect, w: i32, h: i32) -> Rect {
    let margin = 12;
    Rect::new(area.x + margin, area.y + margin, w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect::new(x, y, w, h)
    }

    #[test]
    fn window_height_grows_with_row_count() {
        let h1 = window_height(1, 2000);
        let h3 = window_height(3, 2000);
        assert!(h3 > h1);
        assert_eq!(h3 - h1, (2.0 * ROW_H_LOGICAL).round() as i32);
    }

    #[test]
    fn window_height_never_drops_below_one_row_even_with_zero_sessions() {
        assert_eq!(window_height(0, 2000), window_height(1, 2000), "the empty list still shows the No sessions row");
    }

    #[test]
    fn window_height_is_capped_at_the_monitor_work_area() {
        let uncapped = window_height(50, 100_000);
        let capped = window_height(50, 300);
        assert!(capped < uncapped);
        assert_eq!(capped, 300, "fifty rows must be capped to the work area height");
    }

    #[test]
    fn reflow_height_keeps_the_top_edge_when_not_bottom_anchored() {
        let old = area(100, 100, 360, 200);
        let next = reflow_height(old, 400, false);
        assert_eq!(next.y, 100, "the top edge must not move");
        assert_eq!(next.h, 400);
    }

    #[test]
    fn reflow_height_keeps_the_bottom_edge_when_bottom_anchored() {
        // Old bottom is 100 + 200 = 300; growing to 400 must keep that
        // bottom edge fixed, i.e. the window grows upward.
        let old = area(100, 100, 360, 200);
        let next = reflow_height(old, 400, true);
        assert_eq!(next.bottom(), 300, "the bottom edge must stay put");
        assert_eq!(next.y, -100);
    }

    #[test]
    fn is_bottom_anchored_true_in_the_bottom_half_of_the_monitor() {
        let mon = area(0, 0, 1920, 1080);
        let bottom_window = area(100, 900, 360, 100);
        let top_window = area(100, 50, 360, 100);
        assert!(is_bottom_anchored(bottom_window, mon));
        assert!(!is_bottom_anchored(top_window, mon));
    }

    #[test]
    fn clamp_into_leaves_a_rect_already_inside_untouched() {
        let mon = area(0, 0, 1920, 1080);
        let r = area(100, 100, 360, 200);
        assert_eq!(clamp_into(r, mon), r);
    }

    #[test]
    fn clamp_into_translates_a_rect_hanging_off_the_right_and_bottom_edges() {
        let mon = area(0, 0, 1920, 1080);
        let r = area(1800, 1000, 360, 200);
        let clamped = clamp_into(r, mon);
        assert_eq!(clamped.right(), 1920);
        assert_eq!(clamped.bottom(), 1080);
        assert_eq!(clamped.w, 360);
        assert_eq!(clamped.h, 200);
    }

    #[test]
    fn clamp_into_translates_a_rect_off_the_left_and_top_edges() {
        let mon = area(0, 0, 1920, 1080);
        let r = area(-50, -50, 360, 200);
        let clamped = clamp_into(r, mon);
        assert_eq!(clamped.x, 0);
        assert_eq!(clamped.y, 0);
    }

    #[test]
    fn clamp_into_shrinks_a_rect_bigger_than_the_whole_monitor() {
        let mon = area(0, 0, 800, 600);
        let r = area(0, 0, 1200, 2000);
        let clamped = clamp_into(r, mon);
        assert_eq!(clamped.w, 800);
        assert_eq!(clamped.h, 600);
    }

    // ---- restore_or_fallback: the three PR-review scenarios ------------

    #[test]
    fn restore_or_fallback_removed_monitor_uses_the_fallback() {
        // The saved position sat on a second monitor to the right that is
        // no longer connected.
        let saved = area(2000, 100, 360, 200);
        let areas = [area(0, 0, 1920, 1080)];
        let fallback = default_rect(areas[0], 360, 200);
        let got = restore_or_fallback(saved, &areas, fallback);
        assert!(intersects(got, areas[0]), "the fallback must land on a real monitor");
        assert_eq!(got, fallback);
    }

    #[test]
    fn restore_or_fallback_resolution_change_clamps_rather_than_falls_back() {
        // The window was placed bottom-right on a 1920x1080 monitor; the
        // monitor is now 1600x900 (e.g. a resolution change). The saved
        // point still overlaps the new, smaller area, so it is clamped in
        // place rather than replaced outright.
        let saved = area(1560, 880, 360, 200);
        let areas = [area(0, 0, 1600, 900)];
        let fallback = default_rect(areas[0], 360, 200);
        assert!(intersects(saved, areas[0]), "sanity: the saved rect still overlaps the shrunk monitor");
        let got = restore_or_fallback(saved, &areas, fallback);
        assert_eq!(got.right(), 1600);
        assert_eq!(got.bottom(), 900);
    }

    #[test]
    fn restore_or_fallback_leaves_a_fully_visible_saved_rect_alone() {
        let saved = area(200, 200, 360, 200);
        let areas = [area(0, 0, 1920, 1080)];
        let fallback = default_rect(areas[0], 360, 200);
        assert_eq!(restore_or_fallback(saved, &areas, fallback), saved);
    }

    #[test]
    fn restore_or_fallback_bottom_preset_growth_keeps_the_bottom_edge_onscreen() {
        // A window parked at the bottom preset grows (more rows bound):
        // reflow_height then restore_or_fallback/clamp_into together must
        // keep the whole thing onscreen with the bottom edge anchored.
        let mon = area(0, 0, 1920, 1080);
        let old = area(600, 980, 360, 100); // bottom = 1080
        assert!(is_bottom_anchored(old, mon));
        let grown = reflow_height(old, 400, true);
        let areas = [mon];
        let fallback = default_rect(mon, 360, 400);
        let got = restore_or_fallback(grown, &areas, fallback);
        assert_eq!(got.bottom(), 1080, "the bottom edge must still be exactly onscreen");
        assert_eq!(got.y, 680);
    }

    #[test]
    fn area_for_picks_the_monitor_the_rect_overlaps() {
        let left = area(0, 0, 1920, 1080);
        let right = area(1920, 0, 1920, 1080);
        let r = area(2000, 100, 360, 200);
        let areas = [left, right];
        let picked = area_for(r, &areas).unwrap();
        assert_eq!(*picked, right);
    }

    #[test]
    fn area_for_falls_back_to_the_first_monitor_when_nothing_overlaps() {
        let left = area(0, 0, 1920, 1080);
        let r = area(5000, 100, 360, 200);
        let areas = [left];
        let picked = area_for(r, &areas).unwrap();
        assert_eq!(*picked, left);
    }

    #[test]
    fn area_for_gives_none_with_no_monitors() {
        let r = area(0, 0, 360, 200);
        assert!(area_for(r, &[]).is_none());
    }
}
