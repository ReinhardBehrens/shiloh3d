//! Edit / Play mode switch for the editor.
//!
//! Entering play mode snapshots the current scene (and camera) to a
//! [`SceneFile`]; stopping restores that snapshot so edits made while
//! playing never leak back into the authored scene.
//!
//! Play also drives [`RhaiHost`] Godot-style `on_ready` / `on_update` hooks
//! when a project script is loaded (Phase Compete / Phase 5).

use shiloh_scene::{Camera, Scene, SceneFile};
use shiloh_scripting::{RhaiHost, ScriptCommand};

/// Which mode the editor is currently in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorMode {
    #[default]
    Edit,
    Play,
}

/// Tracks play/edit transitions and the snapshot used to restore state.
pub struct PlaySession {
    pub mode: EditorMode,
    pub edit_snapshot: Option<SceneFile>,
    /// Sandboxed Rhai host — loaded on enter_play when a script path exists.
    pub rhai: RhaiHost,
    /// Whether `on_ready` has already run for the current play session.
    ready_ran: bool,
}

impl Default for PlaySession {
    fn default() -> Self {
        Self {
            mode: EditorMode::Edit,
            edit_snapshot: None,
            rhai: RhaiHost::new(),
            ready_ran: false,
        }
    }
}

impl PlaySession {
    pub fn is_playing(&self) -> bool {
        self.mode == EditorMode::Play
    }

    /// Snapshot the current scene/camera and switch to `Play`.
    ///
    /// Optionally compile `script_source` (`.rhai` text) before `on_ready`.
    pub fn enter_play(
        &mut self,
        scene: &Scene,
        camera: Option<&Camera>,
        script_source: Option<&str>,
    ) {
        self.edit_snapshot = Some(SceneFile::from_scene(scene, camera));
        self.mode = EditorMode::Play;
        self.ready_ran = false;
        self.rhai = RhaiHost::new();
        if let Some(src) = script_source {
            if let Err(err) = self.rhai.load_str(src) {
                tracing::warn!(error = %err, "play: failed to load rhai script");
            }
        }
    }

    /// Restore the pre-play snapshot into `scene` and switch back to `Edit`.
    ///
    /// Returns the camera stored in the snapshot, if any. Does nothing (and
    /// returns `None`) if there was no snapshot to restore.
    pub fn exit_play(&mut self, scene: &mut Scene) -> Option<Camera> {
        self.mode = EditorMode::Edit;
        self.ready_ran = false;
        self.rhai = RhaiHost::new();
        let snapshot = self.edit_snapshot.take()?;
        *scene = Scene::new(snapshot.name.clone());
        snapshot.apply_to_scene(scene)
    }

    /// Tick play scripting: first call runs `on_ready`, then `on_update(dt)`.
    pub fn tick(&mut self, dt: f32) -> Vec<ScriptCommand> {
        if !self.is_playing() {
            return Vec::new();
        }
        if !self.ready_ran {
            self.ready_ran = true;
            let mut cmds = self.rhai.run_ready();
            cmds.extend(self.rhai.run_update(dt));
            return cmds;
        }
        self.rhai.run_update(dt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;
    use shiloh_scene::Transform;

    #[test]
    fn play_then_stop_restores_snapshot() {
        let mut scene = Scene::new("test");
        scene.spawn_transform(Transform::from_translation(Vec3::new(1.0, 2.0, 3.0)));
        let camera = Camera::default();

        let mut session = PlaySession::default();
        assert_eq!(session.mode, EditorMode::Edit);

        session.enter_play(&scene, Some(&camera), None);
        assert!(session.is_playing());

        // Simulate play-mode mutation that should not survive Stop.
        scene.spawn_transform(Transform::from_translation(Vec3::ZERO));
        assert_eq!(scene.world.entity_count(), 2);

        let restored_camera = session.exit_play(&mut scene);
        assert_eq!(session.mode, EditorMode::Edit);
        assert!(restored_camera.is_some());
        assert_eq!(scene.world.entity_count(), 1);
    }

    #[test]
    fn play_runs_rhai_on_ready() {
        let mut scene = Scene::new("test");
        let mut session = PlaySession::default();
        let src = r#"
            fn on_ready() {
                log("hello play");
            }
            fn on_update(dt) {}
        "#;
        session.enter_play(&scene, None, Some(src));
        let cmds = session.tick(0.016);
        assert!(
            cmds
                .iter()
                .any(|c| matches!(c, ScriptCommand::Log(m) if m.contains("hello"))),
            "expected log from on_ready, got {cmds:?}"
        );
        let _ = session.exit_play(&mut scene);
    }
}
