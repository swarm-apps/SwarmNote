//! Higher-level filesystem operations built on top of the [`FileSystem`]
//! trait. These add SwarmNote-specific business rules:
//!
//! - **Auto-numbering on name conflict** (`"note" → "note 1"`).
//! - **`.assets/` sidecar tracking** (renames / moves of a `.md` file move
//!   its resource directory atomically).
//! - **Folder-into-descendant rejection** (no recursive self-nesting).
//!
//! All functions are generic over `&(dyn FileSystem)` so both `LocalFs` and
//! future alternatives (e.g. Android SAF) automatically inherit the rules.

use crate::error::{AppError, AppResult};
use crate::fs::FileSystem;

/// Outcome metadata returned from [`move_node`].
#[derive(Debug, Clone, Copy)]
pub struct MoveResult {
    /// `true` if the moved entry was a directory, `false` for a file.
    pub is_dir: bool,
}

/// Create a new `.md` file at `{parent_rel}/{name}.md`, auto-numbering the
/// name (`"note 1.md"`, `"note 2.md"`, …) if a file already exists there.
///
/// Returns the workspace-relative path of the created file.
pub async fn create_file(fs: &dyn FileSystem, parent_rel: &str, name: &str) -> AppResult<String> {
    let filename = format!("{name}.md");
    let candidate = join_rel(parent_rel, &filename);

    let actual = if fs.exists(&candidate).await {
        resolve_conflict(fs, parent_rel, name, "md").await
    } else {
        filename
    };

    let rel = join_rel(parent_rel, &actual);
    fs.write_text(&rel, "").await?;
    Ok(rel)
}

/// Create a new directory at `{parent_rel}/{name}`, auto-numbering the name
/// if a directory already exists there.
pub async fn create_dir(fs: &dyn FileSystem, parent_rel: &str, name: &str) -> AppResult<String> {
    let candidate = join_rel(parent_rel, name);

    let actual = if fs.exists(&candidate).await {
        resolve_conflict(fs, parent_rel, name, "").await
    } else {
        name.to_owned()
    };

    let rel = join_rel(parent_rel, &actual);
    fs.create_dir(&rel).await?;
    Ok(rel)
}

/// Delete a file. For `.md` files, the matching `.assets/` sidecar directory
/// is removed as a best-effort step (errors inside the sidecar are ignored —
/// the main file deletion is what matters).
///
/// Idempotent: missing file is not an error.
pub async fn delete_file(fs: &dyn FileSystem, rel_path: &str) -> AppResult<()> {
    fs.remove_file(rel_path).await?;
    if rel_path.ends_with(".md") {
        let sidecar = sidecar_path(rel_path);
        let _ = fs.remove_dir(&sidecar).await;
    }
    Ok(())
}

/// Recursively delete a directory. Idempotent.
pub async fn delete_dir(fs: &dyn FileSystem, rel_path: &str) -> AppResult<()> {
    fs.remove_dir(rel_path).await
}

/// Rename a file or directory to a new name (staying in the same parent
/// directory). For `.md` files, the matching `.assets/` sidecar is renamed
/// alongside as best-effort (existing target sidecar is left alone).
///
/// If `new_name` lacks an extension and the source is a `.md` file, the
/// extension is preserved automatically: `rename("a.md", "b")` → `"b.md"`.
///
/// Fails with `NameConflict` when the target name already exists.
pub async fn rename(fs: &dyn FileSystem, rel_path: &str, new_name: &str) -> AppResult<String> {
    let (parent, _) = split_parent_name(rel_path);
    let is_file_md = rel_path.ends_with(".md");

    let target_name = if is_file_md && !new_name.contains('.') {
        format!("{new_name}.md")
    } else {
        new_name.to_owned()
    };

    let new_rel = join_rel(parent, &target_name);
    if fs.exists(&new_rel).await {
        return Err(AppError::NameConflict(target_name));
    }

    // Move the sidecar best-effort before renaming the main path.
    if is_file_md {
        let old_sidecar = sidecar_path(rel_path);
        let new_sidecar = sidecar_path(&new_rel);
        if fs.exists(&old_sidecar).await && !fs.exists(&new_sidecar).await {
            let _ = fs.rename(&old_sidecar, &new_sidecar).await;
        }
    }

    fs.rename(rel_path, &new_rel).await?;
    Ok(new_rel)
}

