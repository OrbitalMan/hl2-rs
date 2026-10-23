//! PC bucket selection behavior, checked against the installed client and Valve's SDK.
use crate::inventory::InventoryView;
use crate::weapons::Weapon;
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

pub fn ordered<'a>(
    inventory: &'a dyn InventoryView,
    weapons: &BTreeMap<String, Weapon>,
) -> Vec<&'a str> {
    let mut names = inventory
        .owned()
        .keys()
        .filter(|n| weapons.get(*n).is_some_and(|w| w.slot < WEAPON_SLOTS))
        .map(String::as_str)
        .collect::<Vec<_>>();
    names.sort_by_key(|n| (weapons[*n].slot, weapons[*n].slot_position));
    names
}

pub fn can_be_selected(
    inv: &dyn InventoryView,
    weapons: &BTreeMap<String, Weapon>,
    name: &str,
) -> bool {
    let Some(w) = weapons.get(name) else {
        return false;
    };
    let Some(clip) = inv.owned().get(name) else {
        return false;
    };
    w.ammo_type.is_empty()
        || w.ammo_type.eq_ignore_ascii_case("none")
        || *clip > 0
        || inv.reserve_for(name, weapons) > 0
        || inv.secondary_for(name, weapons) > 0
}

impl Selection {
    pub fn new() -> Self {
        // The retail cvar hud_showemptyweaponslots defaults to 1.
        Self::default()
    }

    pub fn tick(&mut self, inv: &dyn InventoryView, weapons: &BTreeMap<String, Weapon>, time: f64) {
        if time > self.until
            || inv.health() <= 0.
            || !inv.suit()
            || self
                .pending
                .as_deref()
                .is_some_and(|p| !inv.owned().contains_key(p) || !weapons.contains_key(p))
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
        inv: &dyn InventoryView,
        weapons: &BTreeMap<String, Weapon>,
        delta: i32,
        time: f64,
    ) -> SelectionResult {
        self.tick(inv, weapons, time);
        if delta == 0 || inv.health() <= 0. || !inv.suit() {
            return SelectionResult::default();
        }
        let names = ordered(inv, weapons)
            .into_iter()
            .filter(|name| can_be_selected(inv, weapons, name))
            .collect::<Vec<_>>();
        if names.is_empty() {
            return SelectionResult::default();
        }
        let current = self.pending.as_deref().unwrap_or(inv.active());
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
        inv: &dyn InventoryView,
        weapons: &BTreeMap<String, Weapon>,
        slot: usize,
        time: f64,
    ) -> SelectionResult {
        self.tick(inv, weapons, time);
        if slot >= WEAPON_SLOTS || inv.health() <= 0. || !inv.suit() {
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
            Some(inv.active())
        } else {
            self.pending.as_deref()
        };
        let next = current
            .and_then(|p| names.iter().position(|n| *n == p))
            .map_or(0, |i| (i + 1) % names.len());
        if self.fast_switch {
            self.pending = None;
            SelectionResult {
                weapon: (names[next] != inv.active()).then(|| names[next].into()),
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
        inv: &dyn InventoryView,
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

    pub fn last(
        &mut self,
        inv: &dyn InventoryView,
        weapons: &BTreeMap<String, Weapon>,
    ) -> SelectionResult {
        self.pending = None;
        SelectionResult {
            weapon: can_be_selected(inv, weapons, inv.previous())
                .then(|| inv.previous().to_owned()),
            ..SelectionResult::default()
        }
    }
}
