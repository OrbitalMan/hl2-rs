//! Which player weapons the player can pick up, given the assets the host has loaded (O50).
//! A map loads only the viewmodels it needs; one that appears later (an NPC's drop of a
//! weapon the map's NPCs do not carry, `ent_create`) is loaded when it appears, and until it
//! is in, touching it leaves it lying (`Inventory::pickup_blocked`), so the player never holds
//! a weapon that cannot be drawn or heard.
use super::Gameplay;
use crate::runtime_weapons::PendingWeaponGrant;

impl Gameplay {
    /// Blocks pickups of every player weapon not loaded yet; called whenever the inventory or
    /// the map's weapon set is replaced.
    pub(crate) fn sync_pickup_blocked(&mut self) {
        self.inventory.pickup_blocked = self
            .weapons
            .keys()
            .filter(|class| !self.precached_weapons.contains(*class))
            .cloned()
            .collect();
    }
    /// A weapon's assets are in: it is drawn and heard from now on and can be picked up.
    pub(crate) fn mark_weapon_loaded(&mut self, class: &str) {
        self.precached_weapons.insert(class.to_owned());
        self.inventory.pickup_blocked.remove(class);
    }
    /// A weapon entity of `class` appeared during play: load its assets now, ahead of its
    /// pickup, without giving it to the player.
    pub(crate) fn request_weapon_assets(&mut self, class: &str) {
        if !self.weapons.contains_key(class)
            || self.precached_weapons.contains(class)
            || self.weapon_load_failed.contains(class)
            || self.weapon_requests.iter().any(|r| r.weapon_id == class)
        {
            return;
        }
        self.weapon_requests.push(PendingWeaponGrant {
            weapon_id: class.to_owned(),
            give: false,
            equip: false,
            refill_ammo: false,
            priority: true,
            attachments: Vec::new(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::Gameplay;

    fn weapon(class: &str) -> hl2_simulation::gameplay::Weapon {
        hl2_simulation::gameplay::Weapon {
            class: class.into(),
            ..Default::default()
        }
    }

    #[test]
    fn unloaded_weapons_cannot_be_picked_up_until_their_assets_are_in() {
        let mut game = Gameplay::synthetic(modkit_core::World::default());
        for class in ["weapon_pistol", "weapon_smg1"] {
            game.weapons.insert(class.into(), weapon(class));
        }
        game.precached_weapons.insert("weapon_pistol".into());
        game.sync_pickup_blocked();
        assert_eq!(
            game.inventory.pickup_blocked,
            ["weapon_smg1".to_owned()].into()
        );
        game.request_weapon_assets("weapon_smg1");
        game.request_weapon_assets("weapon_smg1");
        game.request_weapon_assets("weapon_pistol");
        game.request_weapon_assets("weapon_stunstick");
        let requests: Vec<_> = game
            .weapon_requests
            .iter()
            .map(|r| (r.weapon_id.as_str(), r.give, r.priority))
            .collect();
        assert_eq!(
            requests,
            [("weapon_smg1", false, true)],
            "once, loaded only"
        );
        game.mark_weapon_loaded("weapon_smg1");
        assert!(game.inventory.pickup_blocked.is_empty());
        assert!(game.precached_weapons.contains("weapon_smg1"));
        game.weapon_requests.clear();
        game.weapon_load_failed.insert("weapon_ar2".into());
        game.weapons
            .insert("weapon_ar2".into(), weapon("weapon_ar2"));
        game.request_weapon_assets("weapon_ar2");
        assert!(
            game.weapon_requests.is_empty(),
            "a failed load is not retried"
        );
    }
}
