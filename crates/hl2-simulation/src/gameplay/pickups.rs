use super::{Inventory, Weapon};
use std::collections::BTreeMap;

impl Inventory {
    /// Whether touching a weapon entity of `class` picks it up: a player weapon whose assets
    /// the host has loaded. One whose viewmodel is still loading stays where it lies, so the
    /// player never holds a weapon that cannot be drawn or heard (retail precaches every weapon
    /// entity's viewmodel when it is created, `CBaseCombatWeapon::Precache`).
    pub(super) fn takes_weapon(&self, class: &str, weapons: &BTreeMap<String, Weapon>) -> bool {
        weapons.contains_key(class) && !self.pickup_blocked.contains(class)
    }

    pub fn give_ammo(
        &mut self,
        ammo_type: &str,
        amount: i32,
        weapons: &BTreeMap<String, Weapon>,
    ) -> i32 {
        let Some((canonical, maximum)) = weapons.values().find_map(|w| {
            if w.ammo_type.eq_ignore_ascii_case(ammo_type) && w.ammo_max > 0 {
                Some((&w.ammo_type, w.ammo_max))
            } else if w.secondary_ammo_type.eq_ignore_ascii_case(ammo_type)
                && w.secondary_ammo_max > 0
            {
                Some((&w.secondary_ammo_type, w.secondary_ammo_max))
            } else {
                None
            }
        }) else {
            return 0;
        };
        let old = self.ammo(canonical);
        let added = amount.max(0).min((maximum - old).max(0));
        self.set_ammo(canonical, old + added);
        added
    }
}

// SDK items.h/item_ammo.cpp quantities/aliases; difficulty scaling and crates remain separate.
pub(super) fn ammo_pickup(class: &str) -> Option<(&'static str, i32)> {
    Some(match class {
        "item_ammo_pistol" | "item_box_srounds" => ("Pistol", 20),
        "item_ammo_pistol_large" | "item_large_box_srounds" => ("Pistol", 100),
        "item_ammo_smg1" | "item_box_mrounds" => ("SMG1", 45),
        "item_ammo_smg1_large" | "item_large_box_mrounds" => ("SMG1", 225),
        "item_ammo_ar2" | "item_box_lrounds" => ("AR2", 20),
        "item_ammo_ar2_large" | "item_large_box_lrounds" => ("AR2", 100),
        "item_ammo_357" => ("357", 6),
        "item_ammo_357_large" => ("357", 20),
        "item_box_buckshot" => ("Buckshot", 20),
        "item_ammo_crossbow" => ("XBowBolt", 6),
        "item_rpg_round" | "item_ml_grenade" => ("RPG_Round", 1),
        "item_ammo_smg1_grenade" | "item_ar2_grenade" => ("SMG1_Grenade", 1),
        "item_ammo_ar2_altfire" => ("AR2AltFire", 1),
        _ => return None,
    })
}
