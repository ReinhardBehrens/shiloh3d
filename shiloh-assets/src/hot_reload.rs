//! Hot-reload watcher (feature `hot-reload`).

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc;

pub struct HotReloader {
    _watcher: RecommendedWatcher,
    rx: mpsc::Receiver<notify::Result<notify::Event>>,
}

impl HotReloader {
    pub fn watch(root: impl AsRef<Path>) -> notify::Result<Self> {
        let (tx, rx) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(tx)?;
        watcher.watch(root.as_ref(), RecursiveMode::Recursive)?;
        Ok(Self {
            _watcher: watcher,
            rx,
        })
    }

    pub fn poll_changed(&self) -> Vec<std::path::PathBuf> {
        let mut out = Vec::new();
        while let Ok(Ok(event)) = self.rx.try_recv() {
            for path in event.paths {
                out.push(path);
            }
        }
        out
    }
}