/// Atomically move a file or directory from `from_rel` to `to_rel` (both are
/// full workspace-relative paths, not parent directories).
///
/// Rules enforced:
/// - For `.md` files, the `.assets/` sidecar moves along with the main file
///   (target sidecar must not exist).
/// - Directories cannot be moved into their own descendants.
/// - The target path must not already exist.
pub async fn move_node(fs: &dyn FileSystem, from_rel: &str, to_rel: &str) -> AppResult<MoveResult> {
    if from_rel == to_rel {
        // No-op; callers typically filter this earlier.
        let is_dir = fs.is_dir(from_rel).await;
        return Ok(MoveResult { is_dir });
    }

    if !fs.exists(from_rel).await {
        return Err(AppError::InvalidPath(format!("source missing: {from_rel}")));
    }

    let is_dir = fs.is_dir(from_rel).await;

    if is_dir {
        let from_with_sep = if from_rel.ends_with('/') {
            from_rel.to_owned()
        } else {
            format!("{from_rel}/")
        };
        if to_rel.starts_with(&from_with_sep) {
            return Err(AppError::InvalidPath(
                "cannot move a folder into its own descendant".into(),
            ));
        }
    }

    if fs.exists(to_rel).await {
        return Err(AppError::NameConflict(to_rel.to_owned()));
    }

    // Move sidecar first so a failure leaves the main file in place.
    if !is_dir && from_rel.ends_with(".md") {
        let from_side = sidecar_path(from_rel);
        let to_side = sidecar_path(to_rel);
        if fs.exists(&from_side).await {
            if fs.exists(&to_side).await {
                return Err(AppError::NameConflict(to_side));
            }
            fs.rename(&from_side, &to_side).await?;
        }
    }

    fs.rename(from_rel, to_rel).await?;
    Ok(MoveResult { is_dir })
}

// ── internal helpers ───────────────────────────────────────────────────────

/// Join a parent rel-path + child name with a single `/`. If `parent` is
/// empty the child is returned unchanged.
fn join_rel(parent: &str, child: &str) -> String {
    if parent.is_empty() {
        child.to_owned()
    } else if parent.ends_with('/') {
        format!("{parent}{child}")
    } else {
        format!("{parent}/{child}")
    }
}

/// Split a rel-path into `(parent, file_name)`. Empty parent for top-level.
fn split_parent_name(rel_path: &str) -> (&str, &str) {
    match rel_path.rfind('/') {
        Some(i) => (&rel_path[..i], &rel_path[i + 1..]),
        None => ("", rel_path),
    }
}

/// The sidecar `.assets/` path for a `.md` file. For `"notes/a.md"` →
/// `"notes/a.assets"`.
fn sidecar_path(md_rel: &str) -> String {
    let stem = md_rel.strip_suffix(".md").unwrap_or(md_rel);
    format!("{stem}.assets")
}

