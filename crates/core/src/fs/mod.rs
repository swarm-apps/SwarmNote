//! Workspace filesystem abstraction + watcher.
//!
//! All paths passed to [`FileSystem`] methods are **workspace-relative** with
//! forward-slash separators (`"notes/a.md"`), regardless of OS. The workspace
//! root is baked into the implementation at construction time — core code
//! never sees absolute paths.
//!
//! Higher-level business helpers (auto-numbered create, sidecar-aware rename,
//! folder move with descendant rejection) live in [`ops`].

pub mod ops;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

// ═══════════════════════════════════════════════════════════════════════════
// FileSystem trait + LocalFs implementation
// ═══════════════════════════════════════════════════════════════════════════

/// A node in the workspace file tree returned by [`FileSystem::scan_tree`].
///
/// Matches the shape emitted to the frontend — do not change field names
/// without coordinating with `src/commands/fs.ts` and the file-tree store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTreeNode {
    /// Workspace-relative path (stable ID in the frontend tree).
    pub id: String,
    /// Display name. For `.md` files the extension is stripped.
    pub name: String,
    /// `Some(children)` for directories, `None` for files.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<FileTreeNode>>,
}

/// Platform-abstracted filesystem operations scoped to a single workspace root.
///
/// Implementations MUST:
/// - Resolve every `rel_path` relative to the root configured at construction.
/// - Reject paths containing `..` or absolute components ([`AppError::PathTraversal`]).
/// - Normalize returned paths to forward slashes (`/`) even on Windows.
#[async_trait]
pub trait FileSystem: Send + Sync + 'static {
    async fn read_text(&self, rel_path: &str) -> AppResult<String>;
    async fn write_text(&self, rel_path: &str, content: &str) -> AppResult<()>;

    async fn read_bytes(&self, rel_path: &str) -> AppResult<Vec<u8>>;
    async fn write_bytes(&self, rel_path: &str, data: &[u8]) -> AppResult<()>;

    async fn exists(&self, rel_path: &str) -> bool;

    /// Return `true` if the path exists and is a directory. Missing paths
    /// return `false` (not an error).
    async fn is_dir(&self, rel_path: &str) -> bool;

    /// Remove a file. Idempotent — missing file is NOT an error.
    async fn remove_file(&self, rel_path: &str) -> AppResult<()>;

    /// Recursively remove a directory. Idempotent.
    async fn remove_dir(&self, rel_path: &str) -> AppResult<()>;

    /// Rename a file or directory in-place. Fails if target exists on OSes
    /// that enforce that; Windows callers requiring no-overwrite MUST check
    /// separately (see the `fs::move_node` helper in PR #2).
    async fn rename(&self, from: &str, to: &str) -> AppResult<()>;

    async fn create_dir(&self, rel_path: &str) -> AppResult<()>;

    /// Recursively scan the workspace and build a tree of `.md` files and
    /// directories. Hidden entries (names starting with `.`) and `.assets/`
    /// resource directories are excluded; symlinks are skipped. Results are
    /// sorted directories-first, then case-insensitive alphabetical.
    async fn scan_tree(&self, rel_path: &str) -> AppResult<Vec<FileTreeNode>>;

    /// Save a media file under the note's `.assets/` sidecar directory using
    /// a content-addressed filename derived from blake3 of `data`.
    ///
    /// Returns the workspace-relative path of the stored file (e.g.
    /// `"notes/photo.assets/img-af3b9e2c.png"`).
    ///
    /// If `data` hashes to the same prefix as an existing file at the target
    /// path, the write is skipped (content-addressed dedup).
    async fn save_media(&self, note_rel: &str, file_name: &str, data: &[u8]) -> AppResult<String>;
}

/// `FileSystem` implementation backed by `tokio::fs`, shared by both desktop
/// (user-chosen path) and mobile sandbox (`documentDirectory/workspaces/…`).
pub struct LocalFs {
    root: PathBuf,
}

impl LocalFs {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolve a workspace-relative path, rejecting traversal attempts.
    /// Pure string-level check — works for both existing and new paths,
    /// unlike a `canonicalize`-based validator.
    fn resolve(&self, rel_path: &str) -> AppResult<PathBuf> {
        if rel_path.is_empty() {
            return Ok(self.root.clone());
        }
        if rel_path.contains("..") {
            return Err(AppError::PathTraversal(rel_path.to_owned()));
        }
        let p = Path::new(rel_path);
        if p.is_absolute() {
            return Err(AppError::PathTraversal(rel_path.to_owned()));
        }
        for component in p.components() {
            use std::path::Component;
            match component {
                Component::Normal(_) | Component::CurDir => {}
                _ => return Err(AppError::PathTraversal(rel_path.to_owned())),
            }
        }
        Ok(self.root.join(p))
    }
}

