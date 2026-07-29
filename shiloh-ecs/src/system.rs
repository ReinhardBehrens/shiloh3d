//! Systems operate on a [`World`](crate::world::World).

use crate::world::World;

/// Object-safe system trait.
pub trait System: Send + Sync {
    fn name(&self) -> &str;
    fn run(&mut self, world: &mut World);
}

/// Wraps a closure as a system.
pub struct SystemFn<F> {
    name: &'static str,
    func: F,
}

impl<F> SystemFn<F>
where
    F: FnMut(&mut World) + Send + Sync,
{
    pub fn new(name: &'static str, func: F) -> Self {
        Self { name, func }
    }
}

impl<F> System for SystemFn<F>
where
    F: FnMut(&mut World) + Send + Sync,
{
    fn name(&self) -> &str {
        self.name
    }

    fn run(&mut self, world: &mut World) {
        (self.func)(world);
    }
}
