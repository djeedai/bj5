#![allow(clippy::too_many_arguments, clippy::type_complexity)]

use bevy::{
    asset::AssetMetaCheck, input::common_conditions::input_toggle_active, log::LogPlugin,
    prelude::*, render::camera::ScalingMode, window::WindowResolution,
};
use bevy_ecs_tilemap::tiles::{TileTextureIndex, TileVisible};
#[cfg(feature = "debug")]
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_keith::{Canvas, KeithPlugin, ShapeExt};
use bevy_kira_audio::prelude::*;
use bevy_rapier2d::{prelude::*, rapier::geometry::CollisionEventFlags};

mod components;
mod tiled;

pub use components::*;
pub use tiled::*;

#[derive(Default, Resource)]
struct UiRes {
    pub font: Handle<Font>,
    pub title_image: Handle<Image>,
    pub cursor_image: Handle<Image>,
    pub cursor_atlas_layout: Handle<TextureAtlasLayout>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, States)]
enum AppState {
    #[default]
    MainMenu,
    //SettingsMenu,
    InGame,
    GameOver,
}

#[derive(Default, Resource)]
struct MainMenu {
    pub selected_index: usize,
}

fn main() {
    let mut app = App::new();

    app.add_plugins(
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
                filter: "wheel-of-time=trace".to_string(),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: String::from("Wheel of Time - Bevy Game Jame #5"),
                    resolution: WindowResolution::new(960., 720.),
                    resizable: false,
                    ..default()
                }),
                ..default()
            })
            .set(ImagePlugin::default_nearest()),
    );

    #[cfg(feature = "debug")]
    app.add_plugins(
        WorldInspectorPlugin::default().run_if(input_toggle_active(false, KeyCode::F1)),
    );

    app.add_plugins(bevy_ecs_tilemap::TilemapPlugin)
        .add_plugins(tiled::TiledMapPlugin)
        .add_plugins(AudioPlugin)
        .add_plugins(KeithPlugin)
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(16.0))
        .add_plugins(RapierDebugRenderPlugin {
            enabled: false,
            mode: DebugRenderMode::default()
                | DebugRenderMode::CONTACTS
                | DebugRenderMode::SOLVER_CONTACTS,
            ..default()
        })
        .register_type::<Player>()
        .insert_resource(ClearColor(Color::BLACK))
        .init_resource::<UiRes>()
        .init_resource::<MainMenu>()
        .init_state::<AppState>()
        // General setup
        .add_systems(Startup, setup)
        // All-state
        .add_systems(Update, close_on_esc)
        // Debug
        .add_systems(First, toggle_debug)
        // Main menu
        .add_systems(OnEnter(AppState::MainMenu), setup_main_menu)
        .add_systems(
            PreUpdate,
            main_menu_inputs.run_if(in_state(AppState::MainMenu)),
        )
        .add_systems(Update, ui_main_menu.run_if(in_state(AppState::MainMenu)))
        // In-game
        .add_systems(PreUpdate, player_input.run_if(in_state(AppState::InGame)))
        .add_systems(OnEnter(AppState::InGame), post_load_setup)
        .add_systems(
            Update,
            (
                animate_sprites,
                animate_tiles,
                teleport,
                damage_player,
                main_ui,
                check_victory,
            )
                .run_if(in_state(AppState::InGame)),
        )
        .add_systems(
            PostUpdate,
            (update_camera, apply_epoch).run_if(in_state(AppState::InGame)),
        )
        // Game over
        .add_systems(Update, (game_over_ui,).run_if(in_state(AppState::GameOver)));

    app.run();
}

pub fn toggle_debug(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut debug_ctx: ResMut<DebugRenderContext>,
) {
    if keyboard.just_pressed(KeyCode::F1) {
        debug_ctx.enabled = !debug_ctx.enabled;
    }
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
    mut ui_res: ResMut<UiRes>,
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

    commands.spawn(Epoch::default());

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
    audio.play(asset_server.load("bgm1.ogg")).looped();

    ui_res.font = asset_server.load("fonts/PressStart2P-Regular.ttf");

    ui_res.title_image = asset_server.load("title.png");

    ui_res.cursor_image = asset_server.load("player1.png");
    let player_layout =
        TextureAtlasLayout::from_grid(UVec2::splat(15), 4, 1, Some(UVec2::ONE), None);
    let player_atlas_layout = texture_atlas_layouts.add(player_layout);
    ui_res.cursor_atlas_layout = player_atlas_layout;
}

