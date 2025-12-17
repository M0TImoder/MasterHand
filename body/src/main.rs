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
        .add_systems(Update, update_hands_and_spawn)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut gizmo_config: ResMut<GizmoConfigStore>,
) {
    // ワイヤーを見やすくする設定
    let (config, _) = gizmo_config.config_mut::<DefaultGizmoConfigGroup>();
    config.depth_bias = -1.0;
    config.line_width = 3.0;

    // カメラ位置
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(0.0, 10.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });

    // ライト
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

    // 床
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

    // 最初の箱
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
    let sphere_mesh = meshes.add(Sphere::new(0.08));
    let right_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.0, 0.8, 1.0),
        emissive: LinearRgba::new(0.0, 0.8, 1.0, 1.0),
        ..default()
    });
    let left_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.0, 0.8),
        emissive: LinearRgba::new(1.0, 0.0, 0.8, 1.0),
        ..default()
    });

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
    (0, 1), (1, 2), (2, 3), (3, 4),
    (0, 5), (5, 6), (6, 7), (7, 8),
    (9, 10), (10, 11), (11, 12),
    (13, 14), (14, 15), (15, 16),
    (0, 17), (17, 18), (18, 19), (19, 20),
    (5, 9), (9, 13), (13, 17)
];

fn update_hands_and_spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    socket_res: Res<UdpConnection>,
    mut query: Query<(&HandPoint, &mut Transform)>,
    mut gizmos: Gizmos,
    time: Res<Time>, 
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
        
        // スナップ検知で箱を生成
        if packet.snap {
            let rand_x = (time.elapsed_seconds() * 10.0).sin() * 5.0;
            commands.spawn((
                PbrBundle {
                    mesh: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
                    material: materials.add(Color::srgb(1.0, 0.5, 0.0)),
                    transform: Transform::from_xyz(rand_x, 15.0, 0.0),
                    ..default()
                },
                RigidBody::Dynamic,
                Collider::cuboid(0.5, 0.5, 0.5),
                Restitution::coefficient(0.5),
            ));
        }

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
                    
                    let scale = 20.0;

                    let x = (lm.x - 0.5) * scale; 
                    let y = (0.5 - lm.y) * scale + 3.0; 
                    let z = lm.z * scale + 8.0;

                    let target_pos = Vec3::new(x, y, z);
                    
                    let smooth_factor = 40.0 * time.delta_seconds(); 
                    let t = smooth_factor.clamp(0.0, 1.0);
                    transform.translation = transform.translation.lerp(target_pos, t);
                    
                    positions.insert((point.side, point.id), transform.translation);
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
