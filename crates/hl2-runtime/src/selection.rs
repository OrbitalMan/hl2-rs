//! PC bucket selection behavior, checked against the installed client and Valve's SDK.
use crate::gameplay::{Inventory, Weapon};
use std::collections::BTreeMap;

// Retail CHudWeaponSelection::OnThink: 0.5 seconds before fading, 0.75 before hiding.
pub const SELECTION_TIMEOUT: f64 = 0.5;
pub const SELECTION_FADEOUT: f64 = 0.75;
pub const WEAPON_SLOTS: usize = 6;

pub struct Selection {
    pub pending: Option<String>,
    pub until: f64,
    pub fast_switch: bool,
    pub show_empty_positions: bool,
    pub opened_at: f64,
    pub changed_at: f64,
}

impl Default for Selection {
    fn default() -> Self {
        Self {
            pending: None,
            until: 0.,
            fast_switch: false,
            show_empty_positions: true,
            opened_at: 0.,
            changed_at: 0.,
        }
    }
}

/// The caller applies `weapon` and plays each installed symbolic sound. Confirmation
/// must also consume both attack buttons until released, as Source's ProcessInput does.
#[derive(Default, Debug, PartialEq, Eq)]
pub struct SelectionResult {
    pub weapon: Option<String>,
    pub sounds: Vec<&'static str>,
}

pub fn ordered<'a>(inventory: &'a Inventory, weapons: &BTreeMap<String, Weapon>) -> Vec<&'a str> {
    let mut names = inventory
        .owned
        .keys()
        .filter(|n| weapons.get(*n).is_some_and(|w| w.slot < WEAPON_SLOTS))
        .map(String::as_str)
        .collect::<Vec<_>>();
    names.sort_by_key(|n| (weapons[*n].slot, weapons[*n].slot_position));
    names
}

pub fn can_be_selected(inv: &Inventory, weapons: &BTreeMap<String, Weapon>, name: &str) -> bool {
    let Some(w) = weapons.get(name) else {
        return false;
    };
    let Some(clip) = inv.owned.get(name) else {
        return false;
    };
    w.ammo_type.is_empty()
        || w.ammo_type.eq_ignore_ascii_case("none")
        || *clip > 0
        || inv.reserve_for(name, weapons) > 0
}

impl Selection {
    pub fn new() -> Self {
        // The retail cvar hud_showemptyweaponslots defaults to 1.
        Self::default()
    }

    pub fn tick(&mut self, inv: &Inventory, weapons: &BTreeMap<String, Weapon>, time: f64) {
        if time > self.until
            || inv.health <= 0.
            || !inv.suit
            || self
                .pending
                .as_deref()
                .is_some_and(|p| !inv.owned.contains_key(p) || !weapons.contains_key(p))
        {
            self.pending = None;
        }
    }

    fn open(&mut self, weapon: &str, time: f64) {
        if self.pending.is_none() || time > self.changed_at + SELECTION_TIMEOUT {
            self.opened_at = time;
        }
        self.pending = Some(weapon.into());
        self.changed_at = time;
        self.until = time + SELECTION_TIMEOUT + SELECTION_FADEOUT;
    }

    pub fn wheel(
        &mut self,
        inv: &Inventory,
        weapons: &BTreeMap<String, Weapon>,
        delta: i32,
        time: f64,
    ) -> SelectionResult {
        self.tick(inv, weapons, time);
        if delta == 0 || inv.health <= 0. || !inv.suit {
            return SelectionResult::default();
        }
        let names = ordered(inv, weapons)
            .into_iter()
            .filter(|name| can_be_selected(inv, weapons, name))
            .collect::<Vec<_>>();
        if names.is_empty() {
            return SelectionResult::default();
        }
        let current = self.pending.as_deref().unwrap_or(&inv.active);
        let index = names
            .iter()
            .position(|name| *name == current)
            .map(|i| (i as i64 + i64::from(delta)).rem_euclid(names.len() as i64) as usize)
            .unwrap_or_else(|| if delta > 0 { 0 } else { names.len() - 1 });
        self.open(names[index], time);
        if self.fast_switch {
            let mut result = self.confirm(inv, weapons, time);
            result.sounds.insert(0, "Player.WeaponSelectionMoveSlot");
            result
        } else {
            SelectionResult {
                sounds: vec!["Player.WeaponSelectionMoveSlot"],
                ..SelectionResult::default()
            }
        }
    }

    pub fn slot(
        &mut self,
        inv: &Inventory,
        weapons: &BTreeMap<String, Weapon>,
        slot: usize,
        time: f64,
    ) -> SelectionResult {
        self.tick(inv, weapons, time);
        if slot >= WEAPON_SLOTS || inv.health <= 0. || !inv.suit {
            return SelectionResult::default();
        }
        let names = ordered(inv, weapons)
            .into_iter()
            .filter(|n| weapons[*n].slot == slot && can_be_selected(inv, weapons, n))
            .collect::<Vec<_>>();
        if names.is_empty() {
            return SelectionResult {
                sounds: vec![if self.fast_switch {
                    "Player.DenyWeaponSelection"
                } else {
                    "Player.WeaponSelectionMoveSlot"
                }],
                ..SelectionResult::default()
            };
        }
        let current = if self.fast_switch {
            Some(inv.active.as_str())
        } else {
            self.pending.as_deref()
        };
        let next = current
            .and_then(|p| names.iter().position(|n| *n == p))
            .map_or(0, |i| (i + 1) % names.len());
        if self.fast_switch {
            self.pending = None;
            SelectionResult {
                weapon: (names[next] != inv.active).then(|| names[next].into()),
                // Source FastWeaponSwitch selects directly without the normal menu sound.
                ..SelectionResult::default()
            }
        } else {
            self.open(names[next], time);
            SelectionResult {
                sounds: vec!["Player.WeaponSelectionMoveSlot"],
                ..SelectionResult::default()
            }
        }
    }

    pub fn confirm(
        &mut self,
        inv: &Inventory,
        weapons: &BTreeMap<String, Weapon>,
        time: f64,
    ) -> SelectionResult {
        self.tick(inv, weapons, time);
        let Some(name) = self.pending.as_deref() else {
            return SelectionResult::default();
        };
        if !can_be_selected(inv, weapons, name) {
            return SelectionResult {
                sounds: vec!["Player.DenyWeaponSelection"],
                ..SelectionResult::default()
            };
        }
        SelectionResult {
            weapon: self.pending.take(),
            // cancelselect is dispatched before Player.WeaponSelected by the installed client.
            sounds: vec!["Player.WeaponSelectionClose", "Player.WeaponSelected"],
        }
    }

    pub fn cancel(&mut self) -> SelectionResult {
        SelectionResult {
            sounds: if self.pending.take().is_some() {
                vec!["Player.WeaponSelectionClose"]
            } else {
                Vec::new()
            },
            ..SelectionResult::default()
        }
    }

    pub fn last(&mut self, inv: &Inventory, weapons: &BTreeMap<String, Weapon>) -> SelectionResult {
        self.pending = None;
        SelectionResult {
            weapon: can_be_selected(inv, weapons, &inv.previous).then(|| inv.previous.clone()),
            ..SelectionResult::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
