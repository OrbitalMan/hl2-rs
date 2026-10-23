//! A read-only view of the local player's inventory: what the HUD and bucket selection read.
use crate::weapons::Weapon;
use std::collections::BTreeMap;

/// The player's weapons, ammunition, health and suit, as the HUD and selection read them.
pub trait InventoryView {
    /// The active weapon's class (empty when unarmed).
    fn active(&self) -> &str;
    /// The previously active weapon's class (`lastinv`).
    fn previous(&self) -> &str;
    fn health(&self) -> f32;
    fn armor(&self) -> f32;
    fn suit(&self) -> bool;
    /// Owned weapon classes and their clip contents (-1 for clipless weapons).
    fn owned(&self) -> &BTreeMap<String, i32>;
    /// Reserve ammunition of `ammo_type`.
    fn ammo(&self, ammo_type: &str) -> i32;

    /// Reserve ammunition of `class`'s primary ammo type.
    fn reserve_for(&self, class: &str, weapons: &BTreeMap<String, Weapon>) -> i32 {
        weapons
            .get(class)
            .map(|w| self.ammo(&w.ammo_type))
            .unwrap_or(0)
    }
    /// Reserve ammunition of `class`'s secondary ammo type.
    fn secondary_for(&self, class: &str, weapons: &BTreeMap<String, Weapon>) -> i32 {
        weapons
            .get(class)
            .map(|w| self.ammo(&w.secondary_ammo_type))
            .unwrap_or(0)
    }
}
