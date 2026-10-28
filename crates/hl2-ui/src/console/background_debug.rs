//! Console commands and state for background visual debug mode.

use super::commands::{Outcome, Spec};
use super::{Console, Effect};
use source_ui::console::command::{boolean, parse};

impl Console {
    /// Whether background debug mode can currently be enabled (background map or blur present).
    pub fn can_enable_background_debug(&self) -> bool {
        self.background_level()
            || self.is_background_loading()
            || self
                .loading_dialog
                .as_ref()
                .is_some_and(|l| l.is_background())
            || self.base == super::Mode::MainMenu
    }

    /// Pre-blurred image name applied to the given background map.
    pub fn blur_texture_name(&self, map: &str, widescreen: bool) -> String {
        let base_map = crate::gameui::resources::blur(map);
        if widescreen {
            let wide_key = format!("{base_map}_widescreen");
            let found = self
                .background_textures
                .keys()
                .any(|(_, name)| name == &wide_key);
            if found {
                return wide_key;
            }
        }
        base_map.to_owned()
    }

    /// Whether the current canvas aspect ratio is widescreen (> 4:3).
    pub fn is_widescreen(&self) -> bool {
        self.canvas.width() / self.canvas.height().max(1.0) > 4. / 3. + 1e-3
    }
}

pub(super) fn background_debug_mode(
    console: &mut Console,
    spec: &'static Spec,
    args: &[String],
) -> Outcome {
    let [value] = args else {
        return Err(spec.usage());
    };
    let Some(on) = boolean(value) else {
        return Err(spec.usage());
    };
    if on {
        if !console.can_enable_background_debug() {
            console.log("background_debug_mode needs a background map or loading blur; start or return to the main menu.");
            return Ok(None);
        }
        console.background_debug_mode = true;
        console.log("background_debug_mode 1");
        Ok(Some(Effect::BackgroundDebugMode(true)))
    } else {
        console.background_debug_mode = false;
        console.log("background_debug_mode 0");
        Ok(Some(Effect::BackgroundDebugMode(false)))
    }
}

pub(super) fn background_blur_opacity(
    console: &mut Console,
    spec: &'static Spec,
    args: &[String],
) -> Outcome {
    if args.is_empty() {
        console.log(format!(
            "\"background_blur_opacity\" = \"{}\"",
            console.background_blur_opacity
        ));
        return Ok(None);
    }
    let [value] = args else {
        return Err(spec.usage());
    };
    if !console.background_debug_mode {
        console.log("background_blur_opacity requires background_debug_mode 1.");
        return Ok(None);
    }
    let opacity: u8 = parse(value).map_err(|_| spec.usage())?;
    if opacity > 100 {
        return Err(spec.usage());
    }
    console.background_blur_opacity = opacity;
    console.log(format!("background_blur_opacity {opacity}"));
    Ok(Some(Effect::BackgroundBlurOpacity(opacity)))
}

pub(super) fn background_map_fov_horizontal(
    _console: &mut Console,
    spec: &'static Spec,
    args: &[String],
) -> Outcome {
    if args.is_empty() {
        return Ok(Some(Effect::BackgroundMapFovHorizontal(None)));
    }
    let [value] = args else {
        return Err(spec.usage());
    };
    let deg: f32 = parse(value).map_err(|_| spec.usage())?;
    if !(1.0..=170.0).contains(&deg) {
        return Err(spec.usage());
    }
    Ok(Some(Effect::BackgroundMapFovHorizontal(Some(deg))))
}

pub(super) fn background_map_fov_vertical(
    _console: &mut Console,
    spec: &'static Spec,
    args: &[String],
) -> Outcome {
    if args.is_empty() {
        return Ok(Some(Effect::BackgroundMapFovVertical(None)));
    }
    let [value] = args else {
        return Err(spec.usage());
    };
    let deg: f32 = parse(value).map_err(|_| spec.usage())?;
    if !(1.0..=170.0).contains(&deg) {
        return Err(spec.usage());
    }
    Ok(Some(Effect::BackgroundMapFovVertical(Some(deg))))
}
