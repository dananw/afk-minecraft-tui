//! Configuration file watching for hot-reload support

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::Sender;

use crate::app::AppEvent;

// Re-export Config from core for convenience
#[allow(unused_imports)]
pub use crate::core::Config;

/// Watches a config file for changes and sends reload events
pub struct ConfigWatcher {
    _watcher: RecommendedWatcher,
}

impl ConfigWatcher {
    pub fn new<P: AsRef<Path>>(path: P, event_sender: Sender<AppEvent>) -> anyhow::Result<Self> {
        let path = path.as_ref().to_path_buf();

        let watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            match res {
                Ok(event) => {
                    // Only reload on modify events
                    if matches!(event.kind, notify::EventKind::Modify(_)) {
                        // Debounce: wait a bit to ensure file is fully written
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        let _ = event_sender.send(AppEvent::ConfigReloaded);
                    }
                }
                Err(e) => {
                    eprintln!("Config watch error: {:?}", e);
                }
            }
        })?;

        let mut watcher = watcher;
        watcher.watch(&path, RecursiveMode::NonRecursive)?;

        Ok(Self { _watcher: watcher })
    }
}
