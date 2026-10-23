//! PC bucket selection (shared with the HUD).
pub use hl2_shared::selection::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::{Inventory, Weapon};
    use std::collections::BTreeMap;
    fn setup() -> (Inventory, BTreeMap<String, Weapon>) {
        let mk = |slot, pos, ammo: &str| Weapon {
            slot,
            slot_position: pos,
            magazine: if ammo == "None" { -1 } else { 18 },
            ammo_type: ammo.into(),
            ..Weapon::default()
        };
        let defs = BTreeMap::from([
            ("weapon_crowbar".into(), mk(0, 0, "None")),
            ("weapon_pistol".into(), mk(1, 0, "Pistol")),
            ("weapon_357".into(), mk(1, 1, "357")),
        ]);
        let mut inv = Inventory::default();
        inv.suit = true;
        inv.active = "weapon_crowbar".into();
        inv.owned = BTreeMap::from([
            ("weapon_crowbar".into(), -1),
            ("weapon_pistol".into(), 18),
            ("weapon_357".into(), 6),
        ]);
        (inv, defs)
    }
    #[test]
    fn wheel_wraps_in_slot_order_and_slot_key_cycles_same_bucket() {
        let (inv, defs) = setup();
        let mut selection = Selection::new();
        selection.wheel(&inv, &defs, -1, 0.);
        assert_eq!(selection.pending.as_deref(), Some("weapon_357"));
        selection.wheel(&inv, &defs, 1, 0.);
        assert_eq!(selection.pending.as_deref(), Some("weapon_crowbar"));
        selection.slot(&inv, &defs, 1, 0.);
        assert_eq!(selection.pending.as_deref(), Some("weapon_pistol"));
        selection.slot(&inv, &defs, 1, 0.);
        assert_eq!(selection.pending.as_deref(), Some("weapon_357"));
    }
    #[test]
    fn exhausted_guns_remain_in_display_but_are_skipped_in_selection() {
        let (mut inv, defs) = setup();
        inv.owned.insert("weapon_pistol".into(), 0);
        assert_eq!(ordered(&inv, &defs).len(), 3);
        let mut selection = Selection::new();
        selection.wheel(&inv, &defs, 1, 0.);
        assert_eq!(selection.pending.as_deref(), Some("weapon_357"));
        inv.pistol_ammo = 1;
        selection.cancel();
        selection.wheel(&inv, &defs, 1, 0.);
        assert_eq!(selection.pending.as_deref(), Some("weapon_pistol"));
    }
    #[test]
    fn secondary_reserve_keeps_an_owned_empty_primary_weapon_selectable() {
        let (mut inv, mut defs) = setup();
        defs.insert(
            "weapon_smg1".into(),
            Weapon {
                slot: 2,
                magazine: 45,
                ammo_type: "SMG1".into(),
                secondary_ammo_type: "SMG1_Grenade".into(),
                ..Weapon::default()
            },
        );
        inv.owned.insert("weapon_smg1".into(), 0);
        assert!(!can_be_selected(&inv, &defs, "weapon_smg1"));
        inv.reserve_ammo.insert("SMG1_Grenade".into(), 1);
        assert!(can_be_selected(&inv, &defs, "weapon_smg1"));
        let mut selection = Selection::new();
        selection.slot(&inv, &defs, 2, 0.);
        assert_eq!(
            selection.confirm(&inv, &defs, 0.1).weapon.as_deref(),
            Some("weapon_smg1")
        );
        inv.owned.remove("weapon_smg1");
        assert!(!can_be_selected(&inv, &defs, "weapon_smg1"));
    }
    #[test]
    fn timeout_cancel_and_confirmation_do_not_switch_inventory_by_themselves() {
        let (inv, defs) = setup();
        let mut selection = Selection::new();
        selection.slot(&inv, &defs, 1, 10.);
        selection.tick(&inv, &defs, 11.25);
        assert!(selection.pending.is_some());
        selection.tick(&inv, &defs, 11.250_01);
        assert!(selection.pending.is_none());
        selection.slot(&inv, &defs, 1, 12.);
        let result = selection.confirm(&inv, &defs, 12.1);
        assert_eq!(result.weapon.as_deref(), Some("weapon_pistol"));
        assert!(selection.pending.is_none());
        assert_eq!(inv.active, "weapon_crowbar");
        selection.slot(&inv, &defs, 1, 13.);
        assert_eq!(selection.cancel().sounds, ["Player.WeaponSelectionClose"]);
        assert_eq!(inv.active, "weapon_crowbar");
    }
    #[test]
    fn fast_switch_uses_active_bucket_position_and_returns_an_immediate_switch() {
        let (mut inv, defs) = setup();
        inv.active = "weapon_pistol".into();
        let mut selection = Selection::new();
        selection.fast_switch = true;
        let result = selection.slot(&inv, &defs, 1, 0.);
        assert_eq!(result.weapon.as_deref(), Some("weapon_357"));
        assert!(selection.pending.is_none());
        let result = selection.wheel(&inv, &defs, -1, 1.);
        assert_eq!(result.weapon.as_deref(), Some("weapon_crowbar"));
        assert!(selection.pending.is_none());
    }
    #[test]
    fn unarmed_scroll_starts_at_endpoints_and_expiry_restarts_from_active() {
        let (mut inv, defs) = setup();
        inv.active.clear();
        let mut selection = Selection::new();
        selection.wheel(&inv, &defs, 1, 0.);
        assert_eq!(selection.pending.as_deref(), Some("weapon_crowbar"));
        selection.cancel();
        selection.wheel(&inv, &defs, -1, 0.);
        assert_eq!(selection.pending.as_deref(), Some("weapon_357"));
        inv.active = "weapon_crowbar".into();
        selection.wheel(&inv, &defs, 1, 2.);
        assert_eq!(selection.pending.as_deref(), Some("weapon_pistol"));
    }
}
