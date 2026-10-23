//! Weapon script data: the implemented weapons' installed `scripts/weapon_*.txt`, `cfg/skill.cfg`
//! damage values and `resource/hl2_english.txt` names.
use anyhow::{Context, Result};
use source_assets::{keyvalues, vpk::Vfs};
use std::collections::BTreeMap;

#[derive(Clone, Default)]
pub struct Weapon {
    /// Entity class (weapon_*), the script file name.
    pub class: String,
    pub name: String,
    pub slot: usize,
    pub slot_position: usize,
    pub icon: String,
    pub viewmodel: String,
    pub magazine: i32,
    pub default_clip: i32,
    pub ammo_type: String,
    pub ammo_max: i32,
    pub secondary_ammo_type: String,
    pub secondary_ammo_max: i32,
    pub secondary_damage: f32,
    pub secondary_radius: f32,
    pub secondary_mass: f32,
    pub secondary_lifetime: f32,
    pub pellets: usize,
    pub damage: f32,
    pub sounds: BTreeMap<String, String>,
    /// Script "weight" (GetNextBestWeapon prefers the highest).
    pub weight: i32,
    /// Script "autoswitchto" / "autoswitchfrom" set to 0.
    pub no_autoswitch_to: bool,
    pub no_autoswitch_from: bool,
}
const IMPLEMENTED: [&str; 11] = [
    "weapon_crowbar",
    "weapon_pistol",
    "weapon_357",
    "weapon_smg1",
    "weapon_ar2",
    "weapon_shotgun",
    "weapon_frag",
    "weapon_crossbow",
    "weapon_rpg",
    "weapon_bugbait",
    "weapon_physcannon",
];
pub fn definitions(vfs: &Vfs) -> Result<BTreeMap<String, Weapon>> {
    let skill = vfs.read("cfg/skill.cfg")?.context("skill.cfg missing")?;
    let t = keyvalues::tokens(&String::from_utf8_lossy(&skill))?;
    let number = |name: &str| {
        t.windows(2)
            .find(|p| p[0].eq_ignore_ascii_case(name))
            .and_then(|p| p[1].parse::<f32>().ok())
            .filter(|value| value.is_finite())
    };
    let localization = localization(vfs)?;
    let mut weapons = BTreeMap::new();
    for class in IMPLEMENTED {
        let data = vfs
            .read(&format!("scripts/{class}.txt"))?
            .context("weapon script missing")?;
        let script = keyvalues::parse(&String::from_utf8_lossy(&data))?;
        let root = script.first().context("empty weapon script")?;
        let value = |key| root.get(key).and_then(|e| e.text());
        let sounds = root
            .get("SoundData")
            .map(|e| {
                e.children()
                    .iter()
                    .filter_map(|entry| entry.text().map(|sound| (entry.key.clone(), sound.into())))
                    .collect()
            })
            .unwrap_or_default();
        let magazine: i32 = value("clip_size").context("clip size absent")?.parse()?;
        let ammo_type = value("primary_ammo").unwrap_or("None");
        let secondary_ammo_type = value("secondary_ammo").unwrap_or("None");
        // AR2AltFire's registered carry cvar has an underscore absent from its ammo name.
        let secondary_skill_name = if secondary_ammo_type.eq_ignore_ascii_case("AR2AltFire") {
            "ar2_altfire".into()
        } else {
            secondary_ammo_type.to_ascii_lowercase()
        };
        // Buckshot is the ammunition/damage name; sk_plr_dmg_shotgun does not exist.
        // CAmmoDef (hl2_gamerules.cpp GetAmmoDef): XBowBolt uses the sk_*_crossbow cvars.
        let skill_name = if class == "weapon_crowbar" {
            "crowbar".into()
        } else if ammo_type.eq_ignore_ascii_case("XBowBolt") {
            "crossbow".into()
        } else {
            ammo_type.to_ascii_lowercase()
        };
        let printname = value("printname").unwrap_or(class);
        let name = localization
            .get(&printname.trim_start_matches('#').to_lowercase())
            .cloned()
            .unwrap_or_else(|| printname.trim_start_matches('#').into());
        weapons.insert(
            class.into(),
            Weapon {
                class: class.into(),
                name,
                slot: value("bucket").and_then(|s| s.parse().ok()).unwrap_or(0),
                slot_position: value("bucket_position")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                icon: root
                    .get("TextureData")
                    .and_then(|e| e.get("weapon"))
                    .and_then(|e| e.get("character"))
                    .and_then(|e| e.text())
                    .unwrap_or_default()
                    .into(),
                viewmodel: value("viewmodel").context("viewmodel absent")?.into(),
                weight: value("weight").and_then(|s| s.parse().ok()).unwrap_or(0),
                no_autoswitch_to: value("autoswitchto").is_some_and(|s| s.trim() == "0"),
                no_autoswitch_from: value("autoswitchfrom").is_some_and(|s| s.trim() == "0"),
                magazine,
                default_clip: value("default_clip")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(magazine),
                ammo_type: ammo_type.into(),
                ammo_max: number(&format!("sk_max_{skill_name}"))
                    .unwrap_or(0.)
                    .max(0.) as i32,
                secondary_ammo_type: secondary_ammo_type.into(),
                secondary_ammo_max: number(&format!("sk_max_{secondary_skill_name}"))
                    .unwrap_or(0.)
                    .max(0.) as i32,
                secondary_damage: number(&format!("sk_plr_dmg_{secondary_skill_name}"))
                    .unwrap_or(0.)
                    .max(0.),
                secondary_radius: if class == "weapon_smg1" {
                    number("sk_smg1_grenade_radius").unwrap_or(0.).max(0.)
                } else if class == "weapon_frag" {
                    number("sk_fraggrenade_radius").unwrap_or(0.).max(0.)
                } else if class == "weapon_ar2" {
                    number("sk_weapon_ar2_alt_fire_radius")
                        .unwrap_or(10.)
                        .clamp(1., 12.)
                } else {
                    0.
                },
                secondary_mass: number("sk_weapon_ar2_alt_fire_mass")
                    .unwrap_or(150.)
                    .max(0.),
                secondary_lifetime: number("sk_weapon_ar2_alt_fire_duration")
                    .unwrap_or(2.)
                    .max(0.),
                pellets: if class == "weapon_shotgun" {
                    // Published CHalfLife2 cvar default; installed skill.cfg may override.
                    number("sk_plr_num_shotgun_pellets")
                        .unwrap_or(7.)
                        .clamp(1., 64.) as usize
                } else {
                    1
                },
                // CGrenadeFrag::Spawn: a player's grenade uses sk_plr_dmg_fraggrenade.
                damage: if class == "weapon_frag" {
                    number("sk_plr_dmg_fraggrenade").unwrap_or(0.)
                } else {
                    number(&format!("sk_plr_dmg_{skill_name}")).unwrap_or(0.)
                },
                sounds,
            },
        );
    }
    Ok(weapons)
}
fn localization(vfs: &Vfs) -> Result<BTreeMap<String, String>> {
    let Some(bytes) = vfs.read("resource/hl2_english.txt")? else {
        return Ok(BTreeMap::new());
    };
    let entries = keyvalues::parse_resource(&bytes)?;
    Ok(entries
        .first()
        .and_then(|e| e.get("Tokens"))
        .map(|e| {
            e.children()
                .iter()
                .filter_map(|e| e.text().map(|v| (e.key.to_lowercase(), v.into())))
                .collect()
        })
        .unwrap_or_default())
}