/// Find the next unused name by appending ` 1`, ` 2`, … until the candidate
/// doesn't exist. Matches the legacy desktop behavior character-for-character.
async fn resolve_conflict(
    fs: &dyn FileSystem,
    parent_rel: &str,
    base_name: &str,
    extension: &str,
) -> String {
    let mut counter = 1u32;
    loop {
        let candidate = if extension.is_empty() {
            format!("{base_name} {counter}")
        } else {
            format!("{base_name} {counter}.{extension}")
        };
        let full = join_rel(parent_rel, &candidate);
        if !fs.exists(&full).await {
            return candidate;
        }
        counter += 1;
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::LocalFs;

    fn tmp_fs() -> (tempfile::TempDir, LocalFs) {
        let dir = tempfile::tempdir().unwrap();
        let fs = LocalFs::new(dir.path().to_path_buf());
        (dir, fs)
    }

    #[tokio::test]
    async fn create_file_basic() {
        let (_tmp, fs) = tmp_fs();
        let rel = create_file(&fs, "", "test").await.unwrap();
        assert_eq!(rel, "test.md");
        assert!(fs.exists("test.md").await);
    }

    #[tokio::test]
    async fn create_file_in_subdir() {
        let (_tmp, fs) = tmp_fs();
        fs.create_dir("notes").await.unwrap();
        let rel = create_file(&fs, "notes", "diary").await.unwrap();
        assert_eq!(rel, "notes/diary.md");
    }

    #[tokio::test]
    async fn create_file_conflict_auto_numbers() {
        let (_tmp, fs) = tmp_fs();
        fs.write_text("note.md", "").await.unwrap();
        let rel = create_file(&fs, "", "note").await.unwrap();
        assert_eq!(rel, "note 1.md");
    }

    #[tokio::test]
    async fn create_dir_conflict_auto_numbers() {
        let (_tmp, fs) = tmp_fs();
        fs.create_dir("folder").await.unwrap();
        let rel = create_dir(&fs, "", "folder").await.unwrap();
        assert_eq!(rel, "folder 1");
    }

    #[tokio::test]
    async fn delete_file_removes_sidecar() {
        let (_tmp, fs) = tmp_fs();
        fs.write_text("photo.md", "").await.unwrap();
        fs.create_dir("photo.assets").await.unwrap();
        fs.write_text("photo.assets/img.png", "").await.unwrap();

        delete_file(&fs, "photo.md").await.unwrap();
        assert!(!fs.exists("photo.md").await);
        assert!(!fs.exists("photo.assets").await);
    }

    #[tokio::test]
    async fn delete_file_idempotent() {
        let (_tmp, fs) = tmp_fs();
        delete_file(&fs, "nonexistent.md").await.unwrap();
    }

    #[tokio::test]
    async fn rename_file_preserves_extension() {
        let (_tmp, fs) = tmp_fs();
        fs.write_text("old.md", "body").await.unwrap();
        let rel = rename(&fs, "old.md", "new").await.unwrap();
        assert_eq!(rel, "new.md");
        assert_eq!(fs.read_text("new.md").await.unwrap(), "body");
    }

    #[tokio::test]
    async fn rename_conflict_errors() {
        let (_tmp, fs) = tmp_fs();
        fs.write_text("a.md", "").await.unwrap();
        fs.write_text("b.md", "").await.unwrap();
        let err = rename(&fs, "a.md", "b").await.unwrap_err();
        assert!(matches!(err, AppError::NameConflict(_)));
    }

    #[tokio::test]
    async fn rename_directory() {
        let (_tmp, fs) = tmp_fs();
        fs.create_dir("old-folder").await.unwrap();
        let rel = rename(&fs, "old-folder", "new-folder").await.unwrap();
        assert_eq!(rel, "new-folder");
        assert!(fs.is_dir("new-folder").await);
    }

    #[tokio::test]
    async fn move_file_into_folder() {
        let (_tmp, fs) = tmp_fs();
        fs.create_dir("archive").await.unwrap();
        fs.write_text("note.md", "body").await.unwrap();
        let result = move_node(&fs, "note.md", "archive/note.md").await.unwrap();
        assert!(!result.is_dir);
        assert_eq!(fs.read_text("archive/note.md").await.unwrap(), "body");
        assert!(!fs.exists("note.md").await);
    }

    #[tokio::test]
    async fn move_file_moves_sidecar() {
        let (_tmp, fs) = tmp_fs();
        fs.create_dir("archive").await.unwrap();
        fs.write_text("photo.md", "body").await.unwrap();
        fs.create_dir("photo.assets").await.unwrap();
        fs.write_text("photo.assets/img.png", "data").await.unwrap();

        move_node(&fs, "photo.md", "archive/photo.md")
            .await
            .unwrap();
        assert!(fs.exists("archive/photo.md").await);
        assert!(fs.is_dir("archive/photo.assets").await);
        assert!(fs.exists("archive/photo.assets/img.png").await);
        assert!(!fs.exists("photo.md").await);
        assert!(!fs.exists("photo.assets").await);
    }

    #[tokio::test]
    async fn move_folder_recursive() {
        let (_tmp, fs) = tmp_fs();
        fs.create_dir("archive").await.unwrap();
        fs.create_dir("drafts").await.unwrap();
        fs.write_text("drafts/a.md", "").await.unwrap();
        let result = move_node(&fs, "drafts", "archive/drafts").await.unwrap();
        assert!(result.is_dir);
        assert!(fs.is_dir("archive/drafts").await);
        assert!(fs.exists("archive/drafts/a.md").await);
        assert!(!fs.exists("drafts").await);
    }

    #[tokio::test]
    async fn move_folder_into_own_descendant_rejected() {
        let (_tmp, fs) = tmp_fs();
        fs.create_dir("drafts/sub").await.unwrap();
        let err = move_node(&fs, "drafts", "drafts/sub/drafts")
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::InvalidPath(_)));
    }

    #[tokio::test]
    async fn move_rejects_existing_target() {
        let (_tmp, fs) = tmp_fs();
        fs.write_text("a.md", "").await.unwrap();
        fs.write_text("b.md", "").await.unwrap();
        let err = move_node(&fs, "a.md", "b.md").await.unwrap_err();
        assert!(matches!(err, AppError::NameConflict(_)));
    }

    #[tokio::test]
    async fn move_noop_same_path() {
        let (_tmp, fs) = tmp_fs();
        fs.write_text("x.md", "").await.unwrap();
        let result = move_node(&fs, "x.md", "x.md").await.unwrap();
        assert!(!result.is_dir);
        assert!(fs.exists("x.md").await);
    }
}
