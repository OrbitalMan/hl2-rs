//! Bevy camera presentation for the already-simulated player view.

use bevy::prelude::*;

/// Publishes the character's current eye position and field of view after the
/// fixed simulation loop. Character motion itself remains in `movement`.
pub(crate) fn present_player_view(
    sim: Res<crate::movement::Simulation>,
    mut cameras: Query<(&mut Transform, &mut Projection), With<crate::FlyCamera>>,
) {
    if let Ok((mut camera, mut projection)) = cameras.single_mut() {
        *camera = Transform::from_translation(crate::source_to_bevy(sim.eye())).looking_to(
            crate::source_to_bevy(crate::source_direction(sim.yaw, sim.pitch)),
            Vec3::Y,
        );
        let fov = crate::video::vertical_fov(sim.fov);
        if let Projection::Perspective(perspective) = &mut *projection
            && perspective.fov != fov
        {
            perspective.fov = fov;
        }
    }
}
