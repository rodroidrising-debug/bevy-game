//! Nearest-station highlight ring (green = free, orange = busy).

use bevy::prelude::*;

use crate::world::{InKitchen, Player, Station};

pub const HIGHLIGHT_REACH: f32 = 2.4;
pub const HIGHLIGHT_Y: f32 = 0.08;
pub const HIGHLIGHT_RADIUS: f32 = 0.9;

#[derive(Component)]
pub struct HighlightRing;

#[derive(Resource)]
pub struct HighlightAssets {
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
    pub free_mat: Handle<StandardMaterial>,
    pub busy_mat: Handle<StandardMaterial>,
}

pub fn setup_highlight(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = meshes.add(Mesh::from(Cylinder::new(HIGHLIGHT_RADIUS, 0.05)));
    let free_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.25, 1.0, 0.35, 0.45),
        emissive: bevy::color::LinearRgba::new(0.1, 0.8, 0.15, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let busy_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.55, 0.10, 0.55),
        emissive: bevy::color::LinearRgba::new(0.9, 0.4, 0.05, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    commands.insert_resource(HighlightAssets {
        mesh,
        material: free_mat.clone(),
        free_mat,
        busy_mat,
    });
}

pub fn nearest_within(player: Vec3, points: &[(Vec3, bool)]) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (i, (pos, _busy)) in points.iter().enumerate() {
        let d = player.distance(*pos);
        if d <= HIGHLIGHT_REACH && best.map_or(true, |(_, bd)| d < bd) {
            best = Some((i, d));
        }
    }
    best.map(|(i, _)| i)
}

pub fn highlight_tick(
    player_q: Query<&Transform, With<Player>>,
    stations: Query<(&Transform, &Station)>,
    mut ring_q: Query<(Entity, &mut Transform, &mut MeshMaterial3d<StandardMaterial>), With<HighlightRing>>,
    assets: Option<Res<HighlightAssets>>,
    mut commands: Commands,
    time: Res<Time>,
) {
    let Some(assets) = assets.as_ref() else {
        return;
    };
    let Ok(player_xf) = player_q.single() else {
        return;
    };
    let player_pos = player_xf.translation;
    let mut best_pos: Option<Vec3> = None;
    let mut best_busy = false;
    let mut best_d = f32::MAX;
    for (xf, station) in stations.iter() {
        let d = player_pos.distance(xf.translation);
        if d <= HIGHLIGHT_REACH && d < best_d {
            best_d = d;
            best_pos = Some(xf.translation);
            best_busy = station.processing.is_some();
        }
    }
    let pulse = 1.0 + 0.08 * (time.elapsed_secs() * 6.0).sin();
    let Some(target) = best_pos else {
        for (_e, mut xf, _mat) in ring_q.iter_mut() {
            xf.scale = Vec3::ZERO;
        }
        return;
    };
    let want_mat = if best_busy { assets.busy_mat.clone() } else { assets.free_mat.clone() };
    let ring_pos = Vec3::new(target.x, HIGHLIGHT_Y, target.z);
    if ring_q.is_empty() {
        commands.spawn((
            HighlightRing,
            InKitchen,
            Mesh3d(assets.mesh.clone()),
            MeshMaterial3d(want_mat),
            Transform { translation: ring_pos, scale: Vec3::splat(pulse), ..default() },
            Visibility::default(),
        ));
        return;
    }
    for (_e, mut xf, mut mat) in ring_q.iter_mut() {
        xf.translation = ring_pos;
        xf.scale = Vec3::splat(pulse);
        if mat.0 != want_mat {
            mat.0 = want_mat.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_nearest_within_range() {
        let p = Vec3::ZERO;
        let pts = vec![
            (Vec3::new(2.0, 0.0, 0.0), false),
            (Vec3::new(1.0, 0.0, 0.0), true),
            (Vec3::new(0.5, 0.0, 0.0), false),
        ];
        assert_eq!(nearest_within(p, &pts), Some(2));
        assert_eq!(nearest_within(p, &[]), None);
        assert_eq!(nearest_within(p, &[ (Vec3::new(5.0, 0.0, 0.0), false) ]), None);
    }
}