#[async_trait]
impl FileSystem for LocalFs {
    async fn read_text(&self, rel_path: &str) -> AppResult<String> {
        let full = self.resolve(rel_path)?;
        tokio::fs::read_to_string(&full).await.map_err(Into::into)
    }

    async fn write_text(&self, rel_path: &str, content: &str) -> AppResult<()> {
        let full = self.resolve(rel_path)?;
        if let Some(parent) = full.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&full, content).await.map_err(Into::into)
    }

    async fn read_bytes(&self, rel_path: &str) -> AppResult<Vec<u8>> {
        let full = self.resolve(rel_path)?;
        tokio::fs::read(&full).await.map_err(Into::into)
    }

    async fn write_bytes(&self, rel_path: &str, data: &[u8]) -> AppResult<()> {
        let full = self.resolve(rel_path)?;
        if let Some(parent) = full.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&full, data).await.map_err(Into::into)
    }

    async fn exists(&self, rel_path: &str) -> bool {
        let Ok(full) = self.resolve(rel_path) else {
            return false;
        };
        tokio::fs::try_exists(&full).await.unwrap_or(false)
    }

    async fn is_dir(&self, rel_path: &str) -> bool {
        let Ok(full) = self.resolve(rel_path) else {
            return false;
        };
        tokio::fs::metadata(&full)
            .await
            .map(|m| m.is_dir())
            .unwrap_or(false)
    }

    async fn remove_file(&self, rel_path: &str) -> AppResult<()> {
        let full = self.resolve(rel_path)?;
        match tokio::fs::remove_file(&full).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    async fn remove_dir(&self, rel_path: &str) -> AppResult<()> {
        let full = self.resolve(rel_path)?;
        match tokio::fs::remove_dir_all(&full).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    async fn rename(&self, from: &str, to: &str) -> AppResult<()> {
        let from_full = self.resolve(from)?;
        let to_full = self.resolve(to)?;
        if let Some(parent) = to_full.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::rename(&from_full, &to_full)
            .await
            .map_err(Into::into)
    }

    async fn create_dir(&self, rel_path: &str) -> AppResult<()> {
        let full = self.resolve(rel_path)?;
        tokio::fs::create_dir_all(&full).await.map_err(Into::into)
    }

    async fn scan_tree(&self, rel_path: &str) -> AppResult<Vec<FileTreeNode>> {
        let start = self.resolve(rel_path)?;
        let root = self.root.clone();
        tokio::task::spawn_blocking(move || scan_dir(&root, &start))
            .await
            .map_err(|e| AppError::Io(std::io::Error::other(e.to_string())))?
    }

    async fn save_media(&self, note_rel: &str, file_name: &str, data: &[u8]) -> AppResult<String> {
        // Resource dir: "notes/photo.md" → "notes/photo.assets/"
        let note_path = self.resolve(note_rel)?;
        let resource_dir = self
            .root
            .join(format!("{}.assets", note_path.with_extension("").display()));
        let stem = Path::new(file_name)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let ext = Path::new(file_name)
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();
        let root = self.root.clone();
        let data = data.to_vec();

        // Hashing is CPU-bound and scales with media size (blake3 is ~3 GB/s
        // but a 50 MB video still costs ~15 ms); keep it inside
        // `spawn_blocking` alongside the write so tokio workers don't stall.
        tokio::task::spawn_blocking(move || -> AppResult<String> {
            let hash = blake3::hash(&data);
            let short_hash = hash.to_hex();
            let unique_name = format!("{stem}-{}{ext}", &short_hash.as_str()[..8]);
            let target = resource_dir.join(&unique_name);

            std::fs::create_dir_all(&resource_dir)?;
            // Content-addressed dedup: same hash in filename ⇒ same content,
            // skip the rewrite.
            if !target.exists() {
                std::fs::write(&target, &data)?;
            }
            let rel = target
                .strip_prefix(&root)
                .unwrap_or(&target)
                .to_string_lossy()
                .replace('\\', "/");
            Ok(rel)
        })
        .await
        .map_err(|e| AppError::Io(std::io::Error::other(e.to_string())))?
    }
}

fn scan_dir(root: &Path, dir: &Path) -> AppResult<Vec<FileTreeNode>> {
    let entries = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };

    let mut dirs = Vec::new();
    let mut files = Vec::new();

    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if name_str.starts_with('.') {
            continue;
        }

        // DirEntry::file_type caches the OS-provided type on Linux/macOS
        // (no extra stat). is_symlink() here avoids the separate read_link()
        // syscall that previously ran per-entry even for non-symlinks.
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        if file_type.is_symlink() {
            continue;
        }

        let rel_path = entry
            .path()
            .strip_prefix(root)
            .unwrap_or(entry.path().as_path())
            .to_string_lossy()
            .replace('\\', "/");

        if file_type.is_dir() {
            if name_str.ends_with(".assets") {
                continue;
            }
            let children = scan_dir(root, &entry.path())?;
            dirs.push(FileTreeNode {
                id: rel_path,
                name: name_str.into_owned(),
                children: Some(children),
            });
        } else if file_type.is_file() && name_str.ends_with(".md") {
            let display_name = name_str.strip_suffix(".md").unwrap_or(&name_str);
            files.push(FileTreeNode {
                id: rel_path,
                name: display_name.to_string(),
                children: None,
            });
        }
    }

    dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    dirs.extend(files);
    Ok(dirs)
}

