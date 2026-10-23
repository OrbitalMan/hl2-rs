//! Mouse cvars (the Options Mouse page): sensitivity, pitch, filtering, acceleration and aim
//! assistance.

use super::{Outcome, Spec};
use crate::config::AimAssist;
use crate::console::{boolean, Console, Effect};
use crate::input::MOUSE_DEGREES_PER_COUNT;

pub(super) fn sensitivity(console: &mut Console, spec: &'static Spec, args: &[String]) -> Outcome {
    if args.is_empty() {
        console.log(format!(
            "\"sensitivity\" = \"{:.2}\"",
            console.config.mouse.sensitivity
        ));
    } else if args.len() == 1 {
        let sens: f32 = args[0].parse().map_err(|_| spec.usage())?;
        let sens = sens.clamp(0.1, 20.0);
        console.config.mouse.sensitivity = sens;
        let _ = console.config.save();
        console.log(format!("sensitivity {:.2}", sens));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn sensitivity_value(console: &Console) -> Option<String> {
    Some(format!("{:.2}", console.config.mouse.sensitivity))
}

pub(super) fn m_pitch(console: &mut Console, spec: &'static Spec, args: &[String]) -> Outcome {
    let current_pitch = if console.config.mouse.invert {
        -MOUSE_DEGREES_PER_COUNT
    } else {
        MOUSE_DEGREES_PER_COUNT
    };
    if args.is_empty() {
        console.log(format!("\"m_pitch\" = \"{:.3}\"", current_pitch));
    } else if args.len() == 1 {
        let pitch: f32 = args[0].parse().map_err(|_| spec.usage())?;
        console.config.mouse.invert = pitch < 0.0;
        let _ = console.config.save();
        let new_pitch = if console.config.mouse.invert {
            -MOUSE_DEGREES_PER_COUNT
        } else {
            MOUSE_DEGREES_PER_COUNT
        };
        console.log(format!("m_pitch {:.3}", new_pitch));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn m_pitch_value(console: &Console) -> Option<String> {
    Some(format!(
        "{:.3}",
        if console.config.mouse.invert {
            -MOUSE_DEGREES_PER_COUNT
        } else {
            MOUSE_DEGREES_PER_COUNT
        }
    ))
}

pub(super) fn m_filter(console: &mut Console, spec: &'static Spec, args: &[String]) -> Outcome {
    if args.is_empty() {
        console.log(format!(
            "\"m_filter\" = \"{}\"",
            u8::from(console.config.mouse.filter)
        ));
    } else if args.len() == 1 {
        let val = boolean(&args[0]).ok_or_else(|| spec.usage())?;
        console.config.mouse.filter = val;
        let _ = console.config.save();
        console.log(format!("m_filter {}", u8::from(val)));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn m_filter_value(console: &Console) -> Option<String> {
    Some(u8::from(console.config.mouse.filter).to_string())
}

pub(super) fn m_rawinput(console: &mut Console, spec: &'static Spec, args: &[String]) -> Outcome {
    if args.is_empty() {
        console.log(format!(
            "\"m_rawinput\" = \"{}\"",
            u8::from(console.config.mouse.raw_input)
        ));
    } else if args.len() == 1 {
        let val = boolean(&args[0]).ok_or_else(|| spec.usage())?;
        console.config.mouse.raw_input = val;
        let _ = console.config.save();
        console.log(format!("m_rawinput {}", u8::from(val)));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn m_rawinput_value(console: &Console) -> Option<String> {
    Some(u8::from(console.config.mouse.raw_input).to_string())
}

pub(super) fn m_customaccel(
    console: &mut Console,
    spec: &'static Spec,
    args: &[String],
) -> Outcome {
    if args.is_empty() {
        console.log(format!(
            "\"m_customaccel\" = \"{}\"",
            u8::from(console.config.mouse.acceleration)
        ));
    } else if args.len() == 1 {
        let val = match args[0].as_str() {
            "0" => false,
            "1" | "3" => true,
            _ => return Err(spec.usage()),
        };
        console.config.mouse.acceleration = val;
        let _ = console.config.save();
        console.log(format!("m_customaccel {}", u8::from(val)));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn m_customaccel_value(console: &Console) -> Option<String> {
    Some(u8::from(console.config.mouse.acceleration).to_string())
}

pub(super) fn m_customaccel_exponent(
    console: &mut Console,
    spec: &'static Spec,
    args: &[String],
) -> Outcome {
    if args.is_empty() {
        console.log(format!(
            "\"m_customaccel_exponent\" = \"{:.2}\"",
            console.config.mouse.accel_exponent
        ));
    } else if args.len() == 1 {
        let exp: f32 = args[0].parse().map_err(|_| spec.usage())?;
        let exp = exp.clamp(1.0, 1.4);
        console.config.mouse.accel_exponent = exp;
        let _ = console.config.save();
        console.log(format!("m_customaccel_exponent {:.2}", exp));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn m_customaccel_exponent_value(console: &Console) -> Option<String> {
    Some(format!("{:.2}", console.config.mouse.accel_exponent))
}

/// `sk_autoaim_mode` and its alias `m_autoaim_mode`.
pub(super) fn autoaim_mode(console: &mut Console, spec: &'static Spec, args: &[String]) -> Outcome {
    let usage = || spec.usage();
    let name = spec.name;
    let current_mode = match console.config.mouse.aim_assist {
        AimAssist::Off => 0,
        AimAssist::Basic => 1,
        AimAssist::Enhanced => 2,
    };
    if args.is_empty() {
        console.log(format!("\"{name}\" = \"{}\"", current_mode));
    } else if args.len() == 1 {
        let mode: u8 = args[0].parse().map_err(|_| usage())?;
        console.config.mouse.aim_assist = match mode {
            0 => AimAssist::Off,
            1 => AimAssist::Basic,
            2 => AimAssist::Enhanced,
            _ => return Err(usage()),
        };
        let _ = console.config.save();
        console.log(format!("{name} {mode}"));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(usage());
    }
    Ok(None)
}

pub(super) fn autoaim_mode_value(console: &Console) -> Option<String> {
    Some(
        match console.config.mouse.aim_assist {
            AimAssist::Off => "0",
            AimAssist::Basic => "1",
            AimAssist::Enhanced => "2",
        }
        .to_string(),
    )
}

/// `sk_autoaim_scale1` and its alias `m_autoaim_scale`.
pub(super) fn autoaim_scale(
    console: &mut Console,
    spec: &'static Spec,
    args: &[String],
) -> Outcome {
    let name = spec.name;
    if args.is_empty() {
        console.log(format!(
            "\"{name}\" = \"{:.2}\"",
            console.config.mouse.aim_assist_scale
        ));
    } else if args.len() == 1 {
        let scale: f32 = args[0].parse().map_err(|_| spec.usage())?;
        let scale = scale.clamp(0.5, 2.0);
        console.config.mouse.aim_assist_scale = scale;
        let _ = console.config.save();
        console.log(format!("{name} {:.2}", scale));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn autoaim_scale_value(console: &Console) -> Option<String> {
    Some(format!("{:.2}", console.config.mouse.aim_assist_scale))
}
