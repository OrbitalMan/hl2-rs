//! Timed explicit NPC interests refreshed by authored scene LOOKAT events.
//! Random/synthetic queues, head poses, PVS gating and native AI scheduling are unfinished.
use glam::Vec3;
use serde::Serialize;
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Target {
    Player,
    Entity(usize),
}
#[derive(Clone, Debug, Serialize)]
pub struct Interest {
    pub target: Target,
    pub importance: f32,
    pub start: f64,
    pub end: f64,
    pub scene: usize,
    pub event: usize,
}
#[derive(Default)]
pub struct LookTargets {
    actors: BTreeMap<usize, Vec<Interest>>,
}
impl LookTargets {
    pub fn refresh(
        &mut self,
        actor: usize,
        target: Target,
        mut importance: f32,
        now: f64,
        scene: usize,
        event: usize,
    ) {
        let queue = self.actors.entry(actor).or_default();
        if let Some(index) = queue.iter().position(|i| i.target == target) {
            let previous = queue.remove(index);
            if previous.start == now {
                importance = importance.max(previous.importance);
            }
        }
        queue.push(Interest {
            target,
            importance,
            start: now,
            end: now + 0.1,
            scene,
            event,
        });
    }
    pub fn cleanup(&mut self, now: f64, alive: impl Fn(usize) -> bool) {
        self.actors.retain(|actor, queue| {
            queue.retain(|i| {
                i.end >= now
                    && match i.target {
                        Target::Player => true,
                        Target::Entity(id) => alive(id),
                    }
            });
            alive(*actor) && !queue.is_empty()
        });
    }
    pub fn report(&self) -> &BTreeMap<usize, Vec<Interest>> {
        &self.actors
    }
    /// Eyes choose newest valid explicit interest, independent of head importance.
    /// Explicit scene targets are not subject to random-interest visibility/distance filters.
    pub fn eye_target(
        &self,
        actor: usize,
        eye: Vec3,
        forward: Vec3,
        position: impl Fn(Target) -> Option<Vec3>,
    ) -> Option<(&Interest, Vec3)> {
        self.actors.get(&actor)?.iter().rev().find_map(|interest| {
            if interest.target == Target::Entity(actor) {
                return Some((interest, eye + forward * 100.));
            }
            let target = position(interest.target)?;
            let delta = target - eye;
            (delta.length() >= 1. && delta.normalize().dot(forward) > 0.259)
                .then_some((interest, target))
        })
    }
}
pub fn scene_importance(intensity: f32, elapsed: f32) -> f32 {
    let t = (elapsed / 0.3).clamp(0., 1.);
    intensity.clamp(0., 3. * t * t - 2. * t * t * t)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refresh_deduplicates_and_eyes_select_tail_even_at_zero_importance() {
        let mut targets = LookTargets::default();
        targets.refresh(1, Target::Player, 0.8, 1., 10, 0);
        targets.refresh(1, Target::Entity(2), 0., 1., 10, 1);
        assert_eq!(
            targets
                .eye_target(1, Vec3::ZERO, Vec3::X, |_| Some(Vec3::X * 4.))
                .unwrap()
                .0
                .target,
            Target::Entity(2)
        );
        targets.refresh(1, Target::Player, 0.2, 1., 11, 0);
        assert_eq!(targets.report()[&1].len(), 2);
        assert_eq!(targets.report()[&1][1].importance, 0.8);
        targets.refresh(1, Target::Player, 0.1, 1.015, 11, 0);
        assert_eq!(targets.report()[&1][1].importance, 0.1);
        targets.cleanup(1.12, |_| true);
        assert!(targets.report().is_empty());
    }
    #[test]
    fn self_looks_forward_and_dead_or_behind_targets_are_rejected() {
        let mut targets = LookTargets::default();
        targets.refresh(1, Target::Entity(1), 1., 0., 3, 0);
        assert_eq!(
            targets
                .eye_target(1, Vec3::ZERO, Vec3::X, |_| None)
                .unwrap()
                .1,
            Vec3::X * 100.
        );
        targets.refresh(1, Target::Entity(2), 1., 0., 3, 1);
        assert_eq!(
            targets
                .eye_target(1, Vec3::ZERO, Vec3::X, |_| Some(-Vec3::X))
                .unwrap()
                .0
                .target,
            Target::Entity(1)
        );
        targets.cleanup(0., |id| id != 2);
        assert_eq!(targets.report()[&1].len(), 1);
        targets.cleanup(0., |_| false);
        assert!(targets.report().is_empty());
        assert!((scene_importance(1., 0.15) - 0.5).abs() < 1e-6);
        assert_eq!(scene_importance(0.2, 0.15), 0.2);
    }
}
