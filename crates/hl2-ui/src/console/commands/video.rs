//! Video cvars (the Options Video page and its Advanced dialog): fields of view, the video mode
//! and the material/render detail settings.

use super::{Outcome, Spec};
use crate::config::{
    Antialiasing, DisplayMode, Filtering, HdrMode, ModelDetail, ShaderDetail, ShadowDetail,
    TextureDetail, WaterDetail,
};
use crate::console::{boolean, Console, Effect};

/// `fov` and its alias `fov_desired`.
pub(super) fn fov(console: &mut Console, spec: &'static Spec, args: &[String]) -> Outcome {
    if args.is_empty() {
        console.log(format!("\"fov\" = \"{:.0}\"", console.config.video.fov));
    } else if args.len() == 1 {
        let fov: f32 = args[0].parse().map_err(|_| spec.usage())?;
        let fov = fov.clamp(50.0, 120.0);
        console.config.video.fov = fov;
        let _ = console.config.save();
        console.log(format!("fov {:.0}", fov));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn fov_value(console: &Console) -> Option<String> {
    Some(format!("{:.0}", console.config.video.fov))
}

pub(super) fn vertical_fov(console: &mut Console, spec: &'static Spec, args: &[String]) -> Outcome {
    if args.is_empty() {
        console.log(format!(
            "\"vertical_fov\" = \"{:.0}\"",
            console.config.video.vertical_fov
        ));
    } else if args.len() == 1 {
        let fov: f32 = args[0].parse().map_err(|_| spec.usage())?;
        let fov = if fov <= 0.0 {
            0.0
        } else {
            fov.clamp(40.0, 100.0)
        };
        console.config.video.vertical_fov = fov;
        let _ = console.config.save();
        console.log(format!("vertical_fov {:.0}", fov));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn vertical_fov_value(console: &Console) -> Option<String> {
    Some(format!("{:.0}", console.config.video.vertical_fov))
}

pub(super) fn viewmodel_fov(
    console: &mut Console,
    spec: &'static Spec,
    args: &[String],
) -> Outcome {
    if args.is_empty() {
        console.log(format!(
            "\"viewmodel_fov\" = \"{:.0}\"",
            console.config.video.viewmodel_fov
        ));
    } else if args.len() == 1 {
        let fov: f32 = args[0].parse().map_err(|_| spec.usage())?;
        let fov = fov.clamp(40.0, 80.0);
        console.config.video.viewmodel_fov = fov;
        let _ = console.config.save();
        console.log(format!("viewmodel_fov {:.0}", fov));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn viewmodel_fov_value(console: &Console) -> Option<String> {
    Some(format!("{:.0}", console.config.video.viewmodel_fov))
}

pub(super) fn mat_colorcorrection(
    console: &mut Console,
    spec: &'static Spec,
    args: &[String],
) -> Outcome {
    if args.is_empty() {
        let val = u8::from(console.config.video.advanced.color_correction);
        console.log(format!("\"mat_colorcorrection\" = \"{val}\""));
    } else if args.len() == 1 {
        let on = boolean(&args[0]).ok_or_else(|| spec.usage())?;
        console.config.video.advanced.color_correction = on;
        let _ = console.config.save();
        console.log(format!("mat_colorcorrection {}", u8::from(on)));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn mat_colorcorrection_value(console: &Console) -> Option<String> {
    Some(u8::from(console.config.video.advanced.color_correction).to_string())
}

pub(super) fn mat_hdr_level(
    console: &mut Console,
    spec: &'static Spec,
    args: &[String],
) -> Outcome {
    if args.is_empty() {
        let lvl = console.config.video.hdr.level();
        console.log(format!("\"mat_hdr_level\" = \"{lvl}\""));
    } else if args.len() == 1 {
        let lvl: u8 = args[0].parse().map_err(|_| spec.usage())?;
        console.config.video.hdr = HdrMode::from_level(lvl)
            .ok_or_else(|| "mat_hdr_level must be 0 (None), 1 (Bloom) or 2 (Full)".to_owned())?;
        let _ = console.config.save();
        console.log(format!("mat_hdr_level {lvl}"));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn mat_hdr_level_value(console: &Console) -> Option<String> {
    Some(console.config.video.hdr.level().to_string())
}

pub(super) fn mat_motion_blur_enabled(
    console: &mut Console,
    spec: &'static Spec,
    args: &[String],
) -> Outcome {
    if args.is_empty() {
        let val = u8::from(console.config.video.advanced.motion_blur);
        console.log(format!("\"mat_motion_blur_enabled\" = \"{val}\""));
    } else if args.len() == 1 {
        let on = boolean(&args[0]).ok_or_else(|| spec.usage())?;
        console.config.video.advanced.motion_blur = on;
        let _ = console.config.save();
        console.log(format!("mat_motion_blur_enabled {}", u8::from(on)));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn mat_motion_blur_enabled_value(console: &Console) -> Option<String> {
    Some(u8::from(console.config.video.advanced.motion_blur).to_string())
}

pub(super) fn mat_queue_mode(
    console: &mut Console,
    spec: &'static Spec,
    args: &[String],
) -> Outcome {
    if args.is_empty() {
        let val = if console.config.video.advanced.multicore {
            "-1"
        } else {
            "0"
        };
        console.log(format!("\"mat_queue_mode\" = \"{val}\""));
    } else if args.len() == 1 {
        let mode: i32 = args[0].parse().map_err(|_| spec.usage())?;
        let multicore = mode != 0;
        console.config.video.advanced.multicore = multicore;
        let _ = console.config.save();
        console.log(format!(
            "mat_queue_mode {}",
            if multicore { "-1" } else { "0" }
        ));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn mat_queue_mode_value(console: &Console) -> Option<String> {
    Some(if console.config.video.advanced.multicore {
        "-1".to_owned()
    } else {
        "0".to_owned()
    })
}

pub(super) fn mat_antialias(
    console: &mut Console,
    spec: &'static Spec,
    args: &[String],
) -> Outcome {
    if args.is_empty() {
        let val = match console.config.video.advanced.antialiasing {
            Antialiasing::None => 0,
            Antialiasing::X2 => 2,
            Antialiasing::X4 => 4,
            Antialiasing::X8 => 8,
        };
        console.log(format!("\"mat_antialias\" = \"{val}\""));
    } else if args.len() == 1 {
        let aa = match args[0].as_str() {
            "0" | "1" => Antialiasing::None,
            "2" => Antialiasing::X2,
            "4" => Antialiasing::X4,
            "8" => Antialiasing::X8,
            _ => return Err(spec.usage()),
        };
        console.config.video.advanced.antialiasing = aa;
        let _ = console.config.save();
        let val = match aa {
            Antialiasing::None => 0,
            Antialiasing::X2 => 2,
            Antialiasing::X4 => 4,
            Antialiasing::X8 => 8,
        };
        console.log(format!("mat_antialias {val}"));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn mat_antialias_value(console: &Console) -> Option<String> {
    Some(
        match console.config.video.advanced.antialiasing {
            Antialiasing::None => "0",
            Antialiasing::X2 => "2",
            Antialiasing::X4 => "4",
            Antialiasing::X8 => "8",
        }
        .to_string(),
    )
}

pub(super) fn mat_aaquality(
    console: &mut Console,
    spec: &'static Spec,
    args: &[String],
) -> Outcome {
    if args.is_empty() {
        console.log("\"mat_aaquality\" = \"0\"");
    } else if args.len() == 1 {
        let q: u32 = args[0].parse().map_err(|_| spec.usage())?;
        console.log(format!("mat_aaquality {q}"));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn mat_aaquality_value(_console: &Console) -> Option<String> {
    Some("0".to_owned())
}

pub(super) fn mat_disable_d3d9ex(
    console: &mut Console,
    spec: &'static Spec,
    args: &[String],
) -> Outcome {
    if args.is_empty() {
        let val = u8::from(!console.config.video.advanced.d3d9ex);
        console.log(format!("\"mat_disable_d3d9ex\" = \"{val}\""));
    } else if args.len() == 1 {
        let disabled = boolean(&args[0]).ok_or_else(|| spec.usage())?;
        console.config.video.advanced.d3d9ex = !disabled;
        let _ = console.config.save();
        console.log(format!("mat_disable_d3d9ex {}", u8::from(disabled)));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn mat_disable_d3d9ex_value(console: &Console) -> Option<String> {
    Some(u8::from(!console.config.video.advanced.d3d9ex).to_string())
}

pub(super) fn mat_forceaniso(
    console: &mut Console,
    spec: &'static Spec,
    args: &[String],
) -> Outcome {
    if args.is_empty() {
        let val = match console.config.video.advanced.filtering {
            Filtering::Bilinear | Filtering::Trilinear => 1,
            Filtering::Anisotropic2 => 2,
            Filtering::Anisotropic4 => 4,
            Filtering::Anisotropic8 => 8,
            Filtering::Anisotropic16 => 16,
        };
        console.log(format!("\"mat_forceaniso\" = \"{val}\""));
    } else if args.len() == 1 {
        let aniso = match args[0].as_str() {
            "0" | "1" => {
                if matches!(console.config.video.advanced.filtering, Filtering::Bilinear) {
                    Filtering::Bilinear
                } else {
                    Filtering::Trilinear
                }
            }
            "2" => Filtering::Anisotropic2,
            "4" => Filtering::Anisotropic4,
            "8" => Filtering::Anisotropic8,
            "16" => Filtering::Anisotropic16,
            _ => return Err(spec.usage()),
        };
        console.config.video.advanced.filtering = aniso;
        let _ = console.config.save();
        let val = match aniso {
            Filtering::Bilinear | Filtering::Trilinear => 1,
            Filtering::Anisotropic2 => 2,
            Filtering::Anisotropic4 => 4,
            Filtering::Anisotropic8 => 8,
            Filtering::Anisotropic16 => 16,
        };
        console.log(format!("mat_forceaniso {val}"));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn mat_forceaniso_value(console: &Console) -> Option<String> {
    Some(
        match console.config.video.advanced.filtering {
            Filtering::Bilinear | Filtering::Trilinear => "1",
            Filtering::Anisotropic2 => "2",
            Filtering::Anisotropic4 => "4",
            Filtering::Anisotropic8 => "8",
            Filtering::Anisotropic16 => "16",
        }
        .to_string(),
    )
}

pub(super) fn mat_trilinear(
    console: &mut Console,
    spec: &'static Spec,
    args: &[String],
) -> Outcome {
    if args.is_empty() {
        let val = u8::from(console.config.video.advanced.filtering != Filtering::Bilinear);
        console.log(format!("\"mat_trilinear\" = \"{val}\""));
    } else if args.len() == 1 {
        let on = boolean(&args[0]).ok_or_else(|| spec.usage())?;
        console.config.video.advanced.filtering =
            match (on, console.config.video.advanced.filtering) {
                (false, _) => Filtering::Bilinear,
                (true, Filtering::Bilinear) => Filtering::Trilinear,
                (true, other) => other,
            };
        let _ = console.config.save();
        console.log(format!("mat_trilinear {}", u8::from(on)));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn mat_trilinear_value(console: &Console) -> Option<String> {
    Some(u8::from(console.config.video.advanced.filtering != Filtering::Bilinear).to_string())
}

pub(super) fn mat_vsync(console: &mut Console, spec: &'static Spec, args: &[String]) -> Outcome {
    if args.is_empty() {
        let val = u8::from(console.config.video.advanced.vsync);
        console.log(format!("\"mat_vsync\" = \"{val}\""));
    } else if args.len() == 1 {
        let on = boolean(&args[0]).ok_or_else(|| spec.usage())?;
        console.config.video.advanced.vsync = on;
        let _ = console.config.save();
        console.log(format!("mat_vsync {}", u8::from(on)));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn mat_vsync_value(console: &Console) -> Option<String> {
    Some(u8::from(console.config.video.advanced.vsync).to_string())
}

pub(super) fn mat_picmip(console: &mut Console, spec: &'static Spec, args: &[String]) -> Outcome {
    if args.is_empty() {
        let val = match console.config.video.advanced.texture_detail {
            TextureDetail::VeryHigh => -1,
            TextureDetail::High => 0,
            TextureDetail::Medium => 1,
            TextureDetail::Low => 2,
        };
        console.log(format!("\"mat_picmip\" = \"{val}\""));
    } else if args.len() == 1 {
        let detail = match args[0].as_str() {
            "-1" => TextureDetail::VeryHigh,
            "0" => TextureDetail::High,
            "1" => TextureDetail::Medium,
            "2" => TextureDetail::Low,
            _ => return Err(spec.usage()),
        };
        console.config.video.advanced.texture_detail = detail;
        let _ = console.config.save();
        let val = match detail {
            TextureDetail::VeryHigh => -1,
            TextureDetail::High => 0,
            TextureDetail::Medium => 1,
            TextureDetail::Low => 2,
        };
        console.log(format!("mat_picmip {val}"));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn mat_picmip_value(console: &Console) -> Option<String> {
    Some(
        match console.config.video.advanced.texture_detail {
            TextureDetail::VeryHigh => "-1",
            TextureDetail::High => "0",
            TextureDetail::Medium => "1",
            TextureDetail::Low => "2",
        }
        .to_string(),
    )
}

pub(super) fn mat_reducefillrate(
    console: &mut Console,
    spec: &'static Spec,
    args: &[String],
) -> Outcome {
    if args.is_empty() {
        let val = u8::from(console.config.video.advanced.shader_detail == ShaderDetail::Low);
        console.log(format!("\"mat_reducefillrate\" = \"{val}\""));
    } else if args.len() == 1 {
        let on = boolean(&args[0]).ok_or_else(|| spec.usage())?;
        console.config.video.advanced.shader_detail = if on {
            ShaderDetail::Low
        } else {
            ShaderDetail::High
        };
        let _ = console.config.save();
        console.log(format!("mat_reducefillrate {}", u8::from(on)));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn mat_reducefillrate_value(console: &Console) -> Option<String> {
    Some(u8::from(console.config.video.advanced.shader_detail == ShaderDetail::Low).to_string())
}

pub(super) fn r_classic_blood(
    console: &mut Console,
    spec: &'static Spec,
    args: &[String],
) -> Outcome {
    if args.is_empty() {
        let val = u8::from(console.config.video.advanced.classic_effects);
        console.log(format!("\"r_classic_blood\" = \"{val}\""));
    } else if args.len() == 1 {
        let on = boolean(&args[0]).ok_or_else(|| spec.usage())?;
        console.config.video.advanced.classic_effects = on;
        let _ = console.config.save();
        console.log(format!("r_classic_blood {}", u8::from(on)));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn r_classic_fire(
    console: &mut Console,
    spec: &'static Spec,
    args: &[String],
) -> Outcome {
    if args.is_empty() {
        let val = u8::from(console.config.video.advanced.classic_effects);
        console.log(format!("\"r_classic_fire\" = \"{val}\""));
    } else if args.len() == 1 {
        let on = boolean(&args[0]).ok_or_else(|| spec.usage())?;
        console.config.video.advanced.classic_effects = on;
        let _ = console.config.save();
        console.log(format!("r_classic_fire {}", u8::from(on)));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

/// `r_classic_blood` and `r_classic_fire` share one setting.
pub(super) fn classic_effects_value(console: &Console) -> Option<String> {
    Some(u8::from(console.config.video.advanced.classic_effects).to_string())
}

pub(super) fn r_rootlod(console: &mut Console, spec: &'static Spec, args: &[String]) -> Outcome {
    if args.is_empty() {
        let val = match console.config.video.advanced.model_detail {
            ModelDetail::High => 0,
            ModelDetail::Medium => 1,
            ModelDetail::Low => 2,
        };
        console.log(format!("\"r_rootlod\" = \"{val}\""));
    } else if args.len() == 1 {
        let detail = match args[0].as_str() {
            "0" => ModelDetail::High,
            "1" => ModelDetail::Medium,
            "2" => ModelDetail::Low,
            _ => return Err(spec.usage()),
        };
        console.config.video.advanced.model_detail = detail;
        let _ = console.config.save();
        let val = match detail {
            ModelDetail::High => 0,
            ModelDetail::Medium => 1,
            ModelDetail::Low => 2,
        };
        console.log(format!("r_rootlod {val}"));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn r_rootlod_value(console: &Console) -> Option<String> {
    Some(
        match console.config.video.advanced.model_detail {
            ModelDetail::High => "0",
            ModelDetail::Medium => "1",
            ModelDetail::Low => "2",
        }
        .to_string(),
    )
}

pub(super) fn r_waterforceexpensive(
    console: &mut Console,
    spec: &'static Spec,
    args: &[String],
) -> Outcome {
    if args.is_empty() {
        let val = u8::from(console.config.video.advanced.water_detail != WaterDetail::Simple);
        console.log(format!("\"r_waterforceexpensive\" = \"{val}\""));
    } else if args.len() == 1 {
        let on = boolean(&args[0]).ok_or_else(|| spec.usage())?;
        console.config.video.advanced.water_detail =
            match (on, console.config.video.advanced.water_detail) {
                (false, _) => WaterDetail::Simple,
                (true, WaterDetail::Simple) => WaterDetail::World,
                (true, other) => other,
            };
        let _ = console.config.save();
        console.log(format!("r_waterforceexpensive {}", u8::from(on)));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn r_waterforceexpensive_value(console: &Console) -> Option<String> {
    Some(u8::from(console.config.video.advanced.water_detail != WaterDetail::Simple).to_string())
}

pub(super) fn r_waterforcereflectentities(
    console: &mut Console,
    spec: &'static Spec,
    args: &[String],
) -> Outcome {
    if args.is_empty() {
        let val = u8::from(console.config.video.advanced.water_detail == WaterDetail::All);
        console.log(format!("\"r_waterforcereflectentities\" = \"{val}\""));
    } else if args.len() == 1 {
        let on = boolean(&args[0]).ok_or_else(|| spec.usage())?;
        console.config.video.advanced.water_detail = if on {
            WaterDetail::All
        } else if console.config.video.advanced.water_detail == WaterDetail::All {
            WaterDetail::World
        } else {
            console.config.video.advanced.water_detail
        };
        let _ = console.config.save();
        console.log(format!("r_waterforcereflectentities {}", u8::from(on)));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn r_waterforcereflectentities_value(console: &Console) -> Option<String> {
    Some(u8::from(console.config.video.advanced.water_detail == WaterDetail::All).to_string())
}

pub(super) fn r_shadowrendertotexture(
    console: &mut Console,
    spec: &'static Spec,
    args: &[String],
) -> Outcome {
    if args.is_empty() {
        let val = u8::from(console.config.video.advanced.shadow_detail != ShadowDetail::Low);
        console.log(format!("\"r_shadowrendertotexture\" = \"{val}\""));
    } else if args.len() == 1 {
        let on = boolean(&args[0]).ok_or_else(|| spec.usage())?;
        console.config.video.advanced.shadow_detail =
            match (on, console.config.video.advanced.shadow_detail) {
                (false, _) => ShadowDetail::Low,
                (true, ShadowDetail::Low) => ShadowDetail::High,
                (true, other) => other,
            };
        let _ = console.config.save();
        console.log(format!("r_shadowrendertotexture {}", u8::from(on)));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn r_shadowrendertotexture_value(console: &Console) -> Option<String> {
    Some(u8::from(console.config.video.advanced.shadow_detail != ShadowDetail::Low).to_string())
}

pub(super) fn r_flashlightdepthtexture(
    console: &mut Console,
    spec: &'static Spec,
    args: &[String],
) -> Outcome {
    if args.is_empty() {
        let val = u8::from(console.config.video.advanced.shadow_detail == ShadowDetail::High);
        console.log(format!("\"r_flashlightdepthtexture\" = \"{val}\""));
    } else if args.len() == 1 {
        let on = boolean(&args[0]).ok_or_else(|| spec.usage())?;
        console.config.video.advanced.shadow_detail = if on {
            ShadowDetail::High
        } else if console.config.video.advanced.shadow_detail == ShadowDetail::High {
            ShadowDetail::Medium
        } else {
            console.config.video.advanced.shadow_detail
        };
        let _ = console.config.save();
        console.log(format!("r_flashlightdepthtexture {}", u8::from(on)));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn r_flashlightdepthtexture_value(console: &Console) -> Option<String> {
    Some(u8::from(console.config.video.advanced.shadow_detail == ShadowDetail::High).to_string())
}

pub(super) fn mat_setvideomode(
    console: &mut Console,
    spec: &'static Spec,
    args: &[String],
) -> Outcome {
    let usage = || spec.usage();
    let [w, h, windowed] = args else {
        return Err(usage());
    };
    let width: u32 = w.parse().map_err(|_| usage())?;
    let height: u32 = h.parse().map_err(|_| usage())?;
    if !(320..=8192).contains(&width) || !(240..=8192).contains(&height) {
        return Err(usage());
    }
    let display_mode = match windowed.as_str() {
        "1" => DisplayMode::Windowed,
        "0" => DisplayMode::Fullscreen,
        _ => return Err(usage()),
    };
    console.config.video.width = width;
    console.config.video.height = height;
    console.config.video.display_mode = display_mode;
    let _ = console.config.save();
    console.log(format!("mat_setvideomode {width} {height} {windowed}"));
    Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))))
}
