//! Share one native ALSA binding package between quad-snd and Bevy/CPAL.
//! Used only by the retained backend on Linux; Windows playback is unaffected.
pub use alsa_sys::*;
