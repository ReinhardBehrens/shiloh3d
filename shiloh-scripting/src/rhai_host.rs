//! Rhai host — Godot-style `on_ready` / `on_update` sandbox (Phase 5).
//!
//! Scripts call a small safe API; side effects are collected as [`ScriptCommand`]s
//! for the engine to apply outside the interpreter.

use std::path::Path;
use std::sync::{Arc, Mutex};

use rhai::{AST, Engine, Scope};

/// Side-effect commands produced by a sandboxed script.
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptCommand {
    Log(String),
    SetTranslation {
        entity_index: i64,
        x: f64,
        y: f64,
        z: f64,
    },
    SpawnNamed {
        name: String,
        x: f64,
        y: f64,
        z: f64,
    },
    EmitSignal {
        name: String,
    },
    PlayAudio {
        name: String,
    },
}

/// Errors from compiling or loading a Rhai script.
#[derive(Debug, thiserror::Error)]
pub enum RhaiHostError {
    #[error("rhai: {0}")]
    Rhai(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl From<Box<rhai::EvalAltResult>> for RhaiHostError {
    fn from(err: Box<rhai::EvalAltResult>) -> Self {
        Self::Rhai(err.to_string())
    }
}

impl From<rhai::ParseError> for RhaiHostError {
    fn from(err: rhai::ParseError) -> Self {
        Self::Rhai(err.to_string())
    }
}

/// Embedded Rhai engine with a Godot-like ready/update lifecycle.
pub struct RhaiHost {
    engine: Engine,
    ast: Option<AST>,
    scope: Scope<'static>,
    commands: Arc<Mutex<Vec<ScriptCommand>>>,
}

impl Default for RhaiHost {
    fn default() -> Self {
        Self::new()
    }
}

impl RhaiHost {
    pub fn new() -> Self {
        let commands = Arc::new(Mutex::new(Vec::new()));
        let mut engine = Engine::new();

        // Keep the sandbox tight: no file/network packages; cap runaway loops.
        engine.set_max_expr_depths(64, 64);
        engine.set_max_operations(100_000);

        Self::register_api(&mut engine, &commands);

        Self {
            engine,
            ast: None,
            scope: Scope::new(),
            commands,
        }
    }

    fn register_api(engine: &mut Engine, commands: &Arc<Mutex<Vec<ScriptCommand>>>) {
        let push_log = |commands: &Arc<Mutex<Vec<ScriptCommand>>>, msg: String| {
            if let Ok(mut guard) = commands.lock() {
                guard.push(ScriptCommand::Log(msg));
            }
        };

        {
            let commands = Arc::clone(commands);
            engine.register_fn("log", move |msg: String| {
                push_log(&commands, msg);
            });
        }
        {
            let commands = Arc::clone(commands);
            // Avoid clashing with Rhai's built-in `print` — expose `say` + keep `log`.
            engine.register_fn("say", move |msg: String| {
                push_log(&commands, msg);
            });
        }
        {
            let commands = Arc::clone(commands);
            engine.register_fn(
                "set_translation",
                move |entity_index: i64, x: f64, y: f64, z: f64| {
                    if let Ok(mut guard) = commands.lock() {
                        guard.push(ScriptCommand::SetTranslation {
                            entity_index,
                            x,
                            y,
                            z,
                        });
                    }
                },
            );
        }
        {
            let commands = Arc::clone(commands);
            engine.register_fn("spawn_named", move |name: String, x: f64, y: f64, z: f64| {
                if let Ok(mut guard) = commands.lock() {
                    guard.push(ScriptCommand::SpawnNamed {
                        name,
                        x,
                        y,
                        z,
                    });
                }
            });
        }
        {
            let commands = Arc::clone(commands);
            engine.register_fn("emit_signal", move |name: String| {
                if let Ok(mut guard) = commands.lock() {
                    guard.push(ScriptCommand::EmitSignal { name });
                }
            });
        }
        {
            let commands = Arc::clone(commands);
            engine.register_fn("play_audio", move |name: String| {
                if let Ok(mut guard) = commands.lock() {
                    guard.push(ScriptCommand::PlayAudio { name });
                }
            });
        }
    }

    /// Compile a `.rhai` source string into the host AST.
    pub fn load_str(&mut self, source: &str) -> Result<(), RhaiHostError> {
        self.ast = Some(self.engine.compile(source)?);
        self.scope = Scope::new();
        Ok(())
    }

    /// Compile a `.rhai` file into the host AST.
    pub fn load_file(&mut self, path: impl AsRef<Path>) -> Result<(), RhaiHostError> {
        let source = std::fs::read_to_string(path)?;
        self.load_str(&source)
    }

    fn has_fn(ast: &AST, name: &str) -> bool {
        ast.iter_functions().any(|f| f.name == name)
    }

    fn take_commands(&self) -> Vec<ScriptCommand> {
        self.commands
            .lock()
            .map(|mut guard| std::mem::take(&mut *guard))
            .unwrap_or_default()
    }

    fn clear_commands(&self) {
        if let Ok(mut guard) = self.commands.lock() {
            guard.clear();
        }
    }

    /// Call `on_ready` if defined; return queued commands.
    pub fn run_ready(&mut self) -> Vec<ScriptCommand> {
        self.clear_commands();
        if let Some(ast) = &self.ast {
            if Self::has_fn(ast, "on_ready") {
                let result: Result<(), _> =
                    self.engine.call_fn(&mut self.scope, ast, "on_ready", ());
                if let Err(err) = result {
                    tracing::warn!(error = %err, "rhai on_ready failed");
                    if let Ok(mut guard) = self.commands.lock() {
                        guard.push(ScriptCommand::Log(format!("on_ready error: {err}")));
                    }
                }
            }
        }
        self.take_commands()
    }

    /// Call `on_update(dt)` if defined; return queued commands.
    pub fn run_update(&mut self, dt: f32) -> Vec<ScriptCommand> {
        self.clear_commands();
        if let Some(ast) = &self.ast {
            if Self::has_fn(ast, "on_update") {
                let result: Result<(), _> =
                    self.engine
                        .call_fn(&mut self.scope, ast, "on_update", (f64::from(dt),));
                if let Err(err) = result {
                    tracing::warn!(error = %err, "rhai on_update failed");
                    if let Ok(mut guard) = self.commands.lock() {
                        guard.push(ScriptCommand::Log(format!("on_update error: {err}")));
                    }
                }
            }
        }
        self.take_commands()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_ready_logs() {
        let mut host = RhaiHost::new();
        host.load_str(
            r#"
            fn on_ready() {
                log("hello from rhai");
            }
            "#,
        )
        .expect("compile");
        let cmds = host.run_ready();
        assert_eq!(
            cmds,
            vec![ScriptCommand::Log("hello from rhai".into())]
        );
    }

    #[test]
    fn on_update_emits_stubs() {
        let mut host = RhaiHost::new();
        host.load_str(
            r#"
            fn on_update(dt) {
                log("tick");
                spawn_named("crate", 1.0, 2.0, 3.0);
                set_translation(0, 4.0, 5.0, 6.0);
                emit_signal("jumped");
                play_audio("jump");
            }
            "#,
        )
        .expect("compile");
        let cmds = host.run_update(1.0 / 60.0);
        assert_eq!(
            cmds,
            vec![
                ScriptCommand::Log("tick".into()),
                ScriptCommand::SpawnNamed {
                    name: "crate".into(),
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                },
                ScriptCommand::SetTranslation {
                    entity_index: 0,
                    x: 4.0,
                    y: 5.0,
                    z: 6.0,
                },
                ScriptCommand::EmitSignal {
                    name: "jumped".into(),
                },
                ScriptCommand::PlayAudio {
                    name: "jump".into(),
                },
            ]
        );
    }

    #[test]
    fn missing_hooks_yield_empty() {
        let mut host = RhaiHost::new();
        host.load_str("fn helper() { log(\"x\"); }")
            .expect("compile");
        assert!(host.run_ready().is_empty());
        assert!(host.run_update(0.016).is_empty());
    }
}
