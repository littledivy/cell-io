use bevy::{prelude::*, window::WindowResolution};
use bevy_rapier2d::prelude::*;

#[cfg(not(feature = "reload"))]
use ::systems::*;
#[cfg(feature = "reload")]
use systems_hot::*;

#[cfg(feature = "reload")]
#[hot_lib_reloader::hot_module(dylib = "systems")]
mod systems_hot {
    use ::common::*;
    use ::systems::*;
    use bevy::prelude::*;
    use bevy_rapier2d::prelude::*;

    hot_functions_from_file!("systems/lib.rs");
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: WindowResolution::new(1000., 1000.),
                title: "cell.io".to_string(),
                ..default()
            }),
            ..default()
        }))
        .add_event::<common::Message>()
        .insert_resource(Msaa::default())
        .insert_resource(ClearColor(Color::WHITE))
        .add_startup_system(setup)
        .add_system(read_events)
        .add_system(spawn_food)
        .add_system(player_movement)
        .add_system(player_movement_mouse)
        .add_plugin(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(100.0))
        .run();
}
