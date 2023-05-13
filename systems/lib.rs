use bevy::input::mouse::{MouseButtonInput, MouseMotion, MouseWheel};
use bevy::sprite::{MaterialMesh2dBundle, Mesh2dHandle};
use bevy::{prelude::*, render::mesh::PrimitiveTopology, window::WindowResolution};
use bevy_rapier2d::prelude::*;
use common::Message;
use crossbeam_channel::{bounded, Receiver, Sender};

mod net;

#[derive(Resource, Deref)]
pub struct ServerEvents {
    rx: Receiver<Message>,
}

impl ServerEvents {
    pub fn new(rx: Receiver<Message>) -> Self {
        Self { rx }
    }
}

fn connect() -> ServerEvents {
    let (tx, rx) = bounded(100);
    std::thread::spawn(move || {
        // ...
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async move {
                net::connect(tx).await.unwrap();
            });
    });
    ServerEvents::new(rx)
}

// The float value is the player movement speed in 'pixels/second'.
#[derive(Component)]
pub struct Player {
    pub speed: f32,
    prev_force: Vec2,
}

static DEFAULT_SPEED: f32 = 2000.0;

#[no_mangle]
pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut rapier_config: ResMut<RapierConfiguration>,
) {
    // Set gravity to 0.0 and spawn camera.
    rapier_config.gravity = Vec2::ZERO;

    commands.insert_resource(connect());
    commands.spawn(Camera2dBundle::default());

    let mut lines: Vec<(Vec3, Vec3)> = Vec::new();
    // Grid lines all over the screen
    let size = 1000.0;
    let step = 100.0;
    for i in -((size / step) as i32)..((size / step) as i32) {
        let i = i as f32;
        lines.push((
            Vec3::new(i * step, -size, 0.0),
            Vec3::new(i * step, size, 0.0),
        ));
        lines.push((
            Vec3::new(-size, i * step, 0.0),
            Vec3::new(size, i * step, 0.0),
        ));
    }

    commands
        .spawn(MaterialMesh2dBundle {
            mesh: Mesh2dHandle(meshes.add(Mesh::from(LineList { lines }))),
            material: materials.add(Color::BLACK.into()),
            ..default()
        })
        .with_children(|parent| {
            let sprite_size = 100.0;

            parent.spawn((
                MaterialMesh2dBundle {
                    mesh: Mesh2dHandle(
                        meshes.add(
                            shape::Circle {
                                radius: sprite_size / 2.0,
                                ..Default::default()
                            }
                            .into(),
                        ),
                    ),
                    transform: Transform::from_translation(Vec3::new(0.0, 0.0, 1.0)),
                    material: materials.add(Color::GREEN.into()),
                    ..Default::default()
                },
                RigidBody::Dynamic,
                Velocity::linear(Vec2::new(0.0, 0.0)),
                ExternalForce {
                    force: Vec2::new(0.0, 0.0),
                    torque: 0.0,
                },
                Collider::ball(sprite_size / 2.0),
                Player { speed: DEFAULT_SPEED, prev_force: Vec2::ZERO },
            ));
        });
}

#[derive(Debug, Clone)]
pub struct LineList {
    pub lines: Vec<(Vec3, Vec3)>,
}

impl From<LineList> for Mesh {
    fn from(line: LineList) -> Self {
        // This tells wgpu that the positions are list of lines
        // where every pair is a start and end point
        let mut mesh = Mesh::new(PrimitiveTopology::LineList);

        let vertices: Vec<_> = line.lines.into_iter().flat_map(|(a, b)| [a, b]).collect();
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
        mesh
    }
}

#[no_mangle]
pub fn read_events(receiver: Res<ServerEvents>, mut events: EventWriter<Message>) {
    for msg in receiver.try_iter() {
        events.send(msg);
    }
}

#[no_mangle]
pub fn spawn_food(
    mut commands: Commands,
    mut reader: EventReader<Message>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    for (per_frame, event) in reader.iter().enumerate() {
        match event {
            Message::SpawnFood(x, y) => {
                // Spawn a small red circle at the given position.
                commands.spawn((
                    MaterialMesh2dBundle {
                        mesh: Mesh2dHandle(
                            meshes.add(
                                shape::Circle {
                                    radius: 10.0,
                                    ..Default::default()
                                }
                                .into(),
                            ),
                        ),
                        transform: Transform::from_translation(Vec3::new(*x, *y, 1.0)),
                        material: materials.add(Color::RED.into()),
                        ..Default::default()
                    },
                    RigidBody::Dynamic,
                    Collider::ball(10.0),
                ));
            }
        }
    }
}

#[no_mangle]
pub fn player_movement(
    keyboard_input: Res<Input<KeyCode>>,
    mut player_info: Query<(&Player, &mut Velocity)>,
) {
    for (player, mut rb_vels) in &mut player_info {
        let up = keyboard_input.any_pressed([KeyCode::W, KeyCode::Up]);
        let down = keyboard_input.any_pressed([KeyCode::S, KeyCode::Down]);
        let left = keyboard_input.any_pressed([KeyCode::A, KeyCode::Left]);
        let right = keyboard_input.any_pressed([KeyCode::D, KeyCode::Right]);

        let x_axis = -(left as i8) + right as i8;
        let y_axis = -(down as i8) + up as i8;

        let mut move_delta = Vec2::new(x_axis as f32, y_axis as f32);
        if move_delta != Vec2::ZERO {
            move_delta /= move_delta.length();
        }

        // Update the velocity on the rigid_body_component,
        // the bevy_rapier plugin will update the Sprite transform.
        rb_vels.linvel = move_delta * player.speed;
    }
}

// Player movement that follows the mouse cursor.
#[no_mangle]
pub fn player_movement_mouse(
    mut player_info: Query<(&mut Player, &mut Velocity, &mut ExternalForce)>,
    mut mouse_motion_events: EventReader<MouseMotion>,
) {
    for (mut player, mut rb_vels, mut force) in &mut player_info {
        for event in mouse_motion_events.iter() {
            let mut move_delta = event.delta;
            // Inverse the y axis, because the mouse y axis is inverted.
            move_delta.y = -move_delta.y;

            if move_delta != Vec2::ZERO {
                move_delta /= move_delta.length();
                if move_delta.x == 0.0 {
                    move_delta.x = player.prev_force.x;
                } 
                if move_delta.y == 0.0 {
                    move_delta.y = player.prev_force.y;
                }

                force.force = move_delta * player.speed;
                player.prev_force = move_delta.clone();
            }
        }
    }
}
