//! Edit / Play mode switch for the editor.
//!
//! Entering play mode snapshots the current scene (and camera) to a
//! [`SceneFile`]; stopping restores that snapshot so edits made while
//! playing never leak back into the authored scene.

use shiloh_scene::{Camera, Scene, SceneFile};

/// Which mode the editor is currently in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorMode {
    #[default]
    Edit,
    Play,
}

/// Tracks play/edit transitions and the snapshot used to restore state.
#[derive(Debug, Default)]
pub struct PlaySession {
    pub mode: EditorMode,
    pub edit_snapshot: Option<SceneFile>,
}

impl PlaySession {
    pub fn is_playing(&self) -> bool {
        self.mode == EditorMode::Play
    }

    /// Snapshot the current scene/camera and switch to `Play`.
    ///
    /// No-op (besides overwriting the snapshot) if already playing.
    pub fn enter_play(&mut self, scene: &Scene, camera: Option<&Camera>) {
        self.edit_snapshot = Some(SceneFile::from_scene(scene, camera));
        self.mode = EditorMode::Play;
    }

    /// Restore the pre-play snapshot into `scene` and switch back to `Edit`.
    ///
    /// Returns the camera stored in the snapshot, if any. Does nothing (and
    /// returns `None`) if there was no snapshot to restore.
    pub fn exit_play(&mut self, scene: &mut Scene) -> Option<Camera> {
        self.mode = EditorMode::Edit;
        let snapshot = self.edit_snapshot.take()?;
        *scene = Scene::new(snapshot.name.clone());
        snapshot.apply_to_scene(scene)
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

        session.enter_play(&scene, Some(&camera));
        assert!(session.is_playing());

        // Simulate play-mode mutation that should not survive Stop.
        scene.spawn_transform(Transform::from_translation(Vec3::ZERO));
        assert_eq!(scene.world.entity_count(), 2);

        let restored_camera = session.exit_play(&mut scene);
        assert_eq!(session.mode, EditorMode::Edit);
        assert!(restored_camera.is_some());
        assert_eq!(scene.world.entity_count(), 1);
    }
}
