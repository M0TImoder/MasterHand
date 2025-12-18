use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use std::net::UdpSocket;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Debug)]
struct Landmark {
    id: usize,
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Deserialize, Debug)]
struct OneHand {
    label: String,
    landmarks: Vec<Landmark>,
    #[serde(default)]
    gesture: String, 
}

#[derive(Deserialize, Debug)]
struct HandPacket {
    hands: Vec<OneHand>,
    #[serde(default)] 
    snap: bool,
}

#[derive(Component, PartialEq, Eq, Clone, Copy, Debug, Hash)]
enum HandSide {
    Left,
    Right,
}

#[derive(Component)]
struct HandPoint {
    id: usize,
    side: HandSide,
}

#[derive(Component)]
struct SpawnedBox; 

#[derive(Resource)]
struct UdpConnection(UdpSocket);

#[derive(Resource)]
struct HandMaterials {
    right: Handle<StandardMaterial>,
    left: Handle<StandardMaterial>,
}

#[derive(Resource, Default)]
struct HandPresence {
    last_seen_right: f32,
    last_seen_left: f32,
}

const FADE_TIMEOUT: f32 = 0.5;

fn main() {
    let socket = UdpSocket::bind("127.0.0.1:5005").expect("Bind failed");
    socket.set_nonblocking(true).expect("Nonblocking failed");

    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
        .insert_resource(UdpConnection(socket))
        .insert_resource(HandPresence::default())
        .add_systems(Startup, setup)
        .add_systems(Update, update_hands_and_physics)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut gizmo_config: ResMut<GizmoConfigStore>,
) {
    let (config, _) = gizmo_config.config_mut::<DefaultGizmoConfigGroup>();
    config.depth_bias = -1.0;
    config.line_width = 3.0;

    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(0.0, 10.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });

    commands.spawn(PointLightBundle {
        point_light: PointLight {
            intensity: 2000.0,
            shadows_enabled: true,
            range: 50.0,
            ..default()
        },
        transform: Transform::from_xyz(0.0, 15.0, 5.0),
        ..default()
    });

    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Plane3d::default().mesh().size(30.0, 30.0)),
            material: materials.add(StandardMaterial {
                base_color: Color::srgb(0.2, 0.2, 0.2),
                perceptual_roughness: 0.8,
                ..default()
            }),
            transform: Transform::from_xyz(0.0, -5.0, 0.0),
            ..default()
        },
        RigidBody::Fixed,
        Collider::cuboid(15.0, 0.01, 15.0), 
    ));

    let right_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.0, 0.8, 1.0, 1.0),
        emissive: LinearRgba::new(0.0, 0.8, 1.0, 1.0),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    
    let left_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.0, 0.8, 1.0),
        emissive: LinearRgba::new(1.0, 0.0, 0.8, 1.0),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });

    commands.insert_resource(HandMaterials {
        right: right_mat.clone(),
        left: left_mat.clone(),
    });

    let sphere_mesh = meshes.add(Sphere::new(0.08));

    let sides = [
        (HandSide::Right, right_mat),
        (HandSide::Left, left_mat)
    ];

    for (side, material) in sides {
        for i in 0..21 {
            commands.spawn((
                PbrBundle {
                    mesh: sphere_mesh.clone(),
                    material: material.clone(),
                    transform: Transform::from_xyz(0.0, -100.0, 0.0),
                    ..default()
                },
                HandPoint { id: i, side: side },
                RigidBody::KinematicPositionBased,
                Collider::ball(0.1),
                Friction::coefficient(2.0),
            ));
        }
    }
}

const HAND_CONNECTIONS: &[(usize, usize)] = &[
    (0, 1), (1, 2), (2, 3), (3, 4),
    (0, 5), (5, 6), (6, 7), (7, 8),
    (9, 10), (10, 11), (11, 12),
    (13, 14), (14, 15), (15, 16),
    (0, 17), (17, 18), (18, 19), (19, 20),
    (5, 9), (9, 13), (13, 17)
];

