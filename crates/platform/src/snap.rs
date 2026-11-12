//! Magnet for a small floating window: where a dragged rectangle should land so its edges line
//! up with the screen's work area or with other windows. Pure geometry, physical pixels.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self { left, top, right, bottom }
    }
    pub fn width(&self) -> i32 {
        self.right - self.left
    }
    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }
    fn shifted(self, dx: i32, dy: i32) -> Self {
        Self { left: self.left + dx, top: self.top + dy, right: self.right + dx, bottom: self.bottom + dy }
    }
}

/// Smallest shift (within `dist`) among the candidates, if any.
fn nearest(cands: impl Iterator<Item = i32>, dist: i32) -> i32 {
    cands.filter(|d| d.abs() <= dist).min_by_key(|d| d.abs()).unwrap_or(0)
}

/// Moves `r` (same size) so that an edge within `dist` of a target edge sits exactly on it.
///
/// - `areas` (monitor work areas): `r` snaps to their inner edges, so it can sit in a corner.
/// - `windows`: `r` snaps beside a window (its left edge to the window's right edge and so on)
///   or level with it (top to top…), but only when the two overlap or nearly overlap on the
///   other axis, so a window far above does not pull it sideways.
///
/// Each axis is handled on its own; `dist <= 0` turns the magnet off.
pub fn snap(r: Rect, areas: &[Rect], windows: &[Rect], dist: i32) -> Rect {
    if dist <= 0 {
        return r;
    }
    let near_y = |w: &Rect| r.top < w.bottom + dist && r.bottom > w.top - dist;
    let near_x = |w: &Rect| r.left < w.right + dist && r.right > w.left - dist;
    let dx = nearest(
        areas
            .iter()
            .flat_map(|a| [a.left - r.left, a.right - r.right])
            .chain(windows.iter().filter(|w| near_y(w)).flat_map(|w| [w.right - r.left, w.left - r.right, w.left - r.left, w.right - r.right])),
        dist,
    );
    let dy = nearest(
        areas
            .iter()
            .flat_map(|a| [a.top - r.top, a.bottom - r.bottom])
            .chain(windows.iter().filter(|w| near_x(w)).flat_map(|w| [w.bottom - r.top, w.top - r.bottom, w.top - r.top, w.bottom - r.bottom])),
        dist,
    );
    r.shifted(dx, dy)
}

/// Keeps the width:height ratio of `start` while one edge or corner of a window is dragged.
/// `edge` follows Windows `WMSZ_*` (1 left, 2 right, 3 top, 4 top-left, 5 top-right, 6 bottom,
/// 7 bottom-left, 8 bottom-right). The side opposite the dragged one stays put.
pub fn keep_ratio(r: Rect, start: Rect, edge: u32, min_w: i32) -> Rect {
    if start.width() <= 0 || start.height() <= 0 {
        return r;
    }
    let ratio = start.height() as f64 / start.width() as f64;
    // Which length drives: a side edge drives width, top/bottom drive height, a corner takes
    // whichever grew more relative to the start.
    let by_width = match edge {
        1 | 2 => true,
        3 | 6 => false,
        _ => (r.width() as f64 / start.width() as f64 - 1.0).abs() >= (r.height() as f64 / start.height() as f64 - 1.0).abs(),
    };
    let (w, h) = if by_width {
        let w = r.width().max(min_w);
        (w, (w as f64 * ratio).round() as i32)
    } else {
        let h = r.height().max((min_w as f64 * ratio).round() as i32);
        ((h as f64 / ratio).round() as i32, h)
    };
    let left_moves = matches!(edge, 1 | 4 | 7);
    let top_moves = matches!(edge, 3 | 4 | 5);
    let (left, right) = if left_moves { (r.right - w, r.right) } else { (r.left, r.left + w) };
    let (top, bottom) = if top_moves { (r.bottom - h, r.bottom) } else { (r.top, r.top + h) };
    Rect { left, top, right, bottom }
}

