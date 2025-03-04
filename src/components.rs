use std::time::Duration;

use bevy::prelude::*;

#[derive(Default, Component)]
pub struct MainCamera {}

#[derive(Default, Component)]
pub struct PlayerStart {
    pub position: Vec3,
}

#[derive(Component)]
pub struct Teleporter {
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
pub struct Player {
    pub impulse_factor: f32,
    /// Side from which the player entered the last teleporter, to determine if
    /// it exited on the opposite side and therefore if teleportation is needed.
    pub teleporter_side: f32,
    pub life: f32,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            impulse_factor: 500.,
            teleporter_side: 0.,
            life: 20.,
        }
    }
}

#[derive(Default, Component)]
pub struct PlayerController {
    pub is_grounded: bool,
    pub is_climbing: bool,
}

#[derive(Component)]
pub struct PlayerLife {
    pub life: f32,
    pub max_life: f32,
    pub last_dmg_time: Option<Duration>,
    pub last_dmg_dir: Vec2,
}

impl Default for PlayerLife {
    fn default() -> Self {
        Self {
            life: 20.,
            max_life: 20.,
            last_dmg_time: None,
            last_dmg_dir: Vec2::ZERO,
        }
    }
}

impl PlayerLife {
    pub const DAMAGE_DURATION: Duration = Duration::from_millis(400);

    pub fn damage(&mut self, time: Duration, amount: f32, dir: Vec2) {
        self.life = (self.life - amount).max(0.);
        self.last_dmg_time = Some(time);
        self.last_dmg_dir = dir;
    }

    pub fn damage_impulse_factor(&self, time: Duration) -> Option<f32> {
        if let Some(last_dmg_time) = self.last_dmg_time {
            if time >= last_dmg_time {
                let delta = time - last_dmg_time;
                if delta <= Self::DAMAGE_DURATION {
                    let x = delta.div_duration_f32(Self::DAMAGE_DURATION).clamp(0., 1.);
                    let x2 = (1. - x) * (1. - x);
                    let ratio = 1. - x2 * x2;
                    Some(ratio.clamp(0., 1.))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    }
}

#[derive(Default, Component)]
pub struct TileAnimation {
    pub frames: Vec<tiled::Frame>,
    pub index: u32,
    pub clock: u32,
}

impl TileAnimation {
    pub fn uniform(start: u32, count: u32, duration: u32) -> Self {
        Self {
            frames: (start..start + count)
                .map(|idx| tiled::Frame {
                    tile_id: idx,
                    duration,
                })
                .collect(),
            ..default()
        }
    }

    pub fn tick(&mut self, dt: u32) -> tiled::TileId {
        self.clock += dt;
        let mut dur = self.frames[self.index as usize].duration;
        if self.clock > dur {
            self.clock -= dur;
            let len = self.frames.len() as u32;
            self.index = (self.index + 1) % len;
            dur = self.frames[self.index as usize].duration;
            while self.clock > dur {
                self.clock -= dur;
                self.index = (self.index + 1) % len;
                dur = self.frames[self.index as usize].duration;
            }
        }
        self.frames[self.index as usize].tile_id
    }
}

#[derive(Default, Component)]
pub struct Epoch {
    pub min: i32,
    pub max: i32,
    pub cur: i32,
}

#[derive(Default, Component)]
pub struct EpochSprite {
    /// Base tile index to add to `first` and `last` to convert an epoch into a
    /// tile ID.
    pub base: usize,
    /// Initial epoch delta at start.
    pub delta: i32,
    /// First epoch the sprite is available at.
    pub first: i32,
    /// Last epoch the sprite is available at.
    pub last: i32,
}

#[derive(Component)]
pub struct Damage(pub f32);

#[derive(Default, Component)]
pub struct Ladder;

#[derive(Default, Component)]
pub struct LevelEnd;