fn post_load_setup(
    mut commands: Commands,
    q_player_start: Query<&PlayerStart, Added<PlayerStart>>,
    mut q_camera: Query<&mut Transform, With<MainCamera>>,
    ui_res: Res<UiRes>,
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
    commands.spawn((
        SpriteBundle {
            transform: Transform::from_xyz(player_start.position.x, player_start.position.y, 4.),
            texture: ui_res.cursor_image.clone(),
            ..default()
        },
        TextureAtlas {
            layout: ui_res.cursor_atlas_layout.clone(),
            index: 0,
        },
        TileAnimation::uniform(0, 2, 100),
        RigidBody::Dynamic,
        Ccd::enabled(),
        ExternalImpulse::default(),
        ActiveEvents::COLLISION_EVENTS,
        Collider::ball(7.5),
        Velocity::zero(),
        GravityScale(1.),
        Name::new("Player"),
        Player::default(),
        PlayerController::default(),
        PlayerLife::default(),
    ));
}

fn animate_sprites(time: Res<Time>, mut query: Query<(&mut TileAnimation, &mut TextureAtlas)>) {
    for (mut anim, mut atlas) in &mut query {
        let idx = anim.tick(time.delta().as_millis() as u32) as usize;
        if idx != atlas.index {
            atlas.index = idx;
        }
    }
}

fn animate_tiles(time: Res<Time>, mut query: Query<(&mut TileAnimation, &mut TileTextureIndex)>) {
    for (mut anim, mut tex_index) in &mut query {
        let idx = anim.tick(time.delta().as_millis() as u32);
        if idx != tex_index.0 {
            tex_index.0 = idx;
        }
    }
}

fn player_input(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut player: Query<(
        Entity,
        &Player,
        &PlayerLife,
        &mut PlayerController,
        &mut Velocity,
        &mut GravityScale,
        &mut ExternalImpulse,
    )>,
    physics: Res<RapierContext>,
    q_ladders: Query<Entity, With<Ladder>>,
) {
    let Ok((
        player_entity,
        player,
        player_life,
        mut player_controller,
        mut velocity,
        mut gravity_scale,
        mut impulse,
    )) = player.get_single_mut()
    else {
        return;
    };

    let mut is_grounded = false;

    for c in physics.contact_pairs_with(player_entity) {
        for m in c.manifolds() {
            if m.normal().y > 0.7 {
                is_grounded = true;
                break;
            }
        }
    }
    if player_controller.is_grounded != is_grounded {
        player_controller.is_grounded = is_grounded;
    }

    // If not already on a ladder, check if intersecting one
    if !player_controller.is_climbing
        && (keyboard.pressed(KeyCode::KeyW) || keyboard.pressed(KeyCode::KeyS))
    {
        for (e1, e2, _) in physics.intersection_pairs_with(player_entity) {
            assert!(e1 == player_entity || e2 == player_entity);
            let other_entity = if e1 == player_entity { e2 } else { e1 };
            // Check if the other entity is a ladder
            if q_ladders.contains(other_entity) {
                player_controller.is_climbing = true;
                gravity_scale.0 = 0.;
                break;
            }
        }
    } else if player_controller.is_climbing {
        // Falling from ladder
        let mut is_on_ladder = false;
        for (e1, e2, _) in physics.intersection_pairs_with(player_entity) {
            assert!(e1 == player_entity || e2 == player_entity);
            let other_entity = if e1 == player_entity { e2 } else { e1 };
            // Check if the other entity is a ladder
            if q_ladders.contains(other_entity) {
                is_on_ladder = true;
                break;
            }
        }
        if !is_on_ladder {
            player_controller.is_climbing = false;
            gravity_scale.0 = 1.;
        }
    }

    let mut dv = Vec2::ZERO;
    if keyboard.pressed(KeyCode::KeyA) {
        dv.x -= 1.;
    }
    if keyboard.pressed(KeyCode::KeyD) {
        dv.x += 1.;
    }
    if (is_grounded || player_controller.is_climbing) && keyboard.just_pressed(KeyCode::Space) {
        dv.y += 30.;
        if player_controller.is_climbing {
            player_controller.is_climbing = false;
            gravity_scale.0 = 1.;
        }
    }

    if player_controller.is_climbing {
        let mut target_velocity = velocity.linvel;
        let mut has_input = false;
        if keyboard.pressed(KeyCode::KeyW) {
            target_velocity.y += 2.;
            has_input = true;
        } else if keyboard.pressed(KeyCode::KeyS) {
            target_velocity.y -= 2.;
            has_input = true;
        }
        if keyboard.pressed(KeyCode::KeyA) {
            target_velocity.x -= 1.;
            has_input = true;
        } else if keyboard.pressed(KeyCode::KeyD) {
            target_velocity.x += 1.;
            has_input = true;
        }
        if !has_input {
            target_velocity = Vec2::ZERO;
        }
        let new_vel = target_velocity.clamp_length_max(50.);
        if new_vel != velocity.linvel {
            velocity.linvel = new_vel;
        }
    }

    // trace!("dv: {:?}", dv);

    let mut dv = dv * player.impulse_factor;

    // If damaged, apply the (gradually fading) damage impulse
    if let Some(ratio) = player_life.damage_impulse_factor(time.elapsed()) {
        // warn!(
        //     "ratio={} dv={:?} dir={:?}",
        //     ratio,
        //     dv,
        //     player_life.last_dmg_dir * 6000.
        // );
        dv = dv.lerp(player_life.last_dmg_dir * 6000., 1. - ratio);
        //warn!("dv={:?}", dv);
    }

    if dv != impulse.impulse {
        impulse.impulse = dv;
    }
}

