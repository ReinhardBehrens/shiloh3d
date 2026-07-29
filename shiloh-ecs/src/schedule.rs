//! Ordered stages of systems (Startup → PreUpdate → Update → PostUpdate → Render).

use crate::system::System;
use crate::world::World;

/// Named pipeline stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stage {
    Startup,
    PreUpdate,
    Update,
    PostUpdate,
    FixedUpdate,
    Render,
}

/// Ordered schedule of stages and systems.
#[derive(Default)]
pub struct Schedule {
    startup: Vec<Box<dyn System>>,
    pre_update: Vec<Box<dyn System>>,
    update: Vec<Box<dyn System>>,
    post_update: Vec<Box<dyn System>>,
    fixed_update: Vec<Box<dyn System>>,
    render: Vec<Box<dyn System>>,
    startup_done: bool,
}

impl Schedule {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_system(&mut self, stage: Stage, system: impl System + 'static) -> &mut Self {
        let boxed: Box<dyn System> = Box::new(system);
        match stage {
            Stage::Startup => self.startup.push(boxed),
            Stage::PreUpdate => self.pre_update.push(boxed),
            Stage::Update => self.update.push(boxed),
            Stage::PostUpdate => self.post_update.push(boxed),
            Stage::FixedUpdate => self.fixed_update.push(boxed),
            Stage::Render => self.render.push(boxed),
        }
        self
    }

    pub fn run(&mut self, world: &mut World) {
        if !self.startup_done {
            for system in &mut self.startup {
                system.run(world);
            }
            self.startup_done = true;
        }
        for system in &mut self.pre_update {
            system.run(world);
        }
        for system in &mut self.update {
            system.run(world);
        }
        for system in &mut self.post_update {
            system.run(world);
        }
    }

    pub fn run_fixed(&mut self, world: &mut World, steps: u32) {
        for _ in 0..steps {
            for system in &mut self.fixed_update {
                system.run(world);
            }
        }
    }

    pub fn run_render(&mut self, world: &mut World) {
        for system in &mut self.render {
            system.run(world);
        }
    }
}
