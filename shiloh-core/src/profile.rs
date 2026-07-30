//! Lightweight CPU frame profiler + crash hook (Phase 3 packaging hooks).

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Once;
use std::time::Instant;

thread_local! {
    static SCOPES: RefCell<Vec<(String, Instant)>> = RefCell::new(Vec::new());
    static TOTALS: RefCell<HashMap<String, ScopeStats>> = RefCell::new(HashMap::new());
}

#[derive(Debug, Clone, Default)]
pub struct ScopeStats {
    pub calls: u64,
    pub total_ns: u64,
    pub max_ns: u64,
}

/// RAII scope timer — records into the thread-local profiler.
pub struct ProfileScope {
    name: String,
    start: Instant,
}

impl ProfileScope {
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        SCOPES.with(|s| s.borrow_mut().push((name.clone(), Instant::now())));
        Self {
            name,
            start: Instant::now(),
        }
    }
}

impl Drop for ProfileScope {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed().as_nanos() as u64;
        TOTALS.with(|t| {
            let mut map = t.borrow_mut();
            let entry = map.entry(self.name.clone()).or_default();
            entry.calls += 1;
            entry.total_ns += elapsed;
            entry.max_ns = entry.max_ns.max(elapsed);
        });
        SCOPES.with(|s| {
            let _ = s.borrow_mut().pop();
        });
    }
}

/// Snapshot of accumulated scope stats (clears if `reset`).
pub fn snapshot(reset: bool) -> HashMap<String, ScopeStats> {
    TOTALS.with(|t| {
        let mut map = t.borrow_mut();
        let out = map.clone();
        if reset {
            map.clear();
        }
        out
    })
}

/// Install a panic hook that logs to stderr and optionally writes a crash file.
pub fn install_crash_hook(crash_dir: Option<std::path::PathBuf>) {
    static ONCE: Once = Once::new();
    ONCE.call_once(move || {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let msg = format!("Shiloh3D crash: {info}");
            eprintln!("{msg}");
            if let Some(dir) = &crash_dir {
                let _ = std::fs::create_dir_all(dir);
                let path = dir.join(format!(
                    "crash-{}.txt",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0)
                ));
                let _ = std::fs::write(&path, &msg);
                eprintln!("crash report: {}", path.display());
            }
            prev(info);
        }));
    });
}

/// Convenience macro-like helper for scoped timing.
#[inline]
pub fn scope(name: &'static str) -> ProfileScope {
    ProfileScope::new(name)
}