fn teleport(
    q_teleporters: Query<(Entity, &mut Transform, &Teleporter), Without<Player>>,
    mut q_player: Query<(Entity, &mut Transform, &mut Player)>,
    mut events: EventReader<CollisionEvent>,
    mut epoch: Query<&mut Epoch>,
) {
    let Ok((player_entity, mut player_transform, mut player)) = q_player.get_single_mut() else {
        return;
    };

    let mut tp_dir = 0;
    for ev in events.read() {
        match ev {
            CollisionEvent::Started(e1, e2, flags) => {
                // trace!("Started: e1={:?} e2={:?} flags={:?}", e1, e2, flags);

                // Detect when player starts overlapping a teleporter
                if flags.contains(CollisionEventFlags::SENSOR) {
                    let mut e1 = *e1;
                    let mut e2 = *e2;
                    // Swap entities such that player is always #1 and TP is always #2
                    if e2 == player_entity {
                        std::mem::swap(&mut e1, &mut e2);
                    }
                    if e1 == player_entity {
                        if let Ok(tp1) = q_teleporters.get(e2) {
                            // Save the teleporter enter side
                            player.teleporter_side =
                                player_transform.translation.x - tp1.1.translation.x;
                        }
                    }
                }
            }
            CollisionEvent::Stopped(e1, e2, flags) => {
                // trace!("Stopped: e1={:?} e2={:?} flags={:?}", e1, e2, flags);

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
                            // Find the exit side, to determine the teleport edge.
                            let delta = player_transform.translation - tp1.1.translation;

                            // If the player exits from the same side it entered, ignore.
                            if delta.x * player.teleporter_side >= 0. {
                                player.teleporter_side = 0.;
                                continue;
                            }

                            if let Ok(tp2) = q_teleporters.get(tp1.2.target) {
                                // tp1 -> tp2

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

                                tp_dir = if tp2.1.translation.x > tp1.1.translation.x {
                                    1
                                } else {
                                    -1
                                };
                            }
                        }
                    }
                }
            }
        }
    }

    // Change epoch
    if tp_dir != 0 {
        let mut epoch = epoch.single_mut();
        if tp_dir < 0 && epoch.cur < epoch.max {
            debug!("Epoch {} -> {}", epoch.cur, epoch.cur + 1);
            epoch.cur += 1;
        } else if tp_dir > 0 && epoch.cur > epoch.min {
            debug!("Epoch {} -> {}", epoch.cur, epoch.cur - 1);
            epoch.cur -= 1;
        }
    }
}

