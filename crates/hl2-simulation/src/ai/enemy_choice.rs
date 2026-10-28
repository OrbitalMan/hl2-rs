//! CAI_BaseNPC::ChooseEnemy / ShouldChooseNewEnemy / BestEnemy (ai_basenpc.cpp): every
//! sensing think picks the best remembered enemy again, so an NPC changes targets when a
//! closer or better-seen one appears instead of keeping its first enemy until it dies.
//! Entities are scene indices (the player included); SDK behavior.
use super::memory::{Enemies, INVALID_TIME};
use super::relationships::Disposition;

/// The enemy-selection timers and serial an NPC keeps between thinks
/// (m_EnemiesSerialNumber, m_FailChooseEnemyTimer).
#[derive(Clone, Copy, Debug)]
pub struct ChooseState {
    /// m_EnemiesSerialNumber (-1 at spawn, ForceChooseNewEnemy).
    pub serial: i64,
    /// CSimpleSimTimer m_FailChooseEnemyTimer (m_next -1: expired until set).
    pub fail_timer: f64,
}
impl Default for ChooseState {
    fn default() -> Self {
        Self {
            serial: -1,
            fail_timer: -1.,
        }
    }
}

/// What the NPC thinks of a remembered entity: None when it cannot be an enemy at all
/// (dead, FL_NOTARGET, IsValidEnemy false), else IRelationType and IRelationPriority.
pub type Relation = Option<(Disposition, i32)>;

fn hostile(d: Disposition) -> bool {
    matches!(d, Disposition::HT | Disposition::FR)
}

/// ShouldChooseNewEnemy. `sees_hostile` is COND_SEE_HATE/DISLIKE/NEMESIS/FEAR. Like the
/// SDK, a serial that changed while the NPC has an enemy is only stored again once it has
/// none, so after any new memory the NPC re-runs BestEnemy on every think.
pub fn should_choose_new_enemy(
    enemy: Option<usize>,
    enemies: &Enemies,
    state: &mut ChooseState,
    relation: Relation,
    sees_hostile: bool,
    task_failed: bool,
    now: f64,
) -> bool {
    if let Some(enemy) = enemy {
        if enemies.serial() as i64 != state.serial {
            return true;
        }
        state.serial = enemies.serial() as i64;
        let info = enemies.find(Some(enemy));
        if info.is_some_and(|m| m.eluded) || !relation.is_some_and(|(d, _)| hostile(d)) {
            return true;
        }
        if sees_hostile {
            return true;
        }
        if now >= state.fail_timer {
            state.fail_timer = now + 1.5;
            if task_failed
                || info.is_some_and(|m| {
                    m.time_at_first_hand == INVALID_TIME || now - m.time_last_seen > 10.
                })
            {
                return true;
            }
        }
        if info.is_some_and(|m| m.time_valid < now) {
            return false;
        }
    }
    state.serial = enemies.serial() as i64;
    true
}

/// TRS_NONE / TRS_FALSE / TRS_TRUE.
type Tri = Option<bool>;