// ═══════════════════════════════════════════════════════════════════════════
// FileWatcher trait (desktop-only; mobile holds it as `Option<Arc<_>>`)
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub enum FileEvent {
    Created(String),
    Modified(String),
    Deleted(String),
    Renamed { from: String, to: String },
}

pub type FileEventCallback = Arc<dyn Fn(Vec<FileEvent>) + Send + Sync + 'static>;

/// Recursive directory watcher. Implementations MUST:
///
/// - Debounce rapid events (≥100ms recommended).
/// - Filter hidden entries + `.assets/` subtrees + non-`.md` files.
/// - Deliver workspace-relative paths with forward slashes.
/// - **Invoke the callback from within a tokio runtime context**, so that
///   consumers may use `tokio::spawn` / `tokio::sync::*` freely. Native
///   backends (e.g. `notify-rs`) that deliver events on dedicated OS
///   threads are responsible for bridging into tokio themselves.
#[async_trait]
pub trait FileWatcher: Send + Sync + 'static {
    async fn watch(&self, callback: FileEventCallback) -> AppResult<()>;
    async fn unwatch(&self);
}

// ═══════════════════════════════════════════════════════════════════════════
// Unit tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_fs() -> (tempfile::TempDir, LocalFs) {
        let dir = tempfile::tempdir().expect("tempdir");
        let fs = LocalFs::new(dir.path().to_path_buf());
        (dir, fs)
    }

    #[tokio::test]
    async fn write_and_read_text_roundtrip() {
        let (_tmp, fs) = tmp_fs();
        fs.write_text("note.md", "# hello").await.unwrap();
        assert_eq!(fs.read_text("note.md").await.unwrap(), "# hello");
    }

    #[tokio::test]
    async fn write_text_creates_parent_dirs() {
        let (tmp, fs) = tmp_fs();
        fs.write_text("a/b/c/deep.md", "body").await.unwrap();
        assert!(tmp.path().join("a/b/c/deep.md").exists());
    }

    #[tokio::test]
    async fn read_missing_file_returns_io_error() {
        let (_tmp, fs) = tmp_fs();
        let err = fs.read_text("absent.md").await.unwrap_err();
        assert!(matches!(err, AppError::Io(_)));
    }

    #[tokio::test]
    async fn exists_reports_true_after_write() {
        let (_tmp, fs) = tmp_fs();
        assert!(!fs.exists("x.md").await);
        fs.write_text("x.md", "").await.unwrap();
        assert!(fs.exists("x.md").await);
    }

    #[tokio::test]
    async fn remove_file_is_idempotent() {
        let (_tmp, fs) = tmp_fs();
        fs.write_text("y.md", "").await.unwrap();
        fs.remove_file("y.md").await.unwrap();
        fs.remove_file("y.md").await.unwrap();
        assert!(!fs.exists("y.md").await);
    }

    #[tokio::test]
    async fn remove_dir_recursive_and_idempotent() {
        let (tmp, fs) = tmp_fs();
        fs.create_dir("nested/sub").await.unwrap();
        fs.write_text("nested/sub/file.md", "").await.unwrap();
        fs.remove_dir("nested").await.unwrap();
        assert!(!tmp.path().join("nested").exists());
        fs.remove_dir("nested").await.unwrap();
    }

    #[tokio::test]
    async fn path_traversal_rejected() {
        let (_tmp, fs) = tmp_fs();
        let err = fs.read_text("../etc/passwd").await.unwrap_err();
        assert!(matches!(err, AppError::PathTraversal(_)));
        let err = fs.write_text("../outside.md", "x").await.unwrap_err();
        assert!(matches!(err, AppError::PathTraversal(_)));
    }

    #[tokio::test]
    async fn absolute_path_rejected() {
        let (_tmp, fs) = tmp_fs();
        let abs = if cfg!(windows) {
            "C:/absolute.md"
        } else {
            "/absolute.md"
        };
        let err = fs.read_text(abs).await.unwrap_err();
        assert!(matches!(err, AppError::PathTraversal(_)));
    }

    #[tokio::test]
    async fn rename_moves_file() {
        let (_tmp, fs) = tmp_fs();
        fs.write_text("old.md", "body").await.unwrap();
        fs.rename("old.md", "new.md").await.unwrap();
        assert_eq!(fs.read_text("new.md").await.unwrap(), "body");
        assert!(!fs.exists("old.md").await);
    }

    #[tokio::test]
    async fn scan_tree_filters_hidden() {
        let (tmp, fs) = tmp_fs();
        std::fs::create_dir(tmp.path().join(".swarmnote")).unwrap();
        std::fs::write(tmp.path().join(".DS_Store"), "").unwrap();
        std::fs::write(tmp.path().join("visible.md"), "").unwrap();

        let tree = fs.scan_tree("").await.unwrap();
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].name, "visible");
    }

    #[tokio::test]
    async fn scan_tree_folders_first_case_insensitive() {
        let (tmp, fs) = tmp_fs();
        std::fs::write(tmp.path().join("zebra.md"), "").unwrap();
        std::fs::write(tmp.path().join("Alice.md"), "").unwrap();
        std::fs::create_dir(tmp.path().join("Notes")).unwrap();
        std::fs::create_dir(tmp.path().join("archive")).unwrap();

        let tree = fs.scan_tree("").await.unwrap();
        assert_eq!(tree.len(), 4);
        assert_eq!(tree[0].name, "archive");
        assert_eq!(tree[1].name, "Notes");
        assert_eq!(tree[2].name, "Alice");
        assert_eq!(tree[3].name, "zebra");
    }

    #[tokio::test]
    async fn scan_tree_rel_paths_use_forward_slashes() {
        let (tmp, fs) = tmp_fs();
        std::fs::create_dir(tmp.path().join("sub")).unwrap();
        std::fs::write(tmp.path().join("sub/note.md"), "").unwrap();

        let tree = fs.scan_tree("").await.unwrap();
        let sub = &tree[0];
        let note = &sub.children.as_ref().unwrap()[0];
        assert_eq!(note.id, "sub/note.md");
    }

    #[tokio::test]
    async fn scan_tree_skips_assets_dir() {
        let (tmp, fs) = tmp_fs();
        std::fs::write(tmp.path().join("photo.md"), "").unwrap();
        std::fs::create_dir(tmp.path().join("photo.assets")).unwrap();
        std::fs::write(tmp.path().join("photo.assets/img.png"), b"data").unwrap();

        let tree = fs.scan_tree("").await.unwrap();
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].name, "photo");
    }

    #[tokio::test]
    async fn save_media_content_addressed_dedup() {
        let (_tmp, fs) = tmp_fs();
        fs.write_text("notes/a.md", "body").await.unwrap();

        let p1 = fs
            .save_media("notes/a.md", "image.png", b"image-bytes")
            .await
            .unwrap();
        let p2 = fs
            .save_media("notes/a.md", "image.png", b"image-bytes")
            .await
            .unwrap();

        assert_eq!(p1, p2);
        assert!(p1.starts_with("notes/a.assets/"));
        assert!(p1.contains("image-"));
        assert!(p1.ends_with(".png"));
    }

    #[tokio::test]
    async fn save_media_different_content_different_path() {
        let (_tmp, fs) = tmp_fs();
        fs.write_text("notes/a.md", "body").await.unwrap();

        let p1 = fs
            .save_media("notes/a.md", "img.png", b"content-one")
            .await
            .unwrap();
        let p2 = fs
            .save_media("notes/a.md", "img.png", b"content-two")
            .await
            .unwrap();

        assert_ne!(p1, p2);
    }

    #[tokio::test]
    async fn scan_tree_empty_returns_empty_vec() {
        let (_tmp, fs) = tmp_fs();
        let tree = fs.scan_tree("").await.unwrap();
        assert!(tree.is_empty());
    }
}
