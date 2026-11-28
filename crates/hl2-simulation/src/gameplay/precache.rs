//! Selective map-load weapon selection, projectile/audio dependencies, and switch-free grants.
use super::{Inventory, Weapon};
use modkit_core::World;
use std::collections::{BTreeMap, BTreeSet};

/// Player weapons referenced by a map: placed `weapon_*` entities, `game_player_equip`, and
/// `extra` (carried inventory on transitions or test scripts). NPC `additionalequipment` and
/// `GiveWeapon` outputs stay NPC-exclusive (world models loaded via `npc_weapons::required_npc_weapons`).
/// Unknown names are ignored so custom or incomplete entries stay harmless.
pub fn required_weapons(
    world: &World,
    weapons: &BTreeMap<String, Weapon>,
    extra: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut required = BTreeSet::new();
    for class in extra {
        if weapons.contains_key(class) {
            required.insert(class.clone());
        }
    }
    for entity in &world.entities {
        let class = entity.class();
        if weapons.contains_key(class) {
            required.insert(class.to_owned());
        }
        if class.eq_ignore_ascii_case("game_player_equip") {
            for (key, value) in &entity.properties {
                let lower = key.to_ascii_lowercase();
                if weapons.contains_key(&lower) && value.trim() != "0" {
                    required.insert(lower);
                }
            }
        }
    }
    required
}

/// Projectile models uniquely owned by a player weapon class (only needed when that weapon is).
pub fn projectile_models(class: &str) -> &'static [&'static str] {
    match class {
        "weapon_smg1" => &[crate::projectiles::GRENADE_MODEL],
        "weapon_frag" => &[crate::projectiles::FRAG_MODEL],
        "weapon_crossbow" => &[crate::weapon_crossbow::BOLT_MODEL],
        "weapon_rpg" => &[
            crate::weapon_rpg::MISSILE_LAUNCH_MODEL,
            crate::weapon_rpg::MISSILE_MODEL,
        ],
        "weapon_bugbait" => &[crate::weapon_bugbait::BAIT_MODEL],
        _ => &[],
    }
}

/// Extra fixed soundscript cues tied to a specific player weapon class.
pub fn extra_sound_cues(class: &str) -> &'static [&'static str] {
    match class {
        "weapon_crossbow" => &[
            "Weapon_Crossbow.BoltHitWorld",
            "Weapon_Crossbow.BoltHitBody",
            "Weapon_Crossbow.BoltSkewer",
        ],
        "weapon_rpg" => &["Missile.Ignite"],
        "weapon_physcannon" => &[crate::weapon_physcannon::MOTOR_SOUND],
        "weapon_bugbait" => &["Weapon_Bugbait.Splat", "GrenadeBugBait.Splat"],
        "weapon_frag" => &["Grenade.Blip", "Grenade.ImpactHard"],
        _ => &[],
    }
}

impl Inventory {
    /// Insert `class` into `owned` and award default reserve ammo when absent. Returns `true`
    /// if the weapon definition existed.
    pub(super) fn insert_owned(&mut self, class: &str, weapons: &BTreeMap<String, Weapon>) -> bool {
        let Some(w) = weapons.get(class) else {
            return false;
        };
        if w.magazine < 0 && !self.owned.contains_key(class) && w.ammo_max > 0 {
            // CBaseCombatWeapon::GiveDefaultAmmo: a clipless weapon's default clip
            // is reserve ammunition.
            self.owned.insert(class.into(), -1);
            let ammo = w.ammo_type.clone();
            self.give_ammo(&ammo, w.default_clip.max(0), weapons);
        }
        if !self.owned.contains_key(class) {
            // CBaseCombatCharacter::Weapon_Equip: a default clip larger than the clip
            // fills the clip and gives the rest as reserve (crossbow 5 = 1 + 4).
            let clip = if w.magazine > 0 && w.default_clip > w.magazine {
                let ammo = w.ammo_type.clone();
                self.give_ammo(&ammo, w.default_clip - w.magazine, weapons);
                w.magazine
            } else {
                w.default_clip
            };
            self.owned.insert(class.into(), clip);
        }
        true
    }

    /// Grant a weapon into inventory without deploying or switching away from the active weapon.
    pub fn grant_without_switch(&mut self, class: &str, weapons: &BTreeMap<String, Weapon>) {
        self.insert_owned(class, weapons);
    }
}
