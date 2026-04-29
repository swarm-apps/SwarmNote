//! Tauri IPC command surface.
//!
//! Each sub-module contains the thin command wrappers for one domain
//! (network / pairing / sync / identity / document / fs / yjs / workspace).
//! Implementation lives in [`swarmnote_core`]; commands only marshal
//! arguments and translate error shapes.

pub mod document;
pub mod fs;
pub mod identity;
pub mod network;
pub mod pairing;
pub mod sync;
pub mod workspace;
pub mod yjs;
