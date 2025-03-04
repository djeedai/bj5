// How to use this:
//   You should copy/paste this into your project and use it much like
// examples/tiles.rs uses this   file. When you do so you will need to adjust
// the code based on whether you're using the   'atlas` feature in
// bevy_ecs_tilemap. The bevy_ecs_tilemap uses this as an example of how to
//   use both single image tilesets and image collection tilesets. Since your
// project won't have   the 'atlas' feature defined in your Cargo config, the
// expressions prefixed by the #[cfg(...)]   macro will not compile in your
// project as-is. If your project depends on the bevy_ecs_tilemap
//   'atlas' feature then move all of the expressions prefixed by
// #[cfg(not(feature = "atlas"))].   Otherwise remove all of the expressions
// prefixed by #[cfg(feature = "atlas")].
//
// Functional limitations:
//   * When the 'atlas' feature is enabled tilesets using a collection of images
//     will be skipped.
//   * Only finite tile layers are loaded. Infinite tile layers and object
//     layers will be skipped.

use std::{
    io::{Cursor, ErrorKind},
    path::Path,
    sync::Arc,
};

use bevy::{
    asset::{io::Reader, AssetLoader, AssetPath, AsyncReadExt},
    core::Name,
    log,
    prelude::*,
    reflect::TypePath,
    utils::HashMap,
};
use bevy_ecs_tilemap::prelude::*;
use bevy_rapier2d::prelude::*;
use thiserror::Error;

use crate::{Damage, Epoch, EpochSprite, Ladder, LevelEnd, PlayerStart, Teleporter, TileAnimation};

#[derive(Default, Component)]
pub struct TileCollision;

#[derive(Default)]
pub struct TiledMapPlugin;

impl Plugin for TiledMapPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.init_asset::<TiledMap>()
            .register_asset_loader(TiledLoader)
            .add_systems(PreUpdate, (process_loaded_maps,));
    }
}

#[derive(TypePath, Asset)]
pub struct TiledMap {
    pub map: tiled::Map,

    pub tilemap_textures: HashMap<usize, TilemapTexture>,

    // The offset into the tileset_images for each tile id within each tileset.
    #[cfg(not(feature = "atlas"))]
    pub tile_image_offsets: HashMap<(usize, tiled::TileId), u32>,
}

// Stores a list of tiled layers.
#[derive(Component, Default)]
pub struct TiledLayersStorage {
    pub storage: HashMap<u32, Entity>,
}

#[derive(Default, Bundle)]
pub struct TiledMapBundle {
    pub tiled_map: Handle<TiledMap>,
    pub storage: TiledLayersStorage,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
    pub render_settings: TilemapRenderSettings,
}

struct BytesResourceReader {
    bytes: Arc<[u8]>,
}

impl BytesResourceReader {
    fn new(bytes: &[u8]) -> Self {
        Self {
            bytes: Arc::from(bytes),
        }
    }
}

impl tiled::ResourceReader for BytesResourceReader {
    type Resource = Cursor<Arc<[u8]>>;
    type Error = std::io::Error;

    fn read_from(&mut self, _path: &Path) -> std::result::Result<Self::Resource, Self::Error> {
        // In this case, the path is ignored because the byte data is already provided.
        Ok(Cursor::new(self.bytes.clone()))
    }
}

pub struct TiledLoader;

#[derive(Debug, Error)]
pub enum TiledAssetLoaderError {
    /// An [IO](std::io) Error
    #[error("Could not load Tiled file: {0}")]
    Io(#[from] std::io::Error),
}

impl AssetLoader for TiledLoader {
    type Asset = TiledMap;
    type Settings = ();
    type Error = TiledAssetLoaderError;

    async fn load<'a>(
        &'a self,
        reader: &'a mut Reader<'_>,
        _settings: &'a Self::Settings,
        load_context: &'a mut bevy::asset::LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        let mut loader = tiled::Loader::with_cache_and_reader(
            tiled::DefaultResourceCache::new(),
            BytesResourceReader::new(&bytes),
        );
        let map = loader.load_tmx_map(load_context.path()).map_err(|e| {
            std::io::Error::new(ErrorKind::Other, format!("Could not load TMX map: {e}"))
        })?;

        let mut tilemap_textures = HashMap::default();
        #[cfg(not(feature = "atlas"))]
        let mut tile_image_offsets = HashMap::default();

