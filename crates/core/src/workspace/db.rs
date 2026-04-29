//! Database connection bootstrapping. Host-provided paths + SeaORM migrations.
//!
//! Core layer does NOT resolve `~/.swarmnote/` or any home-dir convention —
//! the app data directory is passed into [`crate::AppCore::new`] by the host.

use std::path::{Path, PathBuf};

use migration::{DevicesMigrator, MigratorTrait, WorkspaceMigrator};
use sea_orm::{Database, DatabaseConnection};

use crate::error::AppError;

/// Open a SQLite connection at `path`, creating parent directories as needed.
/// Uses `mode=rwc` so the file is created if missing.
pub async fn connect_sqlite(path: &Path) -> Result<DatabaseConnection, AppError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let url = format!("sqlite:{}?mode=rwc", path.display());
    Ok(Database::connect(&url).await?)
}

/// Initialize (or open + migrate) the global `devices.db` under `app_data_dir`.
///
/// Host supplies `app_data_dir` (desktop: `~/.swarmnote/`, mobile:
/// documentDirectory). The DB file lives at `{app_data_dir}/devices.db`.
pub async fn init_devices_db(app_data_dir: &Path) -> Result<DatabaseConnection, AppError> {
    let db_path: PathBuf = app_data_dir.join("devices.db");
    let db = connect_sqlite(&db_path).await?;
    DevicesMigrator::up(&db, None).await?;
    Ok(db)
}

/// Initialize (or open + migrate) a workspace's `.swarmnote/workspace.db`.
pub async fn init_workspace_db(workspace_path: &Path) -> Result<DatabaseConnection, AppError> {
    let db_path = workspace_path.join(".swarmnote").join("workspace.db");
    let db = connect_sqlite(&db_path).await?;
    WorkspaceMigrator::up(&db, None).await?;
    Ok(db)
}