fn update_hands_and_physics(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    hand_mats: Res<HandMaterials>,
    mut hand_presence: ResMut<HandPresence>,
    socket_res: Res<UdpConnection>,
    mut hand_query: Query<(&HandPoint, &mut Transform)>,
    mut box_query: Query<(&mut ExternalForce, &Transform), (With<SpawnedBox>, Without<HandPoint>)>,
    mut gizmos: Gizmos,
    time: Res<Time>, 
) {
    let mut buf = [0; 65536];
    let mut latest_packet: Option<HandPacket> = None;
    let current_time = time.elapsed_seconds();

    while let Ok((amt, _src)) = socket_res.0.recv_from(&mut buf) {
        let valid_data = &buf[..amt];
        if let Ok(packet) = serde_json::from_slice::<HandPacket>(valid_data) {
            latest_packet = Some(packet);
        }
    }

    if let Some(packet) = &latest_packet {
        if packet.hands.iter().any(|h| h.label == "Right") {
            hand_presence.last_seen_right = current_time;
        }
        if packet.hands.iter().any(|h| h.label == "Left") {
            hand_presence.last_seen_left = current_time;
        }

        if packet.snap {
            let rand_x = (time.elapsed_seconds() * 10.0).sin() * 5.0;
            let box_size = 5.0;
            commands.spawn((
                PbrBundle {
                    mesh: meshes.add(Cuboid::new(box_size, box_size, box_size)),
                    material: materials.add(Color::srgb(1.0, 0.5, 0.0)),
                    transform: Transform::from_xyz(rand_x, 15.0, 0.0),
                    ..default()
                },
                RigidBody::Dynamic,
                Collider::cuboid(box_size / 2.0, box_size / 2.0, box_size / 2.0),
                Restitution::coefficient(0.1),
                Friction::coefficient(1.0),
                ColliderMassProperties::Density(5.0),
                ExternalForce::default(), 
                SpawnedBox, 
            ));
        }

        let mut hand_centers: HashMap<String, Vec3> = HashMap::new();
        let mut hand_normals: HashMap<String, Vec3> = HashMap::new();
        let mut hand_gestures: HashMap<String, String> = HashMap::new();

        for (point, mut transform) in hand_query.iter_mut() {
            let target_hand_data = packet.hands.iter().find(|h| {
                match point.side {
                    HandSide::Right => h.label == "Right",
                    HandSide::Left => h.label == "Left",
                }
            });

            if let Some(hand_data) = target_hand_data {
                if point.id == 9 { 
                    hand_gestures.insert(hand_data.label.clone(), hand_data.gesture.clone());
                }

                let mut depth_offset = 0.0;
                let wrist = hand_data.landmarks.iter().find(|l| l.id == 0);
                let middle_mcp = hand_data.landmarks.iter().find(|l| l.id == 9);
                let pinky_mcp = hand_data.landmarks.iter().find(|l| l.id == 17);
                let index_mcp = hand_data.landmarks.iter().find(|l| l.id == 5);

                if let (Some(w), Some(m)) = (wrist, middle_mcp) {
                    let dx = w.x - m.x;
                    let dy = w.y - m.y;
                    let hand_size = (dx * dx + dy * dy).sqrt();
                    depth_offset = 20.0 - (hand_size * 80.0);
                }

                if let Some(lm) = hand_data.landmarks.iter().find(|l| l.id == point.id) {
                    let scale = 20.0;
                    let x = (lm.x - 0.5) * scale; 
                    let y = (0.5 - lm.y) * scale + 3.0; 
                    let z = depth_offset + (lm.z * scale);

                    let target_pos = Vec3::new(x, y, z);
                    
                    let smooth_factor = 40.0 * time.delta_seconds(); 
                    let t = smooth_factor.clamp(0.0, 1.0);
                    transform.translation = transform.translation.lerp(target_pos, t);

                    if point.id == 9 {
                        hand_centers.insert(hand_data.label.clone(), transform.translation);
                    }
                }

                if point.id == 0 {
                    if let (Some(w), Some(i), Some(p)) = (wrist, index_mcp, pinky_mcp) {
                         let to_index = Vec3::new(i.x - w.x, w.y - i.y, i.z - w.z);
                         let to_pinky = Vec3::new(p.x - w.x, w.y - p.y, p.z - w.z);
                         
                         let mut normal = if hand_data.label == "Right" {
                             to_index.cross(to_pinky).normalize_or_zero()
                         } else {
                             to_pinky.cross(to_index).normalize_or_zero()
                         };
                         
                         normal.y *= -1.0; 
                         hand_normals.insert(hand_data.label.clone(), normal);
                    }
                }
            }
        }

        let mut total_force_field = HashMap::new(); 

        for (label, gesture) in &hand_gestures {
            if gesture == "Fist" {
                if let Some(center) = hand_centers.get(label) {
                    for (_box_force, box_transform) in box_query.iter() {
                        let dir = *center - box_transform.translation;
                        let dist_sq = dir.length_squared().max(1.0);
                        let force_mag = 50000.0 / dist_sq; 
                        let force = dir.normalize_or_zero() * force_mag;
                        
                        let entity_ptr = box_transform as *const _ as usize; 
                        total_force_field.entry(entity_ptr).and_modify(|f: &mut Vec3| *f += force).or_insert(force);
                    }
                    gizmos.sphere(*center, Quat::IDENTITY, 1.0, Color::srgb(1.0, 0.0, 0.0));
                }
            }
        }

        let right_open = hand_gestures.get("Right").map(|g| g == "Open").unwrap_or(false);
        let left_open = hand_gestures.get("Left").map(|g| g == "Open").unwrap_or(false);

        if right_open && left_open {
             if let (Some(n_r), Some(n_l)) = (hand_normals.get("Right"), hand_normals.get("Left")) {
                 if n_r.dot(*n_l) > 0.5 { 
                     let avg_dir = (*n_r + *n_l).normalize();
                     let wind_force = avg_dir * 1500.0; 

                     for (_box_force, box_transform) in box_query.iter() {
                         let entity_ptr = box_transform as *const _ as usize;
                         total_force_field.entry(entity_ptr).and_modify(|f: &mut Vec3| *f += wind_force).or_insert(wind_force);
                     }
                     
                     if let Some(center) = hand_centers.get("Right") {
                         gizmos.arrow(*center, *center + avg_dir * 5.0, Color::srgb(0.0, 1.0, 0.0));
                     }
                 }
             }
        }

        for (mut box_force, box_transform) in box_query.iter_mut() {
            let entity_ptr = box_transform as *const _ as usize;
            if let Some(force) = total_force_field.get(&entity_ptr) {
                box_force.force = *force;
            } else {
                box_force.force = Vec3::ZERO;
            }
        }
    }

    let show_right = (current_time - hand_presence.last_seen_right) < FADE_TIMEOUT;
    let show_left = (current_time - hand_presence.last_seen_left) < FADE_TIMEOUT;

    if let Some(mat) = materials.get_mut(&hand_mats.right) {
        if show_right {
            mat.base_color.set_alpha(1.0);
            mat.emissive = LinearRgba::new(0.0, 0.8, 1.0, 1.0);
        } else {
            mat.base_color.set_alpha(0.1);
            mat.emissive = LinearRgba::new(0.0, 0.1, 0.15, 1.0);
        }
    }
    if let Some(mat) = materials.get_mut(&hand_mats.left) {
        if show_left {
            mat.base_color.set_alpha(1.0);
            mat.emissive = LinearRgba::new(1.0, 0.0, 0.8, 1.0);
        } else {
            mat.base_color.set_alpha(0.1);
            mat.emissive = LinearRgba::new(0.15, 0.0, 0.1, 1.0);
        }
    }

    let mut current_positions: HashMap<(HandSide, usize), Vec3> = HashMap::new();

    for (point, transform) in hand_query.iter() {
        if transform.translation.y > -50.0 {
            current_positions.insert((point.side, point.id), transform.translation);
        }
    }

    for side in [HandSide::Right, HandSide::Left] {
        let (is_visible, base_color) = if side == HandSide::Right { 
            (show_right, Color::srgba(0.0, 1.0, 1.0, 1.0))
        } else { 
            (show_left, Color::srgba(1.0, 0.0, 1.0, 1.0))
        };

        let color = if is_visible {
            base_color
        } else {
            base_color.with_alpha(0.1)
        };

        for &(start_idx, end_idx) in HAND_CONNECTIONS {
            if let (Some(&start), Some(&end)) = (
                current_positions.get(&(side, start_idx)),
                current_positions.get(&(side, end_idx))
            ) {
                gizmos.line(start, end, color);
            }
        }
    }
}