/// The rectangle a resize means: `start` with the dragged edge(s) moved by how far the cursor
/// moved (`dx`, `dy`) since the drag began. Used instead of the rectangle Windows proposes in
/// `WM_SIZING`, which is already widened to the system's minimum tracking size (136 px at
/// 100 %): a strip thinner than that jumped to ~1.6× on the first click (seen 2026-11-12).
pub fn drag_rect(start: Rect, edge: u32, dx: i32, dy: i32) -> Rect {
    let mut r = start;
    if matches!(edge, 1 | 4 | 7) {
        r.left += dx;
    }
    if matches!(edge, 2 | 5 | 8) {
        r.right += dx;
    }
    if matches!(edge, 3 | 4 | 5) {
        r.top += dy;
    }
    if matches!(edge, 6 | 7 | 8) {
        r.bottom += dy;
    }
    r
}

/// The floating strip's shape in physical px: a fixed part (handle, footer, padding) plus
/// `total` equal cells along its axis, and a constant thickness across.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Strip {
    pub vertical: bool,
    pub fixed: f64,
    pub cell: f64,
    pub total: usize,
    pub min_across: i32,
    /// Thickest the strip may get (physical px); `0` = no limit.
    pub max_across: i32,
}

/// What a resize of the strip means, by the edge being dragged (`WMSZ_*` numbering):
/// - a corner splits the drag: across the strip it sets the thickness (the icons scale with
///   it, 1:1 with the cursor), along the strip it sets how many cells show. Keeping the
///   width:height ratio instead made a bar grow ~9× faster along than across (2026-11-12);
/// - a long side sets the thickness only: the icons scale and the same cells stay, so the
///   length grows with them;
/// - an end of the strip (top/bottom when vertical, left/right when horizontal) shows one more
///   or one fewer cell at a time, so the length always fits whole cells;
pub fn strip_sizing(r: Rect, start: Rect, edge: u32, s: &Strip) -> Rect {
    let corner = matches!(edge, 4 | 5 | 7 | 8);
    let side = if s.vertical { matches!(edge, 1 | 2) } else { matches!(edge, 3 | 6) };
    let (across0, along0) = if s.vertical { (start.width(), start.height()) } else { (start.height(), start.width()) };
    let (want_across, want_along) = if s.vertical { (r.width(), r.height()) } else { (r.height(), r.width()) };
    let max = if s.max_across > 0 { s.max_across } else { i32::MAX };
    let across = want_across.clamp(s.min_across.max(8).min(max), max);
    // Everything scales with the thickness.
    let f = across as f64 / across0.max(1) as f64;
    if side {
        // Same cells, scaled: the length follows the thickness. The dragged side moves, the
        // opposite one and the start of the strip stay.
        let along = (along0 as f64 * f).round() as i32;
        let (w, h) = if s.vertical { (across, along) } else { (along, across) };
        let left = if edge == 1 { start.right - w } else { start.left };
        let top = if edge == 3 { start.bottom - h } else { start.top };
        return Rect { left, top, right: left + w, bottom: top + h };
    }
    if corner {
        // The length then takes whole cells at that scale.
        let along = if s.cell > 0.0 && s.total > 0 {
            let (fixed, cell) = (s.fixed * f, s.cell * f);
            let n = ((want_along as f64 - fixed) / cell).round().clamp(1.0, s.total as f64);
            (fixed + n * cell).round() as i32
        } else {
            (along0 as f64 * f).round() as i32
        };
        let (w, h) = if s.vertical { (across, along) } else { (along, across) };
        let left_moves = matches!(edge, 4 | 7);
        let top_moves = matches!(edge, 4 | 5);
        let (left, right) = if left_moves { (start.right - w, start.right) } else { (start.left, start.left + w) };
        let (top, bottom) = if top_moves { (start.bottom - h, start.bottom) } else { (start.top, start.top + h) };
        return Rect { left, top, right, bottom };
    }
    let along_edge = if s.vertical { matches!(edge, 3 | 6) } else { matches!(edge, 1 | 2) };
    if !along_edge || s.cell <= 0.0 || s.total == 0 {
        return start;
    }
    let len = if s.vertical { r.height() } else { r.width() } as f64;
    let n = ((len - s.fixed) / s.cell).round().clamp(1.0, s.total as f64);
    let len = (s.fixed + n * s.cell).round() as i32;
    match edge {
        3 => Rect { top: r.bottom - len, ..start.with_bottom(r.bottom) },
        6 => Rect { bottom: start.top + len, ..start },
        1 => Rect { left: r.right - len, ..start.with_right(r.right) },
        _ => Rect { right: start.left + len, ..start },
    }
}

