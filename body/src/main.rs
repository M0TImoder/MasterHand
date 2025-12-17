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
}

#[derive(Deserialize, Debug)]
struct HandPacket {
    hands: Vec<OneHand>,
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

#[derive(Resource)]
struct UdpConnection(UdpSocket);

fn main() {
    let socket = UdpSocket::bind("127.0.0.1:5005").expect("Bind failed");
    socket.set_nonblocking(true).expect("Nonblocking failed");

    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
        .insert_resource(UdpConnection(socket))
        .add_systems(Startup, setup)
        .add_systems(Update, update_hands_and_draw_wires)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // カメラ
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(0.0, 5.0, 15.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });

    // ライト
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            intensity: 1500.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(4.0, 8.0, 4.0),
        ..default()
    });

    // 床
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Plane3d::default().mesh().size(20.0, 20.0)),
            material: materials.add(Color::srgb(0.3, 0.5, 0.3)),
            transform: Transform::from_xyz(0.0, -5.0, 0.0),
            ..default()
        },
        RigidBody::Fixed,
        Collider::cuboid(10.0, 0.01, 10.0), 
    ));

    // 箱
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
            material: materials.add(Color::srgb(0.8, 0.7, 0.6)),
            transform: Transform::from_xyz(0.0, 0.0, 0.0),
            ..default()
        },
        RigidBody::Dynamic,
        Collider::cuboid(0.5, 0.5, 0.5),
        Restitution::coefficient(0.7),
    ));

    // 手の関節
    let sphere_mesh = meshes.add(Sphere::new(0.1));
    let right_mat = materials.add(Color::srgb(0.0, 1.0, 1.0));
    let left_mat = materials.add(Color::srgb(1.0, 0.0, 1.0));

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
            ));
        }
    }
}

// MediaPipeの定義する「骨のつながり」リスト
// (親のID, 子のID)
const HAND_CONNECTIONS: &[(usize, usize)] = &[
    (0, 1), (1, 2), (2, 3), (3, 4),   // 親指
    (0, 5), (5, 6), (6, 7), (7, 8),   // 人差し指
    (9, 10), (10, 11), (11, 12),      // 中指 (根元は下で処理)
    (13, 14), (14, 15), (15, 16),     // 薬指
    (0, 17), (17, 18), (18, 19), (19, 20), // 小指
    (5, 9), (9, 13), (13, 17)         // 手のひら
];

fn update_hands_and_draw_wires(
    socket_res: Res<UdpConnection>,
    mut query: Query<(&HandPoint, &mut Transform)>,
    mut gizmos: Gizmos,
) {
    let mut buf = [0; 65536];
    let mut latest_packet: Option<HandPacket> = None;

    // バッファにあるデータを全て吸い出す
    while let Ok((amt, _src)) = socket_res.0.recv_from(&mut buf) {
        let valid_data = &buf[..amt];
        if let Ok(packet) = serde_json::from_slice::<HandPacket>(valid_data) {
            latest_packet = Some(packet);
        }
    }

    if let Some(packet) = latest_packet {
        
        let mut positions: HashMap<(HandSide, usize), Vec3> = HashMap::new();

        for (point, mut transform) in query.iter_mut() {
            let target_hand_data = packet.hands.iter().find(|h| {
                match point.side {
                    HandSide::Right => h.label == "Right",
                    HandSide::Left => h.label == "Left",
                }
            });

            if let Some(hand_data) = target_hand_data {
                if let Some(lm) = hand_data.landmarks.iter().find(|l| l.id == point.id) {
                    let scale = 12.0;

                    let x = (lm.x - 0.5) * scale; 
                    let y = (0.5 - lm.y) * scale + 2.0;
                    let z = lm.z * scale + 3.0;

                    let new_pos = Vec3::new(x, y, z);
                    transform.translation = new_pos;
                    positions.insert((point.side, point.id), new_pos);
                }
            }
        }

        // ワイヤー描画
        for side in [HandSide::Right, HandSide::Left] {
            let color = if side == HandSide::Right { 
                Color::srgb(0.0, 1.0, 1.0) 
            } else { 
                Color::srgb(1.0, 0.0, 1.0) 
            };

            for &(start_idx, end_idx) in HAND_CONNECTIONS {
                if let (Some(&start), Some(&end)) = (
                    positions.get(&(side, start_idx)),
                    positions.get(&(side, end_idx))
                ) {
                    gizmos.line(start, end, color);
                }
            }
        }
    }
}
