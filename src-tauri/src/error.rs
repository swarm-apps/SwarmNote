//! Re-export of `swarmnote_core::{AppError, AppResult}`.
//!
//! The desktop shell no longer keeps a distinct error type after PR #3 —
//! every Tauri command returns `swarmnote_core::AppResult<T>` directly.
//! The core `AppError` implements `Serialize` to `{ kind, message }`, which
//! Tauri consumes when marshalling command errors to the frontend.

pub use swarmnote_core::{AppError, AppResult};
