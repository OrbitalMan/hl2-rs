//! Engine-independent collision and rigid-body simulation shared by HL2 hosts.
//!
//! The Rapier adapter preserves the retained runtime's behavior and limitations;
//! extracting it does not establish equivalence with Valve's VPhysics solver.

pub mod npc_probe;
pub mod physics;