        for (tileset_index, tileset) in map.tilesets().iter().enumerate() {
            let tilemap_texture = match &tileset.image {
                None => {
                    #[cfg(feature = "atlas")]
                    {
                        log::info!("Skipping image collection tileset '{}' which is incompatible with atlas feature", tileset.name);
                        continue;
                    }

                    #[cfg(not(feature = "atlas"))]
                    {
                        let mut tile_images: Vec<Handle<Image>> = Vec::new();
                        for (tile_id, tile) in tileset.tiles() {
                            if let Some(img) = &tile.image {
                                // The load context path is the TMX file itself. If the file is at
                                // the root of the assets/ directory
                                // structure then the tmx_dir will be empty, which is fine.
                                let tmx_dir = load_context
                                    .path()
                                    .parent()
                                    .expect("The asset load context was empty.");
                                let tile_path = tmx_dir.join(&img.source);
                                let asset_path = AssetPath::from(tile_path);
                                log::info!("Loading tile image from {asset_path:?} as image ({tileset_index}, {tile_id})");
                                let texture: Handle<Image> = load_context.load(asset_path.clone());
                                tile_image_offsets
                                    .insert((tileset_index, tile_id), tile_images.len() as u32);
                                tile_images.push(texture.clone());
                            }
                        }

                        TilemapTexture::Vector(tile_images)
                    }
                }
                Some(img) => {
                    // The load context path is the TMX file itself. If the file is at the root of
                    // the assets/ directory structure then the tmx_dir will be
                    // empty, which is fine.
                    let tmx_dir = load_context
                        .path()
                        .parent()
                        .expect("The asset load context was empty.");
                    let tile_path = tmx_dir.join(&img.source);
                    let asset_path = AssetPath::from(tile_path);
                    let texture: Handle<Image> = load_context.load(asset_path.clone());

                    TilemapTexture::Single(texture.clone())
                }
            };

            tilemap_textures.insert(tileset_index, tilemap_texture);
        }

        let asset_map = TiledMap {
            map,
            tilemap_textures,
            #[cfg(not(feature = "atlas"))]
            tile_image_offsets,
        };

        log::info!("Loaded map: {}", load_context.path().display());
        Ok(asset_map)
    }

    fn extensions(&self) -> &[&str] {
        static EXTENSIONS: &[&str] = &["tmx"];
        EXTENSIONS
    }
}

fn get_teleporter_dst(obj: &tiled::Object) -> Option<u32> {
    let Some(dst) = obj.properties.get("dst") else {
        return None;
    };
    let tiled::PropertyValue::ObjectValue(other_id) = dst else {
        return None;
    };
    Some(*other_id)
}

fn get_int_prop(tile: &tiled::Tile, name: &str) -> Option<i32> {
    let Some(prop) = tile.properties.get(name) else {
        return None;
    };
    let tiled::PropertyValue::IntValue(value) = prop else {
        return None;
    };
    Some(*value)
}

fn get_float_prop(tile: &tiled::Tile, name: &str) -> Option<f32> {
    let Some(prop) = tile.properties.get(name) else {
        return None;
    };
    let tiled::PropertyValue::FloatValue(value) = prop else {
        return None;
    };
    Some(*value)
}

