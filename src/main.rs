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
use bevy_keith::{Canvas, KeithPlugin};
use bevy_kira_audio::prelude::*;
use bevy_rapier2d::prelude::*;
use bevy_rapier2d::rapier::geometry::CollisionEventFlags;

mod tiled;

#[derive(Default, Component)]
struct MainCamera {}

#[derive(Default, Component)]
struct PlayerStart {
    pub position: Vec3,
}

#[derive(Component)]
struct Teleporter {
    pub target: Entity,
}

impl Default for Teleporter {
    fn default() -> Self {
        Self {
            target: Entity::PLACEHOLDER,
        }
    }
}

impl Teleporter {
    pub fn new(target: Entity) -> Self {
        Self { target }
    }
}

#[derive(Component, Reflect)]
struct Player {
    pub impulse_factor: f32,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            impulse_factor: 500.,
        }
    }
}

#[derive(Component)]
struct AnimationIndices {
    first: usize,
    last: usize,
}

#[derive(Component, Deref, DerefMut)]
struct AnimationTimer(Timer);

#[derive(Resource)]
struct UiRes {
    pub font: Handle<Font>,
}

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
        .add_plugins(KeithPlugin)
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(16.0))
        .add_plugins(RapierDebugRenderPlugin {
            mode: DebugRenderMode::default()
                | DebugRenderMode::CONTACTS
                | DebugRenderMode::SOLVER_CONTACTS,
            ..default()
        })
        .register_type::<Player>()
        .insert_resource(ClearColor(Color::BLACK))
        .add_systems(Startup, setup)
        .add_systems(Update, post_load_setup)
        .add_systems(Update, close_on_esc)
        .add_systems(Update, animate_sprites)
        .add_systems(Update, player_input)
        .add_systems(Update, teleport)
        .add_systems(Update, main_ui)
        .add_systems(PostUpdate, update_camera)
        .run();
}

