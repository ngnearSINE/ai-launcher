//! Where the floating strip sits inside the taskbar once docked: next to one of the taskbar's
//! landmarks (the group of app icons, the widgets button, the notification area) or at a free
//! offset. Pure geometry in the taskbar's client pixels, so it is tested without a taskbar.
//!
//! Other apps dock their own windows into the taskbar too (AI Usage, system monitors). Their
//! spans are obstacles: the strip never covers one; where its anchor would put it on top of
//! one it moves aside (away from the anchor's landmark), so two docked apps share the taskbar
//! without either fighting for the same place.

/// Landmarks of one taskbar, in its client x (physical px).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DockMarks {
    /// Start, search, task view and app buttons: `(left, right)`.
    pub icons: Option<(i32, i32)>,
    /// The widgets (weather) button, when shown.
    pub widgets: Option<(i32, i32)>,
    /// Where the notification area (tray icons, clock) begins: the strip never goes past it.
    pub tray: i32,
    /// Windows other apps docked into the taskbar: `(left, right)` each.
    pub others: Vec<(i32, i32)>,
}

/// What the strip stays attached to. Saved as a string (`float_dock_anchor`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockAnchor {
    /// Right edge against the notification area (the default).
    Tray,
    /// Left edge right after the app icons.
    IconsEnd,
    /// Right edge just before the app icons (centred taskbars).
    IconsStart,
    /// Right edge just before the widgets button.
    WidgetsLeft,
    /// Left edge right after the widgets button.
    WidgetsRight,
    /// Wherever it was put, by its left edge.
    Free,
}

impl DockAnchor {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tray => "tray",
            Self::IconsEnd => "iconsEnd",
            Self::IconsStart => "iconsStart",
            Self::WidgetsLeft => "widgetsLeft",
            Self::WidgetsRight => "widgetsRight",
            Self::Free => "free",
        }
    }
    /// Whether the strip's right edge is the one kept against the landmark (it grows to the
    /// left); otherwise the left edge.
    pub fn holds_right(self) -> bool {
        matches!(self, Self::Tray | Self::IconsStart | Self::WidgetsLeft)
    }
    pub fn parse(s: &str) -> Self {
        match s {
            "iconsEnd" => Self::IconsEnd,
            "iconsStart" => Self::IconsStart,
            "widgetsLeft" => Self::WidgetsLeft,
            "widgetsRight" => Self::WidgetsRight,
            "free" => Self::Free,
            _ => Self::Tray,
        }
    }
}

/// Left edge of a strip `width` wide attached to `anchor` (`gap` px from the landmark), kept
/// inside `[0, tray - width]`. A landmark that is not there (widgets hidden) falls back to the
/// notification area; `free_x` is used for [`DockAnchor::Free`].
pub fn slot_x(anchor: DockAnchor, free_x: i32, width: i32, m: &DockMarks, gap: i32) -> i32 {
    let x = unclamped_x(anchor, free_x, width, m, gap).clamp(0, (m.tray - width).max(0));
    // Right-edge anchors make way to the left, left-edge ones to the right.
    let leftward = anchor.holds_right();
    clear_of_others(x, width, m, gap, leftward)
}

/// `x` moved off any other app's docked window (with `gap` around it): first toward
/// `leftward`, then the other way when that runs out of taskbar, else left where it was.
fn clear_of_others(x: i32, width: i32, m: &DockMarks, gap: i32, leftward: bool) -> i32 {
    let max = (m.tray - width).max(0);
    let hit = |x: i32| m.others.iter().copied().find(|&(l, r)| x < r + gap && x + width > l - gap);
    let slide = |mut x: i32, left: bool| -> Option<i32> {
        for _ in 0..=m.others.len() {
            match hit(x) {
                None => return (0..=max).contains(&x).then_some(x),
                Some((l, r)) => x = if left { l - gap - width } else { r + gap },
            }
        }
        None
    };
    slide(x, leftward).or_else(|| slide(x, !leftward)).unwrap_or(x)
}

fn unclamped_x(anchor: DockAnchor, free_x: i32, width: i32, m: &DockMarks, gap: i32) -> i32 {
    let tray = m.tray - gap - width;
    match anchor {
        DockAnchor::Tray => tray,
        DockAnchor::IconsEnd => m.icons.map_or(tray, |i| i.1 + gap),
        DockAnchor::IconsStart => m.icons.map_or(tray, |i| i.0 - gap - width),
        DockAnchor::WidgetsLeft => m.widgets.map_or(tray, |w| w.0 - gap - width),
        DockAnchor::WidgetsRight => m.widgets.map_or(tray, |w| w.1 + gap),
        DockAnchor::Free => free_x,
    }
}

/// While the strip is dragged along the taskbar: `raw` is where its left edge would be. Within
/// `dist` px of a landmark it snaps there (the nearest one wins); otherwise it stays free.
pub fn snap_slot(raw: i32, width: i32, m: &DockMarks, gap: i32, dist: i32) -> (DockAnchor, i32) {
    let max = (m.tray - width).max(0);
    let raw = raw.clamp(0, max);
    let mut candidates = vec![DockAnchor::Tray];
    if m.icons.is_some() {
        candidates.extend([DockAnchor::IconsEnd, DockAnchor::IconsStart]);
    }
    if m.widgets.is_some() {
        candidates.extend([DockAnchor::WidgetsLeft, DockAnchor::WidgetsRight]);
    }
    let best = candidates
        .into_iter()
        .filter_map(|a| {
            let want = unclamped_x(a, 0, width, m, gap);
            // A landmark the strip only reaches by being pushed back inside (no room before
            // icons that start at 0, none after the widgets) is no target. Where another app's
            // window is in the way, the target is beside that window.
            let x = slot_x(a, 0, width, m, gap);
            (want == want.clamp(0, max) && (x - raw).abs() <= dist).then_some((a, x))
        })
        .min_by_key(|(_, x)| (x - raw).abs());
    best.unwrap_or_else(|| {
        // Free: beside another app's window rather than over it, on the side it came from.
        let leftward = hit_center(raw, width, m).is_some_and(|c| raw + width / 2 < c);
        (DockAnchor::Free, clear_of_others(raw, width, m, gap, leftward))
    })
}

