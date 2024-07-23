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
}

impl Default for Player {
    fn default() -> Self {
        Self {
            impulse_factor: 500.,
            teleporter_side: 0.,
        }
    }
}

#[derive(Component)]
pub struct AnimationIndices {
    pub first: usize,
    pub last: usize,
}

#[derive(Component, Deref, DerefMut)]
pub struct AnimationTimer(pub Timer);

#[derive(Default, Component)]
pub struct Epoch(pub i32);

#[derive(Default, Component)]
pub struct EpochSprite {
    pub first: usize,
    pub last: usize,
}
