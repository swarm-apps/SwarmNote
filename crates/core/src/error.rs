//! Unified error type for swarmnote-core.
//!
//! `AppError` uses **structured variants** with named fields so every
//! downstream consumer (desktop Tauri IPC, mobile-core FFI, core logic) can
//! match on the `kind` discriminant and access typed payloads without
//! parsing strings.
//!
//! Frontend IPC wire shape: `{ "kind": "<VariantName>", "message": "<Display>" }`.
//! The `kind` string is the Rust variant identifier and is stable across
//! patch releases — frontend i18n / recovery code can match on it.

use std::borrow::Cow;

use serde::ser::SerializeStruct;
use serde::Serialize;
use uuid::Uuid;

/// Core-layer error type. Every `AppResult<T>` returns this.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    // ── Automatic `?` conversions ─────────────────────────────
    #[error("database error: {0}")]
    Database(#[from] sea_orm::DbErr),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    // ── Identity (Ed25519 keypair) ────────────────────────────
    #[error("keypair decode failed: {0}")]
    KeypairDecode(String),
    #[error("keypair encode failed: {0}")]
    KeypairEncode(String),

    // ── Keychain ──────────────────────────────────────────────
    #[error("keychain unavailable: {0}")]
    KeychainUnavailable(String),

    // ── Config ────────────────────────────────────────────────
    #[error("config parse failed: {0}")]
    ConfigParse(String),

    // ── Yjs / documents ───────────────────────────────────────
    #[error("yjs decode ({context}): {reason}")]
    YjsDecode {
        context: &'static str,
        reason: String,
    },
    #[error("yjs apply ({context}): {reason}")]
    YjsApply {
        context: &'static str,
        reason: String,
    },
    #[error("document row missing: {0}")]
    DocRowMissing(Uuid),
    #[error("document not open: {0}")]
    DocNotOpen(Uuid),

    // ── Network / P2P ─────────────────────────────────────────
    #[error("P2P node is not running")]
    NetworkNotRunning,
    #[error("P2P node is already running")]
    NetworkAlreadyRunning,
    #[error("swarm I/O failed ({context}): {reason}")]
    SwarmIo {
        context: &'static str,
        reason: String,
    },

    // ── Pairing ───────────────────────────────────────────────
    #[error("pairing code has expired")]
    PairingCodeExpired,
    #[error("pairing code is invalid")]
    PairingCodeInvalid,
    #[error("no pending pairing request for id {0}")]
    PairingPendingNotFound(u64),
    #[error("pairing failed ({context}): {reason}")]
    PairingOther {
        context: &'static str,
        reason: String,
    },

    // ── Workspace / FS ────────────────────────────────────────
    #[error("no workspace database open")]
    NoWorkspaceDb,
    #[error("app data directory not found")]
    NoAppDataDir,
    #[error("folder is not empty: {0}")]
    FolderNotEmpty(String),
    #[error("invalid path: {0}")]
    InvalidPath(String),
    #[error("path traversal detected: {0}")]
    PathTraversal(String),
    #[error("name conflict: {0}")]
    NameConflict(String),
    #[error("no workspace open")]
    NoWorkspaceOpen,
    #[error("workspace {workspace_id} close failed: {} doc(s) failed to persist", failures.len())]
    WorkspaceCloseFailed {
        workspace_id: Uuid,
        failures: Vec<(Uuid, String)>,
    },

    // ── Window (desktop shell) ────────────────────────────────
    #[error("window error: {0}")]
    Window(String),
}

/// Structured serialization for frontend consumption: `{ kind, message }`.
///
/// `kind` is the variant identifier (stable across patch versions — frontend
/// match arms SHALL NOT break on minor refactors). `message` is the
/// `Display` form, suitable for logs and fallback UI.
impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("AppError", 2)?;

        // Message is computed once via Display; Cow keeps the hot-path
        // static-string variants allocation-free.
        let message: Cow<'_, str> = Cow::Owned(self.to_string());

        let kind = match self {
            AppError::Database(_) => "Database",
            AppError::Io(_) => "Io",

            AppError::KeypairDecode(_) => "KeypairDecode",
            AppError::KeypairEncode(_) => "KeypairEncode",

            AppError::KeychainUnavailable(_) => "KeychainUnavailable",

            AppError::ConfigParse(_) => "ConfigParse",

            AppError::YjsDecode { .. } => "YjsDecode",
            AppError::YjsApply { .. } => "YjsApply",
            AppError::DocRowMissing(_) => "DocRowMissing",
            AppError::DocNotOpen(_) => "DocNotOpen",

            AppError::NetworkNotRunning => "NetworkNotRunning",
            AppError::NetworkAlreadyRunning => "NetworkAlreadyRunning",
            AppError::SwarmIo { .. } => "SwarmIo",

            AppError::PairingCodeExpired => "PairingCodeExpired",
            AppError::PairingCodeInvalid => "PairingCodeInvalid",
            AppError::PairingPendingNotFound(_) => "PairingPendingNotFound",
            AppError::PairingOther { .. } => "PairingOther",

            AppError::NoWorkspaceDb => "NoWorkspaceDb",
            AppError::NoAppDataDir => "NoAppDataDir",
            AppError::FolderNotEmpty(_) => "FolderNotEmpty",
            AppError::InvalidPath(_) => "InvalidPath",
            AppError::PathTraversal(_) => "PathTraversal",
            AppError::NameConflict(_) => "NameConflict",
            AppError::NoWorkspaceOpen => "NoWorkspaceOpen",
            AppError::WorkspaceCloseFailed { .. } => "WorkspaceCloseFailed",

            AppError::Window(_) => "Window",
        };

        state.serialize_field("kind", kind)?;
        state.serialize_field("message", &message)?;
        state.end()
    }
}

pub type AppResult<T> = Result<T, AppError>;

// ── specta TS bindings ────────────────────────────────────────────
//
// AppError 的 Serialize impl 写出 `{ kind, message }`,但 specta 看不到这
// 形状(thiserror enum 的字段千差万别),手写 `Type` impl 喂一个等价投影。
// `inline` 让它不在 TS 里 export 名字 —— Throw 模式下错误从不出现在
// commands 签名,这个 payload 只是 schema 占位。

#[cfg(feature = "specta")]
#[derive(specta::Type)]
#[specta(inline)]
#[allow(dead_code)]
struct AppErrorPayload {
    kind: String,
    message: String,
}

#[cfg(feature = "specta")]
impl specta::Type for AppError {
    fn definition(types: &mut specta::Types) -> specta::datatype::DataType {
        <AppErrorPayload as specta::Type>::definition(types)
    }
}
