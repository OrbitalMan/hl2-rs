//! Audio cvars (the Options Audio page): volumes, captions, speakers and sound quality.

use super::{Outcome, Spec};
use crate::config::{Captions, SoundQuality, Speakers};
use crate::console::{boolean, Console, Effect};

pub(super) fn volume(console: &mut Console, spec: &'static Spec, args: &[String]) -> Outcome {
    if args.is_empty() {
        console.log(format!(
            "\"volume\" = \"{:.2}\"",
            console.config.audio.volume
        ));
    } else if args.len() == 1 {
        let vol: f32 = args[0].parse().map_err(|_| spec.usage())?;
        let vol = vol.clamp(0.0, 1.0);
        console.config.audio.volume = vol;
        let _ = console.config.save();
        console.log(format!("volume {:.2}", vol));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn volume_value(console: &Console) -> Option<String> {
    Some(format!("{:.2}", console.config.audio.volume))
}

pub(super) fn snd_musicvolume(
    console: &mut Console,
    spec: &'static Spec,
    args: &[String],
) -> Outcome {
    if args.is_empty() {
        console.log(format!(
            "\"snd_musicvolume\" = \"{:.2}\"",
            console.config.audio.music_volume
        ));
    } else if args.len() == 1 {
        let vol: f32 = args[0].parse().map_err(|_| spec.usage())?;
        let vol = vol.clamp(0.0, 1.0);
        console.config.audio.music_volume = vol;
        let _ = console.config.save();
        console.log(format!("snd_musicvolume {:.2}", vol));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn snd_musicvolume_value(console: &Console) -> Option<String> {
    Some(format!("{:.2}", console.config.audio.music_volume))
}

pub(super) fn snd_mute_losefocus(
    console: &mut Console,
    spec: &'static Spec,
    args: &[String],
) -> Outcome {
    if args.is_empty() {
        console.log(format!(
            "\"snd_mute_losefocus\" = \"{}\"",
            u8::from(console.config.audio.mute_losefocus)
        ));
    } else if args.len() == 1 {
        let val = boolean(&args[0]).ok_or_else(|| spec.usage())?;
        console.config.audio.mute_losefocus = val;
        let _ = console.config.save();
        console.log(format!("snd_mute_losefocus {}", u8::from(val)));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn snd_mute_losefocus_value(console: &Console) -> Option<String> {
    Some(u8::from(console.config.audio.mute_losefocus).to_string())
}

pub(super) fn closecaption(console: &mut Console, spec: &'static Spec, args: &[String]) -> Outcome {
    let is_on = console.config.audio.captions != Captions::None;
    if args.is_empty() {
        console.log(format!("\"closecaption\" = \"{}\"", u8::from(is_on)));
    } else if args.len() == 1 {
        let val = boolean(&args[0]).ok_or_else(|| spec.usage())?;
        if !val {
            console.config.audio.captions = Captions::None;
        } else if console.config.audio.captions == Captions::None {
            console.config.audio.captions = Captions::ClosedCaptions;
        }
        let _ = console.config.save();
        let new_on = console.config.audio.captions != Captions::None;
        console.log(format!("closecaption {}", u8::from(new_on)));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn closecaption_value(console: &Console) -> Option<String> {
    Some(u8::from(console.config.audio.captions != Captions::None).to_string())
}

pub(super) fn cc_subtitles(console: &mut Console, spec: &'static Spec, args: &[String]) -> Outcome {
    let is_subs = console.config.audio.captions == Captions::Subtitles;
    if args.is_empty() {
        console.log(format!("\"cc_subtitles\" = \"{}\"", u8::from(is_subs)));
    } else if args.len() == 1 {
        let val = boolean(&args[0]).ok_or_else(|| spec.usage())?;
        if val {
            console.config.audio.captions = Captions::Subtitles;
        } else if console.config.audio.captions == Captions::Subtitles {
            console.config.audio.captions = Captions::ClosedCaptions;
        }
        let _ = console.config.save();
        let new_subs = console.config.audio.captions == Captions::Subtitles;
        console.log(format!("cc_subtitles {}", u8::from(new_subs)));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn cc_subtitles_value(console: &Console) -> Option<String> {
    Some(u8::from(console.config.audio.captions == Captions::Subtitles).to_string())
}

pub(super) fn snd_surround_speakers(
    console: &mut Console,
    spec: &'static Spec,
    args: &[String],
) -> Outcome {
    let usage = || spec.usage();
    if args.is_empty() {
        console.log(format!(
            "\"snd_surround_speakers\" = \"{}\"",
            console.config.audio.speakers.channels()
        ));
    } else if args.len() == 1 {
        let val: u8 = args[0].parse().map_err(|_| usage())?;
        let speakers = Speakers::from_channels(val).ok_or_else(usage)?;
        console.config.audio.speakers = speakers;
        let _ = console.config.save();
        console.log(format!("snd_surround_speakers {}", val));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(usage());
    }
    Ok(None)
}

pub(super) fn snd_surround_speakers_value(console: &Console) -> Option<String> {
    Some(console.config.audio.speakers.channels().to_string())
}

pub(super) fn snd_pitchquality(
    console: &mut Console,
    spec: &'static Spec,
    args: &[String],
) -> Outcome {
    let is_high = console.config.audio.quality == SoundQuality::High;
    if args.is_empty() {
        console.log(format!("\"snd_pitchquality\" = \"{}\"", u8::from(is_high)));
    } else if args.len() == 1 {
        let val = boolean(&args[0]).ok_or_else(|| spec.usage())?;
        console.config.audio.quality = if val {
            SoundQuality::High
        } else {
            SoundQuality::Medium
        };
        let _ = console.config.save();
        console.log(format!("snd_pitchquality {}", u8::from(val)));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn snd_pitchquality_value(console: &Console) -> Option<String> {
    Some(u8::from(console.config.audio.quality == SoundQuality::High).to_string())
}

pub(super) fn dsp_slow_cpu(console: &mut Console, spec: &'static Spec, args: &[String]) -> Outcome {
    let is_low = console.config.audio.quality == SoundQuality::Low;
    if args.is_empty() {
        console.log(format!("\"dsp_slow_cpu\" = \"{}\"", u8::from(is_low)));
    } else if args.len() == 1 {
        let val = boolean(&args[0]).ok_or_else(|| spec.usage())?;
        console.config.audio.quality = if val {
            SoundQuality::Low
        } else {
            SoundQuality::High
        };
        let _ = console.config.save();
        console.log(format!("dsp_slow_cpu {}", u8::from(val)));
        return Ok(Some(Effect::ApplyConfig(Box::new(console.config.clone()))));
    } else {
        return Err(spec.usage());
    }
    Ok(None)
}

pub(super) fn dsp_slow_cpu_value(console: &Console) -> Option<String> {
    Some(u8::from(console.config.audio.quality == SoundQuality::Low).to_string())
}
