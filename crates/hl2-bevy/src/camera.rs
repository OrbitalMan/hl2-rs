//! Bevy camera presentation for the already-simulated player view.

use bevy::prelude::*;

/// The camera that draws the viewmodel layer (its FOV follows the view's FOV offset).
#[derive(Component)]
pub struct ViewmodelCamera;

/// CBasePlayer::CalcView adds the punch angle (degrees; Source pitch is down-positive, ours
/// up-positive) to the player's view angles (radians). Aim and the viewmodel do not include it.
pub fn punched(yaw: f32, pitch: f32, punch: glam::Vec3) -> (f32, f32) {
    (yaw + punch.y.to_radians(), pitch - punch.x.to_radians())
}

/// CViewRender::SetUpView: the viewmodel's FOV is `viewmodel_fov - (default_fov - fov)`. At or
/// below zero the view has moved past the viewmodel, which is then not drawn: returns the
/// vertical FOV and near plane for the viewmodel camera given its far plane.
pub fn viewmodel_projection(viewmodel_degrees: f32, far: f32) -> (f32, f32) {
    if viewmodel_degrees > 0. {
        (crate::video::vertical_fov(viewmodel_degrees), 0.1)
    } else {
        (crate::video::vertical_fov(1.), far * 0.5)
    }
}

/// Publishes the view after the fixed simulation loop: a point_viewcontrol camera or env_zoom
/// when active, otherwise the player's eye with the weapon punch and SetFOV zoom. Character
/// motion itself remains in `simulation`.
pub(crate) fn present_player_view(
    sim: Res<crate::simulation::Simulation>,
    game: Option<Res<crate::gameplay::Gameplay>>,
    mut cameras: Query<(&mut Transform, &mut Projection), With<crate::FlyCamera>>,
    mut viewmodel: Query<&mut Projection, (With<ViewmodelCamera>, Without<crate::FlyCamera>)>,
) {
    let game = game.as_deref();
    // The player's own FOV, including SetFOV ramps (crossbow zoom).
    let player_fov = game.map_or(sim.fov, |game| game.inventory.view_fov(game.scene.time));
    if let (Some(game), Ok(mut projection)) = (game, viewmodel.single_mut()) {
        let degrees = hl2_simulation::viewmodel_effects::viewmodel_fov(
            game.inventory.fov.default_fov,
            player_fov,
        );
        if let Projection::Perspective(p) = &mut *projection {
            let (fov, near) = viewmodel_projection(degrees, p.far);
            if p.fov != fov || p.near != near {
                p.fov = fov;
                p.near = near;
            }
        }
    }
    if let Ok((mut camera, mut projection)) = cameras.single_mut() {
        let (yaw, pitch) = sim.view_angles();
        let (yaw, pitch) = match game.filter(|_| sim.player_view()) {
            Some(game) => punched(yaw, pitch, game.inventory.punch.angle),
            None => (yaw, pitch),
        };
        *camera = Transform::from_translation(crate::source_to_bevy(sim.view_origin())).looking_to(
            crate::source_to_bevy(crate::source_direction(yaw, pitch)),
            Vec3::Y,
        );
        let fov = crate::video::vertical_fov(sim.view_fov(player_fov));
        if let Projection::Perspective(perspective) = &mut *projection
            && perspective.fov != fov
        {
            perspective.fov = fov;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn punch_tilts_the_view_in_source_conventions() {
        let (yaw, pitch) = punched(0.5, 0.1, glam::Vec3::new(2., 3., 0.));
        assert!((yaw - (0.5 + 3f32.to_radians())).abs() < 1e-6);
        assert!(
            (pitch - (0.1 - 2f32.to_radians())).abs() < 1e-6,
            "down-positive pitch"
        );
        assert_eq!(punched(0.5, 0.1, glam::Vec3::ZERO), (0.5, 0.1));
    }

    #[test]
    fn viewmodel_is_hidden_past_its_fov() {
        let (fov, near) = viewmodel_projection(54., 1000.);
        assert_eq!((fov, near), (crate::video::vertical_fov(54.), 0.1));
        let (_, near) = viewmodel_projection(0., 1000.);
        assert_eq!(near, 500., "near plane beyond any viewmodel");
    }
}
