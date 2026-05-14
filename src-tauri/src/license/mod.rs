// src-tauri/src/license/mod.rs
pub mod errors;
pub mod fingerprint;
pub mod storage;
pub mod trial;
pub mod certificate;
pub mod api_client;
pub mod state;
pub mod heartbeat;
pub mod watermark;

pub use errors::LicenseError;
