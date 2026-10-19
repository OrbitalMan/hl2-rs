//! UI tuning values embedded from `assets/ui/tuning.json` and parsed once: numbers that shape
//! the UI but come from no game resource.

use serde::Deserialize;
use std::sync::LazyLock;

#[derive(Debug, Deserialize)]
struct Tuning {
    /// Width:height of the narrowest screen the 480-line UI design fits ([`crate::proportional`]).
    narrowest_design_aspect: (u32, u32),
    /// Width:height of the widest screen the menu keeps its retail size on; wider screens grow
    /// the menu ([`crate::proportional::widescreen_menu_scale`]).
    widest_design_aspect: (u32, u32),
    /// Largest growth of the menu on wide screens.
    widescreen_menu_scale_max: f32,
    /// Rows of the tallest menu: the stack whose bottom stays put when the menu grows.
    menu_stack_rows: u32,
    /// Seconds before an unconfirmed video change reverts (`gameui::video_confirm`).
    video_confirm_seconds: f64,
    /// Smallest and largest explicit vertical field of view of the Video > Advanced slider
    /// (below the smallest the slider means automatic).
    vertical_fov_range: (f32, f32),
    /// Smallest and largest viewmodel field of view of the Video > Advanced slider.
    viewmodel_fov_range: (f32, f32),
    /// Width:height of each centered HUD aspect of Video > HUD Aspect Ratio
    /// ([`crate::proportional::hud_frame`]).
    hud_aspects: HudAspects,
    /// Aspect-adaptive and per-weapon viewmodel framing outside `5:4..=16:9`.
    viewmodel: ViewmodelTuning,
}

/// The centered HUD aspects, width:height, named like [`crate::config::HudAspect`].
#[derive(Debug, Deserialize)]
struct HudAspects {
    normal: (u32, u32),
    wide16x9: (u32, u32),
    wide16x10: (u32, u32),
}

/// Aspect-adaptive viewmodel framing outside `narrowest_design_aspect..=widest_design_aspect`
/// (`5:4..=16:9`), with a default profile and per-weapon overrides.
#[derive(Debug, Deserialize)]
struct ViewmodelTuning {
    /// Width:height where the portrait framing reaches full strength (e.g. `9:16`).
    portrait_aspect: (u32, u32),
    /// Width:height where the ultrawide framing reaches full strength (e.g. `32:9`).
    ultrawide_aspect: (u32, u32),
    /// Framing applied to weapons without a per-weapon entry.
    default: ViewmodelWeaponTuning,
    /// Per-weapon framing keyed by weapon classname (e.g. `"weapon_physcannon"`, `"weapon_rpg"`).
    weapons: std::collections::BTreeMap<String, ViewmodelWeaponTuning>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct ViewmodelWeaponTuning {
    portrait: ViewmodelFraming,
    ultrawide: ViewmodelFraming,
}

/// Additive viewmodel field-of-view offset (degrees) and local camera-space translation offset
/// (`[forward, left, up]` in Source units) for frames narrower than `5:4` or wider than `16:9`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize)]
pub struct ViewmodelFraming {
    pub fov_offset: f32,
    pub offset: [f32; 3],
}

impl ViewmodelFraming {
    pub const ZERO: Self = Self {
        fov_offset: 0.0,
        offset: [0.0; 3],
    };

    fn scale(self, t: f32) -> Self {
        if t <= 0.0 {
            Self::ZERO
        } else {
            Self {
                fov_offset: self.fov_offset * t,
                offset: self.offset.map(|c| c * t),
            }
        }
    }
}

fn data() -> &'static Tuning {
    static DATA: LazyLock<Tuning> = LazyLock::new(|| {
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/ui/tuning.json"
        )))
        .expect("embedded UI tuning data is valid")
    });
    &DATA
}

/// Width / height of the narrowest screen the UI design fits.
pub fn narrowest_design_aspect() -> f32 {
    let (width, height) = data().narrowest_design_aspect;
    width as f32 / height as f32
}

/// Width / height of the widest screen the menu keeps its retail size on.
pub fn widest_design_aspect() -> f32 {
    let (width, height) = data().widest_design_aspect;
    width as f32 / height as f32
}

/// Largest growth of the menu on wide screens.
pub fn widescreen_menu_scale_max() -> f32 {
    data().widescreen_menu_scale_max
}

/// Rows of the tallest menu.
pub fn menu_stack_rows() -> f32 {
    data().menu_stack_rows as f32
}

/// Seconds before an unconfirmed video change reverts.
pub fn video_confirm_seconds() -> f64 {
    data().video_confirm_seconds
}

/// Smallest and largest explicit vertical field of view (degrees).
pub fn vertical_fov_range() -> (f32, f32) {
    data().vertical_fov_range
}

/// Smallest and largest viewmodel field of view (degrees).
pub fn viewmodel_fov_range() -> (f32, f32) {
    data().viewmodel_fov_range
}

