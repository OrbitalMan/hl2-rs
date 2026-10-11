//! Display-mode choices for the Video page: aspect-ratio buckets and resolution filtering.
//!
//! Resolutions come from the host's monitor query (Bevy `Monitor::video_modes`); nothing here is a
//! hard-coded list. The aspect buckets follow the retail Video page grouping: ratios below 1.5 are
//! "Normal (4:3)" (this also takes 5:4), ratios within 0.05 of 1.6 are 16:10, everything wider is
//! 16:9. When the host reports no usable mode, the current window size is offered so the list is
//! never empty.

use serde::{Deserialize, Serialize};

/// Smallest mode the Video page offers (Source rejects smaller desktop modes as well).
pub const MIN_SIZE: (u32, u32) = (640, 480);

/// The Aspect Ratio combo choices.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AspectRatio {
    /// `#GameUI_AspectNormal`.
    Normal,
    /// `#GameUI_AspectWide16x9`.
    #[default]
    Wide16x9,
    /// `#GameUI_AspectWide16x10`.
    Wide16x10,
}

impl AspectRatio {
    pub const ALL: [Self; 3] = [Self::Normal, Self::Wide16x9, Self::Wide16x10];

    /// Bucket of a width/height pair.
    pub fn of(width: u32, height: u32) -> Self {
        let ratio = width as f32 / height.max(1) as f32;
        if ratio < 1.5 {
            Self::Normal
        } else if (ratio - 1.6).abs() < 0.05 {
            Self::Wide16x10
        } else {
            Self::Wide16x9
        }
    }

    pub fn token(self) -> &'static str {
        match self {
            Self::Normal => "#GameUI_AspectNormal",
            Self::Wide16x9 => "#GameUI_AspectWide16x9",
            Self::Wide16x10 => "#GameUI_AspectWide16x10",
        }
    }
}

/// One selectable resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

impl Resolution {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    pub fn aspect(self) -> AspectRatio {
        AspectRatio::of(self.width, self.height)
    }

    pub fn label(self) -> String {
        format!("{} x {}", self.width, self.height)
    }
}

/// Usable modes, deduplicated and sorted largest first. Modes smaller than [`MIN_SIZE`] or larger
/// than the monitor (when known) are dropped. Falls back to `current` when nothing remains.
pub fn usable_modes(
    reported: impl IntoIterator<Item = Resolution>,
    monitor: Option<Resolution>,
    current: Resolution,
) -> Vec<Resolution> {
    let mut modes: Vec<_> = reported
        .into_iter()
        .filter(|m| m.width >= MIN_SIZE.0 && m.height >= MIN_SIZE.1)
        .filter(|m| monitor.is_none_or(|max| m.width <= max.width && m.height <= max.height))
        .collect();
    modes.sort_by(|a, b| b.cmp(a));
    modes.dedup();
    if modes.is_empty() {
        modes.push(current);
    }
    modes
}

/// Modes of one aspect bucket, keeping the largest-first order.
pub fn modes_for(modes: &[Resolution], aspect: AspectRatio) -> Vec<Resolution> {
    modes
        .iter()
        .copied()
        .filter(|m| m.aspect() == aspect)
        .collect()
}

/// The resolution selected after the Aspect Ratio combo changes: the previous one if it is still
/// listed, otherwise the largest mode of the new bucket (empty buckets fall back to all modes).
pub fn choose_after_aspect_change(
    modes: &[Resolution],
    aspect: AspectRatio,
    previous: Resolution,
) -> Option<Resolution> {
    let candidates = resolution_choices(modes, aspect);
    if candidates.contains(&previous) {
        Some(previous)
    } else {
        candidates.first().copied()
    }
}

/// The list shown in the Resolution combo for an aspect bucket; empty buckets show all modes.
pub fn resolution_choices(modes: &[Resolution], aspect: AspectRatio) -> Vec<Resolution> {
    let bucket = modes_for(modes, aspect);
    if bucket.is_empty() {
        modes.to_vec()
    } else {
        bucket
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(width: u32, height: u32) -> Resolution {
        Resolution::new(width, height)
    }

    #[test]
    fn aspect_buckets_match_the_video_page_grouping() {
        assert_eq!(AspectRatio::of(1024, 768), AspectRatio::Normal);
        assert_eq!(
            AspectRatio::of(1280, 1024),
            AspectRatio::Normal,
            "5:4 joins 4:3"
        );
        assert_eq!(AspectRatio::of(1680, 1050), AspectRatio::Wide16x10);
        assert_eq!(
            AspectRatio::of(1512, 982),
            AspectRatio::Wide16x9,
            "1.54 is wider than 16:10's window"
        );
        assert_eq!(AspectRatio::of(1920, 1080), AspectRatio::Wide16x9);
        assert_eq!(AspectRatio::of(3440, 1440), AspectRatio::Wide16x9);
        assert_eq!(
            AspectRatio::of(10, 0),
            AspectRatio::Wide16x9,
            "zero height is safe"
        );
    }

    #[test]
    fn modes_are_filtered_sorted_deduplicated_with_a_fallback() {
        let modes = usable_modes(
            [
                r(800, 600),
                r(320, 240),
                r(1920, 1080),
                r(800, 600),
                r(2560, 1440),
                r(1280, 800),
            ],
            Some(r(1920, 1200)),
            r(1280, 720),
        );
        assert_eq!(modes, [r(1920, 1080), r(1280, 800), r(800, 600)]);
        assert_eq!(usable_modes([], None, r(1280, 720)), [r(1280, 720)]);
        assert_eq!(
            usable_modes([r(320, 200)], None, r(1024, 768)),
            [r(1024, 768)]
        );
    }

    #[test]
    fn aspect_change_keeps_a_valid_resolution() {
        let modes = [
            r(1920, 1080),
            r(1680, 1050),
            r(1280, 720),
            r(1024, 768),
            r(800, 600),
        ];
        assert_eq!(
            resolution_choices(&modes, AspectRatio::Normal),
            [r(1024, 768), r(800, 600)]
        );
        assert_eq!(
            choose_after_aspect_change(&modes, AspectRatio::Normal, r(1920, 1080)),
            Some(r(1024, 768)),
            "largest mode of the new bucket"
        );
        assert_eq!(
            choose_after_aspect_change(&modes, AspectRatio::Wide16x9, r(1280, 720)),
            Some(r(1280, 720)),
            "a still-valid selection is kept"
        );
        let only_wide = [r(1920, 1080)];
        assert_eq!(
            resolution_choices(&only_wide, AspectRatio::Wide16x10),
            [r(1920, 1080)]
        );
        assert_eq!(
            choose_after_aspect_change(&only_wide, AspectRatio::Wide16x10, r(800, 600)),
            Some(r(1920, 1080)),
            "empty bucket falls back to every mode"
        );
        assert_eq!(
            choose_after_aspect_change(&[], AspectRatio::Normal, r(800, 600)),
            None
        );
    }
}