/// BestEnemy: reachable before unreachable (IsUnreachable, whatever the priority), then
/// the highest priority, then the closer one, with
/// the SDK's seen (DidSeeEntity: sighted this think) / visible (within look distance
/// with FVisible) tie-breaks. EnemyDistTolerance is 0 for every HL2 class.
pub fn best_enemy(
    enemies: &Enemies,
    now: f64,
    mut relation: impl FnMut(usize) -> Relation,
    unreachable: impl Fn(usize) -> bool,
    distance_sq: impl Fn(usize) -> f32,
    did_see: impl Fn(usize) -> bool,
    mut visible: impl FnMut(usize) -> bool,
) -> Option<usize> {
    let mut best: Option<usize> = None;
    let mut best_dist_sq = f32::MAX;
    let mut best_priority = -1000;
    let mut best_seen: Tri = None;
    let mut best_visible: Tri = None;
    // bBestUnreachable starts true (forces the first check).
    let mut best_unreachable = true;
    for (&key, memory) in &enemies.memories {
        let Some(enemy) = key else {
            continue;
        };
        let Some((disposition, priority)) = relation(enemy) else {
            continue;
        };
        if !hostile(disposition)
            || memory.time_valid > now
            || memory.eluded
            || disposition == Disposition::FR
                && !memory.unforgettable
                && memory.time_first_seen == INVALID_TIME
        {
            continue;
        }
        let is_unreachable = unreachable(enemy);
        if !best_unreachable && is_unreachable {
            continue;
        }
        let Some(current_best) = best else {
            best_priority = priority;
            best_dist_sq = distance_sq(enemy);
            best = Some(enemy);
            best_unreachable = is_unreachable;
            best_seen = None;
            best_visible = None;
            continue;
        };
        if best_unreachable && !is_unreachable || priority > best_priority {
            best_priority = priority;
            best_dist_sq = distance_sq(enemy);
            best = Some(enemy);
            best_unreachable = is_unreachable;
            best_seen = None;
            best_visible = None;
            continue;
        }
        if priority < best_priority {
            continue;
        }
        let dist_sq = distance_sq(enemy);
        let closer = best_dist_sq - dist_sq > 0.;
        let mut accept = false;
        let mut cur_seen: Tri = None;
        let mut cur_visible: Tri = None;
        if !closer && (best_seen == Some(true) || best_visible == Some(true)) {
            continue;
        }
        if closer && best_seen == Some(false) && best_visible == Some(false) {
            accept = true;
        }
        if !accept && closer {
            cur_seen = Some(did_see(enemy));
            accept = cur_seen == Some(true);
        }
        if !accept {
            if !closer {
                let seen = *best_seen.get_or_insert_with(|| did_see(current_best));
                if seen {
                    continue;
                }
            }
            if closer && best_visible.is_none() {
                cur_visible = Some(visible(enemy));
                accept = cur_visible == Some(true);
            }
            if !accept {
                let b_seen = *best_seen.get_or_insert_with(|| did_see(current_best));
                let b_visible = *best_visible.get_or_insert_with(|| visible(current_best));
                let c_seen = *cur_seen.get_or_insert_with(|| did_see(enemy));
                let c_visible = *cur_visible.get_or_insert_with(|| visible(enemy));
                let best_known = b_seen || b_visible;
                let cur_known = c_seen || c_visible;
                if !closer && (best_known || !cur_known) || closer && !cur_known && best_known {
                    continue;
                }
            }
        }
        best_seen = cur_seen;
        best_visible = cur_visible;
        best_dist_sq = dist_sq;
        best_priority = priority;
        best = Some(enemy);
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    fn memory_with(entries: &[(usize, f64)]) -> Enemies {
        let mut e = Enemies::default();
        for &(id, seen) in entries {
            e.update(Some(id), Vec3::ZERO, 0., true, seen);
        }
        e
    }

    #[test]
    fn reachable_enemies_come_before_unreachable_ones_whatever_the_priority() {
        let e = memory_with(&[(1, 10.), (2, 10.), (3, 10.)]);
        let dist = |id: usize| (id as f32 * 100.).powi(2);
        // Enemy 1 is the closest and enemy 3 has the highest priority, both unreachable:
        // the reachable enemy 2 wins.
        let relation = |id: usize| Some((Disposition::HT, if id == 3 { 5 } else { 0 }));
        let unreachable = |id: usize| id != 2;
        let pick = best_enemy(&e, 10., relation, unreachable, dist, |_| true, |_| true);
        assert_eq!(pick, Some(2));
        // All unreachable: priority, then distance, as before.
        let pick = best_enemy(&e, 10., relation, |_| true, dist, |_| true, |_| true);
        assert_eq!(pick, Some(3));
    }

    #[test]
    fn a_closer_seen_enemy_displaces_the_first_one() {
        // The zombie report: enemy 1 far away, the player (99) right in front and seen.
        let e = memory_with(&[(1, 10.), (99, 10.)]);
        let dist = |id: usize| if id == 99 { 40. * 40. } else { 900. * 900. };
        let pick = best_enemy(
            &e,
            10.,
            |_| Some((Disposition::HT, 0)),
            |_| false,
            dist,
            |id| id == 99,
            |_| true,
        );
        assert_eq!(pick, Some(99));
        // A closer enemy that is neither seen nor visible loses to a seen farther one.
        let pick = best_enemy(
            &e,
            10.,
            |_| Some((Disposition::HT, 0)),
            |_| false,
            dist,
            |id| id == 1,
            |id| id == 1,
        );
        assert_eq!(pick, Some(1));
    }

    #[test]
    fn priority_dead_and_eluded_enemies() {
        let mut e = memory_with(&[(1, 10.), (2, 10.), (3, 10.)]);
        let dist = |id: usize| id as f32 * 100.;
        // Higher priority wins over distance.
        let pick = best_enemy(
            &e,
            10.,
            |id| Some((Disposition::HT, if id == 3 { 5 } else { 0 })),
            |_| false,
            dist,
            |_| true,
            |_| true,
        );
        assert_eq!(pick, Some(3));
        // Dead / notarget (no relation) and eluded memories are skipped.
        e.mark_eluded(Some(2));
        let pick = best_enemy(
            &e,
            10.,
            |id| (id != 1).then_some((Disposition::HT, 0)),
            |_| false,
            dist,
            |_| true,
            |_| true,
        );
        assert_eq!(pick, Some(3));
        // Liked or neutral entities never become enemies.
        let pick = best_enemy(
            &e,
            10.,
            |_| Some((Disposition::LI, 0)),
            |_| false,
            dist,
            |_| true,
            |_| true,
        );
        assert_eq!(pick, None);
    }

    #[test]
    fn should_choose_follows_the_sdk_gates() {
        let mut e = memory_with(&[(1, 10.)]);
        let mut state = ChooseState::default();
        let hate = Some((Disposition::HT, 0));
        // No enemy: always choose (and remember the serial).
        assert!(should_choose_new_enemy(
            None, &e, &mut state, None, false, false, 10.
        ));
        // Seeing a hated entity: choose again.
        assert!(should_choose_new_enemy(
            Some(1),
            &e,
            &mut state,
            hate,
            true,
            false,
            10.
        ));
        // Nothing new (the first call arms the 1.5 s fail timer): keep the enemy.
        assert!(!should_choose_new_enemy(
            Some(1),
            &e,
            &mut state,
            hate,
            false,
            false,
            10.
        ));
        assert!(!should_choose_new_enemy(
            Some(1),
            &e,
            &mut state,
            hate,
            false,
            false,
            10.5
        ));
        // Not seen for 10 s when the fail timer expires.
        assert!(!should_choose_new_enemy(
            Some(1),
            &e,
            &mut state,
            hate,
            false,
            false,
            11.
        ));
        assert!(should_choose_new_enemy(
            Some(1),
            &e,
            &mut state,
            hate,
            false,
            false,
            21.
        ));
        // No longer hated.
        assert!(should_choose_new_enemy(
            Some(1),
            &e,
            &mut state,
            None,
            false,
            false,
            21.
        ));
        // A new memory changes the serial: true on every think while there is an enemy.
        e.update(Some(2), Vec3::ZERO, 0., false, 22.);
        assert!(should_choose_new_enemy(
            Some(1),
            &e,
            &mut state,
            hate,
            false,
            false,
            22.
        ));
        assert!(should_choose_new_enemy(
            Some(1),
            &e,
            &mut state,
            hate,
            false,
            false,
            22.1
        ));
        // Until it has none (enemy 1 seen again meanwhile).
        e.update(Some(1), Vec3::ZERO, 0., true, 23.);
        assert!(should_choose_new_enemy(
            None, &e, &mut state, None, false, false, 23.
        ));
        assert!(!should_choose_new_enemy(
            Some(1),
            &e,
            &mut state,
            hate,
            false,
            false,
            23.1
        ));
    }
}
