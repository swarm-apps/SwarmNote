//! `FileWatcher` impl backed by `notify` + `notify_debouncer_mini` (100ms
//! debounce).
//!
//! Each instance is bound to a single workspace root at construction. Events
//! are translated to [`FileEvent::Modified`] (the underlying library folds
//! create/modify/delete into one `Any` kind — consumers that need finer
//! granularity rely on `FileEvent::Modified` + a re-scan).
//!
//! Hidden paths (`.swarmnote/`, `.git/`, etc.), `.assets/` directories, and
//! non-`.md` files are filtered out here so the `WorkspaceCore` callback
//! only receives events it actually cares about.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind, Debouncer};
use swarmnote_core::{AppError, AppResult, FileEvent, FileEventCallback, FileWatcher};

type FsNotifyWatcher = notify::RecommendedWatcher;

pub struct NotifyFileWatcher {
    root: PathBuf,
    debouncer: Mutex<Option<Debouncer<FsNotifyWatcher>>>,
}

impl NotifyFileWatcher {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            debouncer: Mutex::new(None),
        }
    }
}

#[async_trait]
impl FileWatcher for NotifyFileWatcher {
    async fn watch(&self, callback: FileEventCallback) -> AppResult<()> {
        // Replace any existing watcher for this instance first.
        {
            let mut guard = self.debouncer.lock().expect("watcher mutex poisoned");
            *guard = None;
        }

        let ws_path = self.root.clone();
        let cb = callback.clone();

        let debouncer = new_debouncer(
            Duration::from_millis(100),
            move |events: Result<Vec<notify_debouncer_mini::DebouncedEvent>, notify::Error>| {
                let events = match events {
                    Ok(e) => e,
                    Err(e) => {
                        tracing::warn!("fs watcher error: {e}");
                        return;
                    }
                };

                let relevant: Vec<FileEvent> = events
                    .iter()
                    .filter(|e| e.kind == DebouncedEventKind::Any)
                    .filter(|e| is_relevant_change(&e.path, &ws_path))
                    .filter_map(|e| to_rel_path(&e.path, &ws_path))
                    .map(FileEvent::Modified)
                    .collect();

                if !relevant.is_empty() {
                    // `notify-rs` delivers events on a plain OS thread not
                    // attached to any tokio runtime. `tauri::async_runtime::spawn`
                    // uses the globally-registered Tauri runtime, so it works
                    // from any thread (unlike `tokio::spawn`, which needs the
                    // current-thread tokio context). This bridges into tokio
                    // so the core-layer callback can freely use `tokio::spawn`
                    // per the `FileWatcher` trait contract.
                    let cb = cb.clone();
                    tauri::async_runtime::spawn(async move {
                        cb(relevant);
                    });
                }
            },
        )
        .map_err(|e| AppError::Io(std::io::Error::other(e.to_string())))?;

        // Start recursive watch, then stow the debouncer into `self` so it
        // lives until `unwatch()` / Drop.
        {
            let mut guard = self.debouncer.lock().expect("watcher mutex poisoned");
            *guard = Some(debouncer);
            if let Some(d) = guard.as_mut() {
                d.watcher()
                    .watch(&self.root, notify::RecursiveMode::Recursive)
                    .map_err(|e| AppError::Io(std::io::Error::other(e.to_string())))?;
            }
        }

        tracing::info!("NotifyFileWatcher started on {}", self.root.display());
        Ok(())
    }

    async fn unwatch(&self) {
        let mut guard = self.debouncer.lock().expect("watcher mutex poisoned");
        if guard.take().is_some() {
            tracing::info!("NotifyFileWatcher stopped on {}", self.root.display());
        }
    }
}

/// Drop acts as a last-ditch unwatch; safe because dropping the debouncer
/// also stops the underlying notify thread.
impl Drop for NotifyFileWatcher {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.debouncer.lock() {
            let _ = guard.take();
        }
    }
}

// ── Filtering ──────────────────────────────────────────────────────────────

/// Return `true` when the path is either a `.md` file (created / modified /
/// deleted) or a directory change the workspace cares about. Hidden entries
/// (names starting with `.`) and `.assets/` subtrees are excluded.
fn is_relevant_change(path: &Path, workspace: &Path) -> bool {
    let Ok(rel) = path.strip_prefix(workspace) else {
        return false;
    };

    for component in rel.components() {
        let s = component.as_os_str().to_string_lossy();
        if s.starts_with('.') || s.ends_with(".assets") {
            return false;
        }
    }

    // For directory changes (or paths that no longer exist), we accept the
    // event — downstream code re-scans the tree.
    if path.is_dir() || !path.exists() {
        let ext = path.extension().and_then(|e| e.to_str());
        return ext.is_none() || ext == Some("md");
    }

    path.extension().and_then(|e| e.to_str()) == Some("md")
}

/// Convert an absolute path inside `workspace` to a forward-slash
/// workspace-relative string.
fn to_rel_path(path: &Path, workspace: &Path) -> Option<String> {
    let rel = path.strip_prefix(workspace).ok()?;
    Some(rel.to_string_lossy().replace('\\', "/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ws() -> PathBuf {
        PathBuf::from("/workspace")
    }

    #[test]
    fn hidden_dirs_filtered() {
        assert!(!is_relevant_change(
            Path::new("/workspace/.swarmnote/db.sqlite"),
            &ws()
        ));
        assert!(!is_relevant_change(
            Path::new("/workspace/.git/HEAD"),
            &ws()
        ));
    }

    #[test]
    fn assets_dirs_filtered() {
        assert!(!is_relevant_change(
            Path::new("/workspace/note.assets/img.png"),
            &ws()
        ));
    }

    #[test]
    fn non_md_files_filtered() {
        assert!(!is_relevant_change(
            Path::new("/workspace/image.png"),
            &ws()
        ));
    }

    #[test]
    fn md_files_relevant() {
        let path = Path::new("/workspace/note.md");
        assert!(is_relevant_change(path, &ws()));
    }

    #[test]
    fn to_rel_path_normalizes_slashes() {
        let path = Path::new("/workspace/notes/hello.md");
        assert_eq!(to_rel_path(path, &ws()), Some("notes/hello.md".to_owned()));
    }

    #[test]
    fn to_rel_path_outside_workspace() {
        let path = Path::new("/other/hello.md");
        assert_eq!(to_rel_path(path, &ws()), None);
    }
}
