//! Damage bits (SDK `shareddefs.h` / `hl2_shareddefs.h`) and the messages the local player's
//! client receives: ScreenFade and the CHL2_Player::UpdateClientData "Damage" message.
use glam::Vec3;
use serde::Serialize;

pub const DMG_CRUSH: u32 = 1 << 0;
pub const DMG_BULLET: u32 = 1 << 1;
pub const DMG_SLASH: u32 = 1 << 2;
pub const DMG_BURN: u32 = 1 << 3;
pub const DMG_FALL: u32 = 1 << 5;
pub const DMG_BLAST: u32 = 1 << 6;
pub const DMG_CLUB: u32 = 1 << 7;
pub const DMG_SHOCK: u32 = 1 << 8;
pub const DMG_SONIC: u32 = 1 << 9;
pub const DMG_DROWN: u32 = 1 << 14;
pub const DMG_PARALYZE: u32 = 1 << 15;
pub const DMG_NERVEGAS: u32 = 1 << 16;
pub const DMG_POISON: u32 = 1 << 17;
pub const DMG_RADIATION: u32 = 1 << 18;
pub const DMG_DROWNRECOVER: u32 = 1 << 19;
pub const DMG_ACID: u32 = 1 << 20;
pub const DMG_SLOWBURN: u32 = 1 << 21;
pub const DMG_NEVERGIB: u32 = 1 << 12;
pub const DMG_PLASMA: u32 = 1 << 24;
pub const DMG_DISSOLVE: u32 = 1 << 26;
pub const DMG_BUCKSHOT: u32 = 1 << 29;
/// hl2_shareddefs.h DMG_SNIPER (DMG_LASTGENERICFLAG << 1).
pub const DMG_SNIPER: u32 = 1 << 30;
/// CSingleplayRules::Damage_GetTimeBased.
pub const TIME_BASED: u32 = DMG_PARALYZE
    | DMG_NERVEGAS
    | DMG_POISON
    | DMG_RADIATION
    | DMG_DROWNRECOVER
    | DMG_ACID
    | DMG_SLOWBURN;
/// CSingleplayRules::Damage_GetShowOnHud.
pub const SHOW_ON_HUD: u32 = DMG_POISON
    | DMG_ACID
    | DMG_DROWN
    | DMG_BURN
    | DMG_SLOWBURN
    | DMG_NERVEGAS
    | DMG_RADIATION
    | DMG_SHOCK;

pub const FFADE_IN: u32 = 0x1;
pub const FFADE_OUT: u32 = 0x2;
pub const FFADE_MODULATE: u32 = 0x4;
pub const FFADE_STAYOUT: u32 = 0x8;
pub const FFADE_PURGE: u32 = 0x10;

/// A ScreenFade message (UTIL_ScreenFade or env_fade).
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct ScreenFade {
    pub color: [u8; 4],
    pub duration: f32,
    pub hold: f32,
    pub flags: u32,
}
impl ScreenFade {
    /// The message carries both times as unsigned 16-bit fixed point with 9 fraction
    /// bits (SCREENFADE_FRACBITS; FixedUnsigned16 truncates and clamps).
    pub fn new(color: [u8; 4], duration: f32, hold: f32, flags: u32) -> Self {
        let fixed = |seconds: f32| {
            let value = if seconds.is_finite() {
                (seconds * 512.).clamp(0., 65535.) as u16
            } else {
                0
            };
            f32::from(value) / 512.
        };
        Self {
            color,
            duration: fixed(duration),
            hold: fixed(hold),
            flags,
        }
    }
}

/// CHL2_Player::UpdateClientData "Damage" user message.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct DamageMessage {
    pub armor: u8,
    pub taken: u8,
    pub bits: u32,
    pub from: Vec3,
}
