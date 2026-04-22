use audionimbus::Direction;
use bevy::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Clone)]
pub struct SharedDirection {
    x: Arc<AtomicU32>,
    y: Arc<AtomicU32>,
    z: Arc<AtomicU32>,
}

impl SharedDirection {
    pub fn new(direction: Vec3) -> Self {
        Self {
            x: Arc::new(AtomicU32::new(direction.x.to_bits())),
            y: Arc::new(AtomicU32::new(direction.y.to_bits())),
            z: Arc::new(AtomicU32::new(direction.z.to_bits())),
        }
    }

    pub fn load(&self) -> Direction {
        Direction::new(
            f32::from_bits(self.x.load(Ordering::Relaxed)),
            f32::from_bits(self.y.load(Ordering::Relaxed)),
            f32::from_bits(self.z.load(Ordering::Relaxed)),
        )
    }

    pub fn store(&self, direction: Vec3) {
        self.x.store(direction.x.to_bits(), Ordering::Relaxed);
        self.y.store(direction.y.to_bits(), Ordering::Relaxed);
        self.z.store(direction.z.to_bits(), Ordering::Relaxed);
    }
}
