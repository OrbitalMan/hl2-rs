//! CAI_BaseNPC::UpdateEfficiency (ai_basenpc.cpp): how often an NPC runs its decisions
//! (0.1 s normal up to 0.6 s super efficient) from its state, whether it is in the
//! player's PVS, whether the player faces it and its distance. Used here for the idle
//! sound roll, which GatherConditions makes once per decision and only with COND_IN_PVS
//! (or in combat). PC defaults: ai_default_efficient 0, so the floor is AIE_NORMAL.
//! Not modeled: the frame-time limit, danger sounds, recent attacks/damage forcing
//! normal, the visibility PVS and COND_SEE_PLAYER caps.
use super::state::NpcState;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Efficiency {
    Normal,
    Efficient,
    VeryEfficient,
    SuperEfficient,
}
impl Efficiency {
    /// g_DecisionIntervals.
    pub fn decision_interval(self) -> f64 {
        match self {
            Self::Normal => 0.1,
            Self::Efficient => 0.2,
            Self::VeryEfficient => 0.4,
            Self::SuperEfficient => 0.6,
        }
    }
}
/// The efficiency mapping for idle, alert (script counts as alert) and combat NPCs;
/// other states are normal. `distance` is from the player's eye to the NPC's origin.
pub fn efficiency(state: NpcState, in_pvs: bool, player_facing: bool, distance: f32) -> Efficiency {
    use Efficiency::*;
    let row = match state {
        NpcState::Idle => 0,
        NpcState::Alert | NpcState::Script => 1,
        NpcState::Combat => 2,
        _ => return Normal,
    };
    if in_pvs && distance < 15. * 12. {
        return Normal;
    }
    let range = if in_pvs {
        if distance < 50. * 12. {
            0
        } else if distance < 200. * 12. {
            1
        } else {
            2
        }
    } else if distance < 25. * 12. {
        0
    } else if distance < 100. * 12. {
        1
    } else {
        2
    };
    const MAPPINGS: [[[Efficiency; 3]; 3]; 3] = [
        // Idle: in PVS facing, in PVS not facing, not in PVS.
        [
            [Normal, Efficient, Efficient],
            [Efficient, Efficient, VeryEfficient],
            [VeryEfficient, SuperEfficient, SuperEfficient],
        ],
        // Alert.
        [
            [Normal, Efficient, Efficient],
            [Normal, Efficient, Efficient],
            [Efficient, VeryEfficient, SuperEfficient],
        ],
        // Combat.
        [
            [Normal, Normal, Efficient],
            [Normal, Efficient, Efficient],
            [Normal, Efficient, VeryEfficient],
        ],
    ];
    let column = if !in_pvs {
        2
    } else if player_facing {
        0
    } else {
        1
    };
    MAPPINGS[row][column][range]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn far_unseen_idle_npcs_decide_rarely() {
        assert_eq!(
            efficiency(NpcState::Idle, true, true, 100.),
            Efficiency::Normal
        );
        assert_eq!(
            efficiency(NpcState::Idle, true, true, 400.),
            Efficiency::Normal
        );
        assert_eq!(
            efficiency(NpcState::Idle, true, true, 1000.),
            Efficiency::Efficient
        );
        assert_eq!(
            efficiency(NpcState::Idle, true, false, 400.),
            Efficiency::Efficient
        );
        assert_eq!(
            efficiency(NpcState::Idle, false, false, 2000.),
            Efficiency::SuperEfficient
        );
        assert_eq!(
            efficiency(NpcState::Combat, false, false, 200.),
            Efficiency::Normal
        );
        assert_eq!(
            efficiency(NpcState::Dead, false, false, 5000.),
            Efficiency::Normal
        );
        assert_eq!(Efficiency::SuperEfficient.decision_interval(), 0.6);
    }
}
