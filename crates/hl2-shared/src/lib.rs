//! What the simulation and the UI share about the local player: weapon script data, bucket
//! selection, damage and fade messages, and a read-only view of the inventory. The UI depends on
//! this crate instead of the simulation, so it builds without the physics and audio decoders.

pub mod damage;
pub mod inventory;
pub mod selection;
pub mod weapons;

/// The retail `viewmodel_fov` (the host's viewmodel camera, Source 4:3 horizontal degrees).
pub const VIEWMODEL_FOV: f32 = 54.;
