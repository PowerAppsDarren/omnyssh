//! DTOs crossing the IPC boundary (tech-gui.md §4.1). Every type derives serde +
//! `specta::Type` so `bindings.ts` is generated, never hand-written. Secret
//! fields (`password`, key material) never appear here.

use serde::{Deserialize, Serialize};

/// Host origin, mirrors `omnyssh_core::ssh::client::HostSource`.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum HostSourceDto {
    SshConfig,
    Manual,
}

/// A host as the frontend sees it — password and private-key material omitted
/// (tech-gui.md §3.4). Mapping from the core `Host` lands with host loading in
/// Stage 1.2.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HostDto {
    pub name: String,
    pub hostname: String,
    pub user: String,
    pub port: u16,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub source: HostSourceDto,
    pub has_key: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password_auth_disabled: Option<bool>,
}