pub fn process_loaded_maps(
    mut commands: Commands,
    mut map_events: EventReader<AssetEvent<TiledMap>>,
    maps: Res<Assets<TiledMap>>,
    tile_storage_query: Query<(Entity, &TileStorage)>,
    mut map_query: Query<(
        &Handle<TiledMap>,
        &mut TiledLayersStorage,
        &TilemapRenderSettings,
    )>,
    new_maps: Query<&Handle<TiledMap>, Added<Handle<TiledMap>>>,
    mut q_epoch: Query<&mut Epoch>,
) {
    let mut changed_maps = Vec::<AssetId<TiledMap>>::default();
    for event in map_events.read() {
        match event {
            AssetEvent::Added { id } => {
                log::info!("Map added!");
                changed_maps.push(*id);
            }
            AssetEvent::Modified { id } => {
                log::info!("Map changed!");
                changed_maps.push(*id);
            }
            AssetEvent::Removed { id } => {
                log::info!("Map removed!");
                // if mesh was modified and removed in the same update, ignore the modification
                // events are ordered so future modification events are ok
                changed_maps.retain(|changed_handle| changed_handle == id);
            }
            _ => continue,
        }
    }

    // If we have new map entities add them to the changed_maps list.
    for new_map_handle in new_maps.iter() {
        changed_maps.push(new_map_handle.id());
    }

    let mut epoch = q_epoch.single_mut();
    let mut min_epoch = epoch.min;
    let mut max_epoch = epoch.max;
    let mut epoch_change = false;

    for changed_map in changed_maps.iter() {
        for (map_handle, mut layer_storage, render_settings) in map_query.iter_mut() {
            // only deal with currently changed map
            if map_handle.id() != *changed_map {
                continue;
            }

            let Some(tiled_map) = maps.get(map_handle) else {
                debug!(
                    "Ignoring change to invalid Tiled map handle {:?}",
                    map_handle
                );
                continue;
            };

            // TODO: Create a RemoveMap component..
            for layer_entity in layer_storage.storage.values() {
                if let Ok((_, layer_tile_storage)) = tile_storage_query.get(*layer_entity) {
                    for tile in layer_tile_storage.iter().flatten() {
                        commands.entity(*tile).despawn_recursive()
                    }
                }
                // commands.entity(*layer_entity).despawn_recursive();
            }

            let map_size = TilemapSize {
                x: tiled_map.map.width,
                y: tiled_map.map.height,
            };

            let grid_size = TilemapGridSize {
                x: tiled_map.map.tile_width as f32,
                y: tiled_map.map.tile_height as f32,
            };

            // The TilemapBundle requires that all tile images come exclusively from a
            // single tiled texture or from a Vec of independent per-tile
            // images. Furthermore, all of the per-tile images must be the same
            // size. Since Tiled allows tiles of mixed tilesets on each layer
            // and allows differently-sized tile images in each tileset,
            // this means we need to load each combination of tileset and layer separately.
            for (tileset_index, tileset) in tiled_map.map.tilesets().iter().enumerate() {
                let Some(tilemap_texture) = tiled_map.tilemap_textures.get(&tileset_index) else {
                    warn!(
                        "Skipped creating tileset #{tileset_index} with missing tilemap texture."
                    );
                    continue;
                };

                let tile_size = TilemapTileSize {
                    x: tileset.tile_width as f32,
                    y: tileset.tile_height as f32,
                };

                let tile_spacing = TilemapSpacing {
                    x: tileset.spacing as f32,
                    y: tileset.spacing as f32,
                };

                // Once materials have been created/added we need to then create the layers.
                for (layer_index, layer) in tiled_map.map.layers().enumerate() {
                    // Only process tile layers here; other types of layers don't need the double
                    // loop on tilesets, and are done separately below.
                    let tiled::LayerType::Tiles(tile_layer) = layer.layer_type() else {
                        continue;
                    };

                    let offset_x = layer.offset_x;
                    let offset_y = layer.offset_y;

                    trace!(
                        "Processing layer #{} '{}' at offset {}x{}...",
                        layer_index,
                        layer.name,
                        offset_x,
                        offset_y
                    );

                    let tiled::TileLayer::Finite(layer_data) = tile_layer else {
                        info!(
                            "Skipping layer {} because only finite layers are supported.",
                            layer.id()
                        );
                        continue;
                    };

                    let map_type = match tiled_map.map.orientation {
                        tiled::Orientation::Hexagonal => TilemapType::Hexagon(HexCoordSystem::Row),
                        tiled::Orientation::Isometric => {
                            TilemapType::Isometric(IsoCoordSystem::Diamond)
                        }
                        tiled::Orientation::Staggered => {
                            TilemapType::Isometric(IsoCoordSystem::Staggered)
                        }
                        tiled::Orientation::Orthogonal => TilemapType::Square,
                    };

                    let mut tile_storage = TileStorage::empty(map_size);
                    let layer_entity = commands.spawn_empty().id();

                    let is_wall = layer.name == "Walls";
                    let layer_transform =
                                    // get_tilemap_center_transform(
                                    //     &map_size,
                                    //     &grid_size,
                                    //     &map_type,
                                    //     layer_index as f32,
                                    // ) * 
                                    Transform::from_xyz(offset_x, -offset_y, layer_index as f32);

                    for x in 0..map_size.x {
                        for y in 0..map_size.y {
                            // Transform TMX coords into bevy coords.
                            let mapped_y = tiled_map.map.height - 1 - y;

                            let mapped_x = x as i32;
                            let mapped_y = mapped_y as i32;

                            let Some(layer_tile) = layer_data.get_tile(mapped_x, mapped_y) else {
                                continue;
                            };

                            if tileset_index != layer_tile.tileset_index() {
                                continue;
                            }

                            let Some(layer_tile_data) =
                                layer_data.get_tile_data(mapped_x, mapped_y)
                            else {
                                continue;
                            };

                            let tile_id = layer_tile_data.id();
                            let Some(tile) = tileset.get_tile(tile_id) else {
                                continue;
                            };

                            let epoch = get_int_prop(&tile, "epoch");
                            let epoch_min = get_int_prop(&tile, "epoch_min");
                            let epoch_max = get_int_prop(&tile, "epoch_max");

                            let texture_index = match tilemap_texture {
                                            TilemapTexture::Single(_) => layer_tile.id(),
                                            #[cfg(not(feature = "atlas"))]
                                            TilemapTexture::Vector(_) =>
                                                *tiled_map.tile_image_offsets.get(&(tileset_index, layer_tile.id()))
                                                .expect("The offset into to image vector should have been saved during the initial load."),
                                            #[cfg(not(feature = "atlas"))]
                                            _ => unreachable!()
                                        };

                            let (epoch_sprite, is_visible) = if let Some(epoch_id) = epoch {
                                let min0 = epoch_min.unwrap_or(epoch_id);
                                let max0 = epoch_max.unwrap_or(epoch_id);
                                let min = min0.min(max0);
                                let max = max0.max(min0);

                                min_epoch = min_epoch.min(min - epoch_id);
                                max_epoch = max_epoch.max(max - epoch_id);
                                epoch_change = true;

                                let epoch_id = epoch_id.clamp(min, max);
                                let epoch_sprite = EpochSprite {
                                    base: tile_id as usize - (epoch_id - min) as usize,
                                    delta: epoch_id,
                                    first: min,
                                    last: max,
                                };
                                trace!(
                                    "EpochSprite: min={} max={} delta=epoch={} base={}",
                                    min,
                                    max,
                                    epoch_id,
                                    epoch_sprite.base
                                );
                                (Some(epoch_sprite), true)
                            } else {
                                (None, true)
                            };

                            // Tile animation
                            let tile_anim = tile.animation.as_ref().map(|frames| TileAnimation {
                                frames: frames.clone(),
                                index: rand::random::<u32>() % frames.len() as u32,
                                clock: rand::random::<u32>() % 1000,
                            });

                            let tile_pos = TilePos { x, y };

                            let mut ent_cmds = commands.spawn(TileBundle {
                                position: tile_pos,
                                tilemap_id: TilemapId(layer_entity),
                                texture_index: TileTextureIndex(texture_index),
                                flip: TileFlip {
                                    x: layer_tile_data.flip_h,
                                    y: layer_tile_data.flip_v,
                                    d: layer_tile_data.flip_d,
                                },
                                visible: TileVisible(is_visible),
                                ..Default::default()
                            });
                            if let Some(epoch_sprite) = epoch_sprite {
                                ent_cmds.insert(epoch_sprite);
                            }
                            if let Some(tile_anim) = tile_anim {
                                debug!(
                                    "Tile anim #{}: {}#{}, ...",
                                    tile_anim.frames.len(),
                                    tile_anim.frames[0].tile_id,
                                    tile_anim.frames[0].duration
                                );
                                ent_cmds.insert(tile_anim);
                            }

                            let tile_entity = ent_cmds.id();
                            tile_storage.set(&tile_pos, tile_entity);

                            // Damage-inducing tile
                            if let Some(damage) = get_float_prop(&tile, "damage") {
                                if let Some(obj_data) = &tile.collision {
                                    for data in obj_data.object_data() {
                                        if data.user_type == "collider" {
                                            if let tiled::ObjectShape::Rect { width, height } =
                                                data.shape
                                            {
                                                let tile_pos: Vec2 = tile_pos.into();
                                                let grid_size: Vec2 = grid_size.into();
                                                let tile_pos2: Vec2 = tile_pos * grid_size
                                                    + Vec2::new(
                                                        layer_transform.translation.x,
                                                        layer_transform.translation.y,
                                                    );

                                                commands.spawn((
                                                    TileCollision,
                                                    Transform::from_xyz(
                                                        tile_pos2.x + data.x,
                                                        tile_pos2.y + grid_size.y / 2.
                                                            - data.y
                                                            - height / 2.,
                                                        0.,
                                                    ),
                                                    GlobalTransform::default(),
                                                    RigidBody::Fixed,
                                                    Sensor,
                                                    Collider::cuboid(width / 2., height / 2.),
                                                    Damage(damage),
                                                    Name::new(format!("dmg{}x{}", x, y)),
                                                ));
                                            }
                                        }
                                    }
                                }
                            }

                            // Static world collider tile
                            if is_wall {
                                let tile_pos: Vec2 = tile_pos.into();
                                let grid_size: Vec2 = grid_size.into();
                                let tile_pos2: Vec2 = tile_pos * grid_size
                                    + Vec2::new(
                                        layer_transform.translation.x,
                                        layer_transform.translation.y,
                                    );
                                // trace!(
                                //     "tile_pos={:?} grid_size={:?} tile_pos2={:?}",
                                //     tile_pos,
                                //     grid_size,
                                //     tile_pos2
                                // );
                                commands.spawn((
                                    TileCollision,
                                    Transform::from_xyz(tile_pos2.x, tile_pos2.y, 0.),
                                    GlobalTransform::default(),
                                    RigidBody::Fixed,
                                    Collider::cuboid(8., 8.),
                                    Name::new(format!("tile{}x{}", x, y)),
                                ));
                            }
                        }
                    }

                    commands.entity(layer_entity).insert(TilemapBundle {
                        grid_size,
                        size: map_size,
                        storage: tile_storage,
                        texture: tilemap_texture.clone(),
                        tile_size,
                        spacing: tile_spacing,
                        transform: layer_transform,
                        map_type,
                        render_settings: *render_settings,
                        ..Default::default()
                    });

                    layer_storage
                        .storage
                        .insert(layer_index as u32, layer_entity);
                }
            }

            // Process object layers (once only)
            let mut tp_map = HashMap::new();
            for (layer_index, layer) in tiled_map.map.layers().enumerate() {
                let tiled::LayerType::Objects(object_layer) = layer.layer_type() else {
                    continue;
                };

                for obj in object_layer.objects() {
                    trace!("Object: {} #{}", obj.name, obj.user_type);

                    let x = obj.x - grid_size.x / 2.;
                    let y = map_size.y as f32 * grid_size.y - obj.y - grid_size.y / 2.;
                    let position = Vec2::new(x, y).extend(layer_index as f32);

                    if obj.user_type == "player_start" {
                        commands.spawn((PlayerStart { position }, Name::new(obj.name.clone())));
                    } else if obj.user_type == "teleport" {
                        let tiled::ObjectShape::Rect { width, height } = &obj.shape else {
                            continue;
                        };

                        let offset = Vec3::new(width / 2., -height / 2., 0.);
                        let Some(dst_id) = get_teleporter_dst(&obj) else {
                            warn!("Teleporter #{} is missing a 'dst' property.", obj.id());
                            continue;
                        };
                        let entity = commands
                            .spawn((
                                TransformBundle::from(Transform::from_translation(
                                    position + offset,
                                )),
                                Collider::cuboid(width / 2., height / 2.),
                                Sensor,
                                Name::new(obj.name.clone()),
                            ))
                            .id();
                        trace!(
                            "Spawned teleporter #{} '{}' entity {:?} at {:?} ({:?} + {:?}) -> {}",
                            obj.id(),
                            obj.name,
                            entity,
                            position + offset,
                            position,
                            offset,
                            dst_id,
                        );
                        tp_map.insert(obj.id(), (entity, dst_id));
                    } else if obj.user_type == "ladder" {
                        let tiled::ObjectShape::Rect { width, height } = &obj.shape else {
                            continue;
                        };

                        let offset = Vec3::new(width / 2., -height / 2., 0.);
                        commands.spawn((
                            TransformBundle::from(Transform::from_translation(position + offset)),
                            Collider::cuboid(width / 2., height / 2.),
                            Sensor,
                            Ladder,
                            Name::new(obj.name.clone()),
                        ));
                    } else if obj.user_type == "level_end" {
                        let tiled::ObjectShape::Rect { width, height } = &obj.shape else {
                            continue;
                        };

                        let offset = Vec3::new(width / 2., -height / 2., 0.);
                        commands.spawn((
                            TransformBundle::from(Transform::from_translation(position + offset)),
                            Collider::cuboid(width / 2., height / 2.),
                            Sensor,
                            LevelEnd,
                            Name::new(obj.name.clone()),
                        ));
                    } else {
                        debug!(
                            "Ignoring unknown object '{}' of class '{}'",
                            obj.name, obj.user_type
                        );
                    }
                }
            }

            // Resolve teleporters once all entities are created, and insert the Teleporter
            // component with a link to the destination entity.
            for (id, (entity, dst_id)) in &tp_map {
                if let Some((dst_entity, src_id)) = tp_map.get(dst_id) {
                    assert_eq!(*src_id, *id);
                    info!(
                        "Adding teleporter to entity {:?} -> {:?}",
                        entity, dst_entity
                    );
                    commands
                        .entity(*entity)
                        .insert(Teleporter::new(*dst_entity));
                } else {
                    warn!("Teleporter #{} has unknown destination #{}", id, *dst_id);
                }
            }
        }
    }

    if epoch_change {
        info!("Loaded map with epoch({}:{})", min_epoch, max_epoch);
        epoch.min = min_epoch;
        epoch.max = max_epoch;
    }
}