pub fn close_on_esc(mut ev_app_exit: EventWriter<AppExit>, input: Res<ButtonInput<KeyCode>>) {
    if input.just_pressed(KeyCode::Escape) {
        ev_app_exit.send(AppExit::Success);
    }
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>, _audio: Res<Audio>) {
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
        MainCamera {},
        Name::new("Camera"),
    ));

    commands.spawn((
        Camera2dBundle {
            camera: Camera {
                order: 100,
                ..default()
            },
            projection: OrthographicProjection {
                scale: 1.0,
                near: -1000.0,
                far: 1000.0,
                viewport_origin: Vec2::new(0.5, 0.5),
                scaling_mode: ScalingMode::WindowSize(1.0),
                ..default()
            },
            ..default()
        },
        Canvas::default(),
        Name::new("UICamera"),
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

    // Start background audio
    // audio.play(asset_server.load("background_audio.ogg")).looped();
}

fn post_load_setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    q_player_start: Query<&PlayerStart, Added<PlayerStart>>,
    mut q_camera: Query<&mut Transform, With<MainCamera>>,
) {
    let Ok(player_start) = q_player_start.get_single() else {
        return;
    };

    // Move camera
    if let Ok(mut camera_transform) = q_camera.get_single_mut() {
        camera_transform.translation.x = player_start.position.x;
        camera_transform.translation.y = player_start.position.y;
    }

    // Spawn player
    trace!("Spawning player at {:?}...", player_start.position);
    let player_sheet = asset_server.load("player1.png");
    let player_layout =
        TextureAtlasLayout::from_grid(UVec2::splat(15), 4, 1, Some(UVec2::ONE), None);
    let player_atlas_layout = texture_atlas_layouts.add(player_layout);
    commands.spawn((
        SpriteBundle {
            transform: Transform::from_xyz(player_start.position.x, player_start.position.y, 10.),
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
        Ccd::enabled(),
        ExternalImpulse::default(),
        ActiveEvents::COLLISION_EVENTS,
        Collider::ball(7.5),
        Name::new("Player"),
        Player::default(),
    ));
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

fn player_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut player: Query<(&Player, &mut ExternalImpulse)>,
) {
    let Ok((player, mut impulse)) = player.get_single_mut() else {
        return;
    };

    let mut dv = Vec2::ZERO;
    if keyboard.pressed(KeyCode::KeyA) {
        dv.x -= 1.;
    }
    if keyboard.pressed(KeyCode::KeyD) {
        dv.x += 1.;
    }
    if keyboard.just_pressed(KeyCode::Space) {
        dv.y += 30.;
    }
    //trace!("dv: {:?}", dv);

    if dv != Vec2::ZERO {
        impulse.impulse = dv * player.impulse_factor;
    }
}

fn teleport(
    q_teleporters: Query<(Entity, &mut Transform, &Teleporter), Without<Player>>,
    mut q_player: Query<(Entity, &mut Transform), With<Player>>,
    mut events: EventReader<CollisionEvent>,
) {
    let Ok((player_entity, mut player_transform)) = q_player.get_single_mut() else {
        return;
    };

    for ev in events.read() {
        match ev {
            CollisionEvent::Started(e1, e2, flags) => {
                trace!("Started: e1={:?} e2={:?} flags={:?}", e1, e2, flags);
            }
            CollisionEvent::Stopped(e1, e2, flags) => {
                trace!("Stopped: e1={:?} e2={:?} flags={:?}", e1, e2, flags);
                // Detect when player stops overlapping a teleporter
                if flags.contains(CollisionEventFlags::SENSOR) {
                    let mut e1 = *e1;
                    let mut e2 = *e2;
                    // Swap entities such that player is always #1 and TP is always #2
                    if e2 == player_entity {
                        std::mem::swap(&mut e1, &mut e2);
                    }
                    if e1 == player_entity {
                        if let Ok(tp1) = q_teleporters.get(e2) {
                            if let Ok(tp2) = q_teleporters.get(tp1.2.target) {
                                // tp1 -> tp2

                                // Find the exit side, to determine the teleport edge.
                                let delta = player_transform.translation - tp1.1.translation;
                                if delta.x > 0. {
                                    // Exited to the right, so teleport to the right edge of tp2
                                    let edge = tp2.1.translation; // TODO - width of TP
                                    debug!("Teleport player from TP {:?} at delta {:?} to TP {:?} at {:?}", tp1.0, delta, tp2.0, edge + delta);
                                    player_transform.translation.x = edge.x + delta.x;
                                    player_transform.translation.y = edge.y + delta.y;
                                } else {
                                    // Exited to the left, so teleport to the right left of tp2
                                    let edge = tp2.1.translation; // TODO - width of TP
                                    debug!("Teleport player from TP {:?} at delta {:?} to TP {:?} at {:?}", tp1.0, delta, tp2.0, edge + delta);
                                    player_transform.translation.x = edge.x + delta.x;
                                    player_transform.translation.y = edge.y + delta.y;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn update_camera(
    player: Query<&Transform, (With<Player>, Without<MainCamera>)>,
    mut camera: Query<&mut Transform, (With<MainCamera>, Without<Player>)>,
) {
    let Ok(player) = player.get_single() else {
        return;
    };
    let Ok(mut camera) = camera.get_single_mut() else {
        return;
    };
    // TEMP: no smoothing or loose follow or any fancy setup, just stick to the
    // player
    camera.translation = player.translation;
}

fn main_ui(mut q_canvas: Query<&mut Canvas>) {
    let mut canvas = q_canvas.single_mut();
    canvas.clear();

    let mut ctx = canvas.render_context();
    let txt = ctx
        .new_layout("Time: 017")
        .font_size(16.)
        .color(Color::WHITE)
        .alignment(JustifyText::Left)
        .bounds(Vec2::new(100., 20.))
        .build();
    ctx.draw_text(txt, Vec2::new(-430., -340.));

    let brush = ctx.solid_brush(Color::WHITE);
    ctx.fill(Rect::new(0., 0., 160., 120.), &brush);
}