/// Centre of the other app's window `x` would overlap, if any.
fn hit_center(x: i32, width: i32, m: &DockMarks) -> Option<i32> {
    m.others.iter().find(|&&(l, r)| x < r && x + width > l).map(|&(l, r)| (l + r) / 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    // This PC: icons from 0 to 2563, weather 3338–3490, tray from 3502; a 511 px strip.
    const M: DockMarks = DockMarks { icons: Some((0, 2563)), widgets: Some((3338, 3490)), tray: 3502, others: Vec::new() };

    #[test]
    fn anchors_place_the_strip_beside_their_landmark() {
        assert_eq!(slot_x(DockAnchor::Tray, 0, 511, &M, 4), 3502 - 4 - 511);
        assert_eq!(slot_x(DockAnchor::IconsEnd, 0, 511, &M, 4), 2567);
        assert_eq!(slot_x(DockAnchor::WidgetsLeft, 0, 511, &M, 4), 3338 - 4 - 511);
        // Room only to the tray: after the widgets it is pushed back inside.
        assert_eq!(slot_x(DockAnchor::WidgetsRight, 0, 511, &M, 4), 3502 - 511);
        assert_eq!(slot_x(DockAnchor::Free, 1200, 511, &M, 4), 1200);
        // Widgets hidden: falls back to the tray.
        let no_widgets = DockMarks { widgets: None, ..M };
        assert_eq!(slot_x(DockAnchor::WidgetsLeft, 0, 511, &no_widgets, 4), 2987);
    }

    #[test]
    fn dragging_snaps_near_landmarks_and_is_free_elsewhere() {
        // Near the end of the icons.
        assert_eq!(snap_slot(2575, 511, &M, 4, 12), (DockAnchor::IconsEnd, 2567));
        // In the open: free, where it was dragged.
        assert_eq!(snap_slot(2000, 300, &M, 4, 12), (DockAnchor::Free, 2000));
        // Near the tray (also the far right limit).
        assert_eq!(snap_slot(2990, 511, &M, 4, 12), (DockAnchor::Tray, 2987));
        assert_eq!(snap_slot(5000, 511, &M, 4, 12), (DockAnchor::Tray, 2987));
        // Just before the widgets.
        assert_eq!(snap_slot(2830, 511, &M, 4, 12), (DockAnchor::WidgetsLeft, 2823));
        // No room before icons that start at 0: never a target, the left limit is free.
        assert_eq!(snap_slot(-40, 300, &M, 4, 12), (DockAnchor::Free, 0));
        // Magnet off.
        assert_eq!(snap_slot(2575, 511, &M, 4, 0), (DockAnchor::Free, 2575));
    }

    #[test]
    fn centred_icons_can_be_approached_from_the_left() {
        let m = DockMarks { icons: Some((1500, 2300)), widgets: None, tray: 3502, others: Vec::new() };
        assert_eq!(snap_slot(1190, 300, &m, 4, 12), (DockAnchor::IconsStart, 1196));
        assert_eq!(DockAnchor::parse(DockAnchor::IconsStart.as_str()), DockAnchor::IconsStart);
        assert_eq!(DockAnchor::parse("nonsense"), DockAnchor::Tray);
    }

    #[test]
    fn other_apps_docked_windows_are_left_alone() {
        // AI Usage sits against the tray (3200–3498): the strip goes just left of it.
        let m = DockMarks { others: vec![(3200, 3498)], ..M };
        assert_eq!(slot_x(DockAnchor::Tray, 0, 300, &m, 4), 3200 - 4 - 300);
        // After the icons there is room: it stays there.
        assert_eq!(slot_x(DockAnchor::IconsEnd, 0, 300, &m, 4), 2567);
        // A widget right after the icons: the strip waits after it.
        let m = DockMarks { others: vec![(2567, 2700)], ..M };
        assert_eq!(slot_x(DockAnchor::IconsEnd, 0, 300, &m, 4), 2704);
        // Dragged onto it from the left half: stays left of it; from the right half: right.
        assert_eq!(snap_slot(2450, 300, &m, 4, 0), (DockAnchor::Free, 2263));
        assert_eq!(snap_slot(2600, 300, &m, 4, 0), (DockAnchor::Free, 2704));
        // Near the tray with AI Usage there: it snaps beside AI Usage, still "tray".
        let m = DockMarks { others: vec![(3200, 3498)], ..M };
        assert_eq!(snap_slot(2890, 300, &m, 4, 12), (DockAnchor::Tray, 2896));
        // No room to the right of a window at the far end: back to its left.
        let m = DockMarks { others: vec![(3300, 3500)], ..M };
        assert_eq!(slot_x(DockAnchor::WidgetsRight, 0, 300, &m, 4), 2996);
    }
}
