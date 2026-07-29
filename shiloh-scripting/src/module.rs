//! Native Rust script module trait.

use shiloh_ecs::World;
use shiloh_scene::Scene;

pub struct ScriptContext<'a> {
    pub world: &'a mut World,
    pub scene_name: &'a str,
    pub delta_seconds: f32,
}

pub trait ScriptModule: Send + Sync {
    fn name(&self) -> &str;
    fn on_load(&mut self, _ctx: &mut ScriptContext<'_>) {}
    fn on_update(&mut self, ctx: &mut ScriptContext<'_>);
    fn on_unload(&mut self, _ctx: &mut ScriptContext<'_>) {}
}

/// Helper to build a context from a scene.
pub fn context_from_scene<'a>(scene: &'a mut Scene, delta_seconds: f32) -> ScriptContext<'a> {
    ScriptContext {
        world: &mut scene.world,
        scene_name: &scene.name,
        delta_seconds,
    }
}
