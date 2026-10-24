//! Audio cvars (the Options Audio page): volumes, captions, speakers and sound quality.

use super::{flag, parse, Invalid};
use crate::config::{Captions, Config, SoundQuality, Speakers};
use crate::console::Console;

pub(super) fn volume_value(console: &Console) -> Option<String> {
    Some(format!("{:.2}", console.config.audio.volume))
}

pub(super) fn set_volume(config: &mut Config, arg: &str) -> Result<(), Invalid> {
    config.audio.volume = parse::<f32>(arg)?.clamp(0.0, 1.0);
    Ok(())
}

pub(super) fn snd_musicvolume_value(console: &Console) -> Option<String> {
    Some(format!("{:.2}", console.config.audio.music_volume))
}

pub(super) fn set_snd_musicvolume(config: &mut Config, arg: &str) -> Result<(), Invalid> {
    config.audio.music_volume = parse::<f32>(arg)?.clamp(0.0, 1.0);
    Ok(())
}

pub(super) fn snd_mute_losefocus_value(console: &Console) -> Option<String> {
    Some(u8::from(console.config.audio.mute_losefocus).to_string())
}

pub(super) fn set_snd_mute_losefocus(config: &mut Config, arg: &str) -> Result<(), Invalid> {
    config.audio.mute_losefocus = flag(arg)?;
    Ok(())
}

pub(super) fn closecaption_value(console: &Console) -> Option<String> {
    Some(u8::from(console.config.audio.captions != Captions::None).to_string())
}

pub(super) fn set_closecaption(config: &mut Config, arg: &str) -> Result<(), Invalid> {
    if !flag(arg)? {
        config.audio.captions = Captions::None;
    } else if config.audio.captions == Captions::None {
        config.audio.captions = Captions::ClosedCaptions;
    }
    Ok(())
}

pub(super) fn cc_subtitles_value(console: &Console) -> Option<String> {
    Some(u8::from(console.config.audio.captions == Captions::Subtitles).to_string())
}

pub(super) fn set_cc_subtitles(config: &mut Config, arg: &str) -> Result<(), Invalid> {
    if flag(arg)? {
        config.audio.captions = Captions::Subtitles;
    } else if config.audio.captions == Captions::Subtitles {
        config.audio.captions = Captions::ClosedCaptions;
    }
    Ok(())
}

pub(super) fn snd_surround_speakers_value(console: &Console) -> Option<String> {
    Some(console.config.audio.speakers.channels().to_string())
}

pub(super) fn set_snd_surround_speakers(config: &mut Config, arg: &str) -> Result<(), Invalid> {
    config.audio.speakers = Speakers::from_channels(parse(arg)?).ok_or(Invalid::Usage)?;
    Ok(())
}

pub(super) fn snd_pitchquality_value(console: &Console) -> Option<String> {
    Some(u8::from(console.config.audio.quality == SoundQuality::High).to_string())
}

pub(super) fn set_snd_pitchquality(config: &mut Config, arg: &str) -> Result<(), Invalid> {
    config.audio.quality = if flag(arg)? {
        SoundQuality::High
    } else {
        SoundQuality::Medium
    };
    Ok(())
}

pub(super) fn dsp_slow_cpu_value(console: &Console) -> Option<String> {
    Some(u8::from(console.config.audio.quality == SoundQuality::Low).to_string())
}

pub(super) fn set_dsp_slow_cpu(config: &mut Config, arg: &str) -> Result<(), Invalid> {
    config.audio.quality = if flag(arg)? {
        SoundQuality::Low
    } else {
        SoundQuality::High
    };
    Ok(())
}
