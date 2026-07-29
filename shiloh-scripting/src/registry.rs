//! Registry of loaded script modules.

use crate::module::{ScriptContext, ScriptModule};

#[derive(Default)]
pub struct ScriptRegistry {
    modules: Vec<Box<dyn ScriptModule>>,
}

impl ScriptRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, module: impl ScriptModule + 'static) {
        self.modules.push(Box::new(module));
    }

    pub fn update_all(&mut self, ctx: &mut ScriptContext<'_>) {
        for module in &mut self.modules {
            module.on_update(ctx);
        }
    }
}