fn damage_player(
    time: Res<Time>,
    mut q_player: Query<(Entity, &Transform, &mut PlayerLife, &mut ExternalImpulse)>,
    q_damage: Query<(&Damage, &Transform), Without<PlayerLife>>,
    mut events: EventReader<CollisionEvent>,
    mut app_state: ResMut<NextState<AppState>>,
) {
    let Ok((player_entity, player_transform, mut player_life, mut player_impulse)) =
        q_player.get_single_mut()
    else {
        return;
    };

    for ev in events.read() {
        let CollisionEvent::Started(e1, e2, flags) = ev else {
            continue;
        };

        // trace!("Started: e1={:?} e2={:?} flags={:?}", e1, e2, flags);

        // Detect when player starts overlapping a teleporter
        if flags.contains(CollisionEventFlags::SENSOR) {
            let mut e1 = *e1;
            let mut e2 = *e2;
            // Swap entities such that player is always #1 and TP is always #2
            if e2 == player_entity {
                std::mem::swap(&mut e1, &mut e2);
            }
            if e1 == player_entity {
                if let Ok((dmg, dmg_transform)) = q_damage.get(e2) {
                    let dir = (player_transform.translation.xy() - dmg_transform.translation.xy())
                        .normalize();
                    //error!("dir={:?}", dir);
                    player_life.damage(time.elapsed(), dmg.0, dir);
                    if player_life.life <= 0. {
                        app_state.set(AppState::GameOver);
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

fn main_ui(
    mut q_canvas: Query<&mut Canvas>,
    q_player: Query<&PlayerLife>,
    //q_temp: Query<&PlayerController>,
    //ui_res: Res<UiRes>,
) {
    let mut canvas = q_canvas.single_mut();
    canvas.clear();

    let mut ctx = canvas.render_context();

    let brush = ctx.solid_brush(Color::srgba(0., 0., 0., 0.7));
    ctx.fill(Rect::new(-480., -370., -380., -325.), &brush);

    // // TEMP
    // if let Ok(pc) = q_temp.get_single() {
    //     let txt = ctx
    //         //.new_layout("Time: 017")
    //         .new_layout(format!(
    //             "grounded={} climbing={}",
    //             pc.is_grounded, pc.is_climbing
    //         ))
    //         .font(ui_res.font.clone())
    //         .font_size(16.)
    //         .color(Color::WHITE)
    //         .alignment(JustifyText::Left)
    //         .bounds(Vec2::new(100., 20.))
    //         .build();
    //     ctx.draw_text(txt, Vec2::new(-430., -340.));
    // }

    if let Ok(player_life) = q_player.get_single() {
        let r = Rect::new(-470., -320., -320., -340.);

        let brush = ctx.solid_brush(Color::BLACK);
        let border_brush = ctx.solid_brush(Color::WHITE);
        ctx.fill(r, &brush).border(&border_brush, 2.);

        let brush = ctx.solid_brush(Color::srgb(1., 0., 0.));
        let mut r = r.inflate(-3.);
        r.max.x = r.min.x + (r.width() / player_life.max_life * player_life.life);
        ctx.fill(r, &brush);
    }
}

fn check_victory(
    mut q_player: Query<Entity, With<Player>>,
    mut events: EventReader<CollisionEvent>,
    q_level_end: Query<Entity, With<LevelEnd>>,
    mut app_state: ResMut<NextState<AppState>>,
) {
    let Ok(player_entity) = q_player.get_single_mut() else {
        return;
    };

    for ev in events.read() {
        let CollisionEvent::Started(e1, e2, flags) = ev else {
            continue;
        };

        // trace!("Started: e1={:?} e2={:?} flags={:?}", e1, e2, flags);

        // Detect when player starts overlapping a teleporter
        if flags.contains(CollisionEventFlags::SENSOR) {
            let mut e1 = *e1;
            let mut e2 = *e2;
            // Swap entities such that player is always #1 and TP is always #2
            if e2 == player_entity {
                std::mem::swap(&mut e1, &mut e2);
            }
            if e1 == player_entity {
                if q_level_end.contains(e2) {
                    info!("LevelEnd!");
                    app_state.set(AppState::GameOver);
                }
            }
        }
    }
}

fn game_over_ui(ui_res: Res<UiRes>, mut q_canvas: Query<&mut Canvas>) {
    let mut canvas = q_canvas.single_mut();
    canvas.clear();

    let mut ctx = canvas.render_context();

    let brush = ctx.solid_brush(Color::srgba(0., 0., 0., 0.7));
    ctx.fill(Rect::new(-480., -370., -380., -325.), &brush);

    // Background
    // let brush = ctx.solid_brush(Srgba::hex("3b69ba").unwrap().into());
    // let screen_rect = Rect::new(-480., -360., 480., 360.);
    // ctx.fill(screen_rect, &brush);

    // Game over
    let txt = ctx
        .new_layout("Game Over")
        .font(ui_res.font.clone())
        .font_size(32.)
        .color(Color::WHITE)
        .alignment(JustifyText::Left)
        .bounds(Vec2::new(300., 20.))
        .build();
    ctx.draw_text(txt, Vec2::new(0., 190.));

    let txt = ctx
        .new_layout("Press ESC / refresh page to quit")
        .font(ui_res.font.clone())
        .font_size(16.)
        .color(Color::WHITE)
        .alignment(JustifyText::Left)
        .bounds(Vec2::new(300., 100.))
        .build();
    ctx.draw_text(txt, Vec2::new(0., 250.));
}

fn apply_epoch(
    epoch: Query<&Epoch, Changed<Epoch>>,
    mut q_epoch_sprites: Query<(&EpochSprite, &mut TileTextureIndex, &mut TileVisible)>,
) {
    let Ok(epoch) = epoch.get_single() else {
        return;
    };

    for (epoch_sprite, mut tile_tex_id, mut tile_visible) in &mut q_epoch_sprites {
        let tile_epoch = epoch.cur + epoch_sprite.delta;
        if tile_epoch >= epoch_sprite.first && tile_epoch <= epoch_sprite.last {
            if !tile_visible.0 {
                tile_visible.0 = true;
            }

            let new_id = epoch_sprite.base as u32 + (tile_epoch - epoch_sprite.first) as u32;
            if new_id != tile_tex_id.0 {
                trace!(
                    "Sprite #{}: epoch={} tile_epoch={} in [{},{}] => visible=true, new_id={}",
                    tile_tex_id.0,
                    epoch.cur,
                    tile_epoch,
                    epoch_sprite.first,
                    epoch_sprite.last,
                    new_id
                );
                tile_tex_id.0 = new_id;
            }
        } else {
            if tile_visible.0 {
                trace!(
                    "Sprite #{}: epoch={} tile_epoch={} out of [{},{}] => visible=false",
                    tile_tex_id.0,
                    epoch.cur,
                    tile_epoch,
                    epoch_sprite.first,
                    epoch_sprite.last
                );
                tile_visible.0 = false;
            }
        }
    }
}

fn setup_main_menu() {}

fn main_menu_inputs(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut main_menu: ResMut<MainMenu>,
    mut app_state: ResMut<NextState<AppState>>,
    mut ev_app_exit: EventWriter<AppExit>,
) {
    if (keyboard.just_pressed(KeyCode::KeyW) || keyboard.just_pressed(KeyCode::ArrowUp))
        && main_menu.selected_index > 0
    {
        main_menu.selected_index -= 1;
    } else if (keyboard.just_pressed(KeyCode::KeyS) || keyboard.just_pressed(KeyCode::ArrowDown))
        && main_menu.selected_index < 1
    {
        main_menu.selected_index += 1;
    }

    if keyboard.just_pressed(KeyCode::Enter) || keyboard.just_pressed(KeyCode::NumpadEnter) {
        match main_menu.selected_index {
            0 => app_state.set(AppState::InGame),
            1 => {
                ev_app_exit.send(AppExit::Success);
            }
            _ => (),
        }
    }
}

fn ui_main_menu(mut q_canvas: Query<&mut Canvas>, ui_res: Res<UiRes>, main_menu: Res<MainMenu>) {
    let mut canvas = q_canvas.single_mut();
    canvas.clear();

    let mut ctx = canvas.render_context();

    // Background
    let brush = ctx.solid_brush(Srgba::hex("3b69ba").unwrap().into());
    let screen_rect = Rect::new(-480., -360., 480., 360.);
    ctx.fill(screen_rect, &brush);

    // Title
    let title_rect = Rect::new(-408., -130., 408., 130.);
    let brush = ctx.solid_brush(Color::WHITE);
    ctx.fill(title_rect, &brush);
    ctx.draw_image(
        title_rect,
        ui_res.title_image.clone(),
        bevy_keith::ImageScaling::Uniform(2.),
    );

    let txt = ctx
        .new_layout("New Game")
        .font(ui_res.font.clone())
        .font_size(32.)
        .color(Color::WHITE)
        .alignment(JustifyText::Left)
        .bounds(Vec2::new(300., 20.))
        .build();
    ctx.draw_text(txt, Vec2::new(0., 190.));

    let txt = ctx
        .new_layout("Exit")
        .font(ui_res.font.clone())
        .font_size(32.)
        .color(Color::WHITE)
        .alignment(JustifyText::Left)
        .bounds(Vec2::new(300., 20.))
        .build();
    ctx.draw_text(txt, Vec2::new(0., 250.));

    // commands.spawn((
    //     SpriteBundle {
    //         transform: Transform::from_xyz(player_start.position.x, player_start.position.y, 4.),
    //         texture: ui_res.cursor_image.clone(),
    //         ..default()
    //     },
    //     TextureAtlas {
    //         layout: ui_res.cursor_atlas_layout.clone(),
    //         index: 0,
    //     },
    //     TileAnimation::uniform(0, 2, 100),
    //     Name::new("StartMenuCursor"),
    // ));

    let cursor_y = 190. + main_menu.selected_index as f32 * 60.;
    let cursor_rect = Rect::from_center_size(Vec2::new(-180., cursor_y), Vec2::splat(48.));
    ctx.draw_image(
        cursor_rect,
        ui_res.cursor_image.clone(),
        bevy_keith::ImageScaling::Uniform(1.),
    );
}
