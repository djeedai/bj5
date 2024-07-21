// Bevy code commonly triggers these lints and they may be important signals
// about code quality. They are sometimes hard to avoid though, and the CI
// workflow treats them as errors, so this allows them throughout the project.
// Feel free to delete this line.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

use bevy::asset::AssetMetaCheck;
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::render::camera::ScalingMode;
use bevy::window::WindowResolution;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_kira_audio::prelude::*;
use bevy_rapier2d::prelude::*;

mod tiled;

#[derive(Component)]
struct AnimationIndices {
    first: usize,
    last: usize,
}

#[derive(Component, Deref, DerefMut)]
struct AnimationTimer(Timer);

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    // Wasm builds will check for meta files (that don't exist) if this isn't set.
                    // This causes errors and even panics in web builds on itch.
                    // See https://github.com/bevyengine/bevy_github_ci_template/issues/48.
                    meta_check: AssetMetaCheck::Never,
                    ..default()
                })
                .set(LogPlugin {
                    level: bevy::log::Level::WARN,
                    filter: "bj5=trace".to_string(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: String::from("bj5"),
                        resolution: WindowResolution::new(960., 720.),
                        resizable: false,
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugins(WorldInspectorPlugin::default())
        .add_plugins(bevy_ecs_tilemap::TilemapPlugin)
        .add_plugins(tiled::TiledMapPlugin)
        .add_plugins(AudioPlugin)
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(16.0))
        .add_plugins(RapierDebugRenderPlugin::default())
        .insert_resource(ClearColor(Color::BLACK))
        .add_systems(Startup, setup)
        .add_systems(Update, close_on_esc)
        .add_systems(Update, animate_sprites)
        .run();
}

pub fn close_on_esc(mut ev_app_exit: EventWriter<AppExit>, input: Res<ButtonInput<KeyCode>>) {
    if input.just_pressed(KeyCode::Escape) {
        ev_app_exit.send(AppExit::Success);
    }
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    audio: Res<Audio>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    commands.spawn((
        Camera2dBundle {
            projection: OrthographicProjection {
                scale: 1.0,
                near: -1000.0,
                far: 1000.0,
                viewport_origin: Vec2::new(0.5, 0.5),
                scaling_mode: ScalingMode::WindowSize(3.0),
                ..default()
            },
            ..default()
        },
        Name::new("Camera"),
    ));

    // Load map
    let map_handle: Handle<tiled::TiledMap> = asset_server.load("map1.tmx");
    commands.spawn((
        tiled::TiledMapBundle {
            tiled_map: map_handle,
            ..Default::default()
        },
        Name::new("TiledLevel"),
    ));

    // Load player
    let player_sheet = asset_server.load("player1.png");
    let player_layout =
        TextureAtlasLayout::from_grid(UVec2::splat(15), 4, 1, Some(UVec2::ONE), None);
    let player_atlas_layout = texture_atlas_layouts.add(player_layout);
    commands.spawn((
        SpriteBundle {
            transform: Transform::from_xyz(40., 0., 10.),
            texture: player_sheet,
            ..default()
        },
        TextureAtlas {
            layout: player_atlas_layout,
            index: 0,
        },
        AnimationIndices { first: 0, last: 3 },
        AnimationTimer(Timer::from_seconds(0.1, TimerMode::Repeating)),
        RigidBody::Dynamic,
        Collider::ball(7.5),
        Name::new("Player"),
    ));

    // Start background audio
    //audio.play(asset_server.load("background_audio.ogg")).looped();
}

fn animate_sprites(
    time: Res<Time>,
    mut query: Query<(&AnimationIndices, &mut AnimationTimer, &mut TextureAtlas)>,
) {
    for (indices, mut timer, mut atlas) in &mut query {
        timer.tick(time.delta());
        if timer.just_finished() {
            atlas.index = if atlas.index == indices.last {
                indices.first
            } else {
                atlas.index + 1
            };
        }
    }
}