/// Cells a strip of `len` physical px along its axis shows.
pub fn cells_for(len: f64, s: &Strip) -> usize {
    if s.cell <= 0.0 {
        return s.total;
    }
    (((len - s.fixed) / s.cell).round().max(1.0) as usize).min(s.total.max(1))
}

impl Rect {
    fn with_bottom(self, bottom: i32) -> Self {
        Self { bottom, ..self }
    }
    fn with_right(self, right: i32) -> Self {
        Self { right, ..self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect { left: 0, top: 0, right: 1920, bottom: 1040 };

    #[test]
    fn snaps_into_screen_corner() {
        let r = Rect::new(1830, 8, 1910, 400);
        assert_eq!(snap(r, &[SCREEN], &[], 12), Rect::new(1840, 0, 1920, 392));
    }

    #[test]
    fn far_from_everything_stays() {
        let r = Rect::new(800, 300, 880, 700);
        assert_eq!(snap(r, &[SCREEN], &[], 12), r);
        assert_eq!(snap(Rect::new(1830, 8, 1910, 400), &[SCREEN], &[], 0), Rect::new(1830, 8, 1910, 400));
    }

    #[test]
    fn sits_beside_a_window_and_levels_with_it() {
        let win = Rect::new(400, 200, 1200, 900);
        // Just right of the window, a little lower than its top: lands touching it, top level.
        let r = Rect::new(1206, 205, 1286, 600);
        assert_eq!(snap(r, &[SCREEN], &[win], 12), Rect::new(1200, 200, 1280, 595));
        // A window far above does not pull it sideways.
        let high = Rect::new(400, 0, 1200, 100);
        let r = Rect::new(1206, 500, 1286, 900);
        assert_eq!(snap(r, &[], &[high], 12), r);
    }

    #[test]
    fn corner_drag_splits_thickness_and_length() {
        // 86 px thick (below Windows' 136 px minimum), 74 + 11 × 66 = 800 px long, 20 icons.
        let start = Rect::new(3754, 468, 3840, 1268);
        let s = Strip { vertical: true, fixed: 74.0, cell: 66.0, total: 20, min_across: 55, max_across: 430 };
        let drag = |edge, dx, dy| strip_sizing(drag_rect(start, edge, dx, dy), start, edge, &s);
        // A click changes nothing.
        assert_eq!(drag(8, 0, 0), start);
        // Sideways: only thicker; the length keeps (nearly) its place, in whole cells.
        let r = drag(8, 10, 0);
        assert_eq!((r.width(), r.height()), (96, 819));
        // Down: one more cell, same thickness.
        let r = drag(8, 0, 66);
        assert_eq!((r.width(), r.height()), (86, 866));
        // Thickness stops at the limit instead of running ahead of the icons.
        assert_eq!(drag(8, 900, 0).width(), 430);
        // From the top-left corner the opposite corner stays.
        let r = drag(4, -10, 0);
        assert_eq!((r.right, r.bottom, r.width()), (3840, 1268, 96));
        // A horizontal strip swaps the roles: up/down is thickness, sideways is cells.
        let h = Rect::new(100, 100, 900, 186);
        let hs = Strip { vertical: false, ..s };
        let r = strip_sizing(drag_rect(h, 8, 0, 10), h, 8, &hs);
        assert_eq!((r.height(), r.width()), (96, 819));
        let r = strip_sizing(drag_rect(h, 8, 66, 0), h, 8, &hs);
        assert_eq!((r.height(), r.width()), (86, 866));
    }

    #[test]
    fn side_drag_scales_and_keeps_the_cells() {
        let start = Rect::new(3754, 468, 3840, 1268); // 86 × 800 = 74 + 11 × 66
        let s = Strip { vertical: true, fixed: 74.0, cell: 66.0, total: 20, min_across: 55, max_across: 430 };
        // Right side 43 px out: 1.5× — same 11 cells, so 1200 long; left and top stay.
        let r = strip_sizing(drag_rect(start, 2, 43, 0), start, 2, &s);
        assert_eq!(r, Rect::new(3754, 468, 3883, 1668));
        // Left side: the right edge stays. Up/down movement on a side is ignored.
        let r = strip_sizing(drag_rect(start, 1, -43, 30), start, 1, &s);
        assert_eq!((r.right, r.top, r.width(), r.height()), (3840, 468, 129, 1200));
        // A click changes nothing.
        assert_eq!(strip_sizing(drag_rect(start, 2, 0, 0), start, 2, &s), start);
        // Horizontal strip: the top / bottom sides do it.
        let h = Rect::new(100, 100, 900, 186);
        let hs = Strip { vertical: false, ..s };
        let r = strip_sizing(drag_rect(h, 6, 0, 43), h, 6, &hs);
        assert_eq!(r, Rect::new(100, 100, 1300, 229));
        // …and its left / right ends still count cells.
        assert_eq!(strip_sizing(drag_rect(h, 2, 66, 0), h, 2, &hs).height(), 86);
    }

    #[test]
    fn strip_ends_step_by_whole_cells() {
        let s = Strip { vertical: true, fixed: 60.0, cell: 50.0, total: 6, min_across: 20, max_across: 0 };
        let start = Rect::new(100, 100, 160, 360); // 60 + 4×50
        // Bottom dragged down by 30 → rounds to 5 cells (60+250=310).
        let r = strip_sizing(Rect::new(100, 100, 160, 390), start, 6, &s);
        assert_eq!(r, Rect::new(100, 100, 160, 410));
        // Top dragged far down → at least one cell, bottom stays.
        let r = strip_sizing(Rect::new(100, 340, 160, 360), start, 3, &s);
        assert_eq!(r, Rect::new(100, 250, 160, 360));
        // Never more cells than items.
        let r = strip_sizing(Rect::new(100, 100, 160, 900), start, 6, &s);
        assert_eq!(r.height(), 360);
        // Long sides scale instead (see `side_drag_scales_and_keeps_the_cells`).
        assert_eq!(strip_sizing(Rect::new(90, 100, 160, 360), start, 1, &s), Rect::new(90, 100, 160, 403));
        assert_eq!(cells_for(310.0, &s), 5);
        assert_eq!(cells_for(10.0, &s), 1);
    }

    #[test]
    fn ratio_kept_while_resizing() {
        let start = Rect::new(100, 100, 180, 500); // 80×400
        // Bottom-right corner dragged down by 100: height drives, width follows.
        let r = keep_ratio(Rect::new(100, 100, 182, 600), start, 8, 20);
        assert_eq!((r.width(), r.height()), (100, 500));
        assert_eq!((r.left, r.top), (100, 100));
        // Bottom-left corner: the right edge stays.
        let r = keep_ratio(Rect::new(80, 100, 180, 500), start, 7, 20);
        assert_eq!((r.right, r.top, r.width(), r.height()), (180, 100, 100, 500));
    }
}