/// Width / height the HUD is laid out in on a wider screen, or `None` for Fit Display.
pub fn hud_aspect_ratio(aspect: crate::config::HudAspect) -> Option<f32> {
    use crate::config::HudAspect;
    let (width, height) = match aspect {
        HudAspect::Fit => return None,
        HudAspect::Normal => data().hud_aspects.normal,
        HudAspect::Wide16x9 => data().hud_aspects.wide16x9,
        HudAspect::Wide16x10 => data().hud_aspects.wide16x10,
    };
    Some(width as f32 / height as f32)
}

/// Aspect-adaptive viewmodel framing (`fov_offset` and Source `[forward, left, up]` offset) for
/// `weapon` at `aspect` (`width / height`).
///
/// Inside `narrowest_design_aspect()..=widest_design_aspect()` (`5:4..=16:9`) this is
/// [`ViewmodelFraming::ZERO`] so standard retail aspects are untouched. Below `5:4` it
/// interpolates linearly toward the weapon's (or default) `portrait` framing at
/// `portrait_aspect` (`9:16`); above `16:9` it interpolates linearly toward the `ultrawide`
/// framing at `ultrawide_aspect` (`32:9`).
pub fn viewmodel_framing(weapon: &str, aspect: f32) -> ViewmodelFraming {
    if !aspect.is_finite() || aspect <= 0.0 {
        return ViewmodelFraming::ZERO;
    }
    let vm = &data().viewmodel;
    let profile = vm.weapons.get(weapon).copied().unwrap_or(vm.default);
    let narrow = narrowest_design_aspect();
    let wide = widest_design_aspect();
    if aspect < narrow {
        let (pw, ph) = vm.portrait_aspect;
        let portrait = pw as f32 / ph as f32;
        let span = (narrow - portrait).max(f32::EPSILON);
        let t = ((narrow - aspect) / span).clamp(0.0, 1.0);
        profile.portrait.scale(t)
    } else if aspect > wide {
        let (uw, uh) = vm.ultrawide_aspect;
        let ultrawide = uw as f32 / uh as f32;
        let span = (ultrawide - wide).max(f32::EPSILON);
        let t = ((aspect - wide) / span).clamp(0.0, 1.0);
        profile.ultrawide.scale(t)
    } else {
        ViewmodelFraming::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tuning_data_parses_with_usable_values() {
        assert!(narrowest_design_aspect() >= 1.0, "a landscape design");
        assert!(video_confirm_seconds() >= 5.0, "time to read and answer");
        assert!(widest_design_aspect() > narrowest_design_aspect());
        assert!(widescreen_menu_scale_max() >= 1.0 && menu_stack_rows() >= 1.0);
        assert_eq!(hud_aspect_ratio(crate::config::HudAspect::Fit), None);
        for aspect in &crate::config::HudAspect::ALL[1..] {
            assert!(
                hud_aspect_ratio(*aspect).is_some_and(|r| r >= 1.),
                "{aspect:?}"
            );
        }
        for (min, max) in [vertical_fov_range(), viewmodel_fov_range()] {
            assert!(0. < min && min < max && max < 180., "{min} {max}");
        }
    }

    #[test]
    fn viewmodel_framing_leaves_retail_aspects_untouched_and_adapts_portrait_and_ultrawide() {
        for aspect in [5. / 4., 4. / 3., 16. / 10., 16. / 9.] {
            for weapon in ["weapon_crowbar", "weapon_physcannon", "weapon_rpg"] {
                assert_eq!(
                    viewmodel_framing(weapon, aspect),
                    ViewmodelFraming::ZERO,
                    "{weapon} at {aspect} is untouched"
                );
            }
        }
        // Gravity Gun in portrait shifts down (negative Z) to hide the severed left arm stump
        // without narrowing horizontal FOV.
        let phys_portrait = viewmodel_framing("weapon_physcannon", 9. / 16.);
        assert_eq!(phys_portrait.fov_offset, 0.0);
        assert!(phys_portrait.offset[2] < -5.0, "{phys_portrait:?}");

        // Rocket Launcher in portrait widens FOV and shifts left (+Y) so the right-mounted tube
        // stays inside the narrow frame; in ultrawide it narrows FOV and shifts back/right/down
        // so the unmodeled rear cutoff stays outside the right edge.
        let rpg_portrait = viewmodel_framing("weapon_rpg", 9. / 16.);
        assert!(rpg_portrait.fov_offset > 15.0 && rpg_portrait.offset[1] > 0.0);
        let rpg_21_9 = viewmodel_framing("weapon_rpg", 64. / 27.);
        let rpg_32_9 = viewmodel_framing("weapon_rpg", 32. / 9.);
        assert!(rpg_21_9.fov_offset < 0.0 && rpg_32_9.fov_offset < rpg_21_9.fov_offset);
        assert!(rpg_32_9.offset[0] < 0.0 && rpg_32_9.offset[1] < 0.0);
    }
}
