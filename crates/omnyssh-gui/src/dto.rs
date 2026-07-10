//! DTOs crossing the IPC boundary (tech-gui.md §4.1). Every type derives serde +
//! `specta::Type` so `bindings.ts` is generated, never hand-written. Secret
//! fields (`password`, key material) never appear here.

use serde::{Deserialize, Serialize};

use omnyssh_core::event::{Metrics, ProcessInfo};
use omnyssh_core::ssh::client::{ConnectionStatus, Host, HostSource};

/// Host origin, mirrors `omnyssh_core::ssh::client::HostSource`.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum HostSourceDto {
    SshConfig,
    Manual,
}

/// A host as the frontend sees it — password and private-key material omitted
/// (tech-gui.md §3.4). `hasKey` reports whether an identity file is configured;
/// the key path itself never crosses the boundary.
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

/// Live connection state for a host (tech-gui.md §4.1). Internally tagged so the
/// frontend consumes a discriminated union keyed on `kind`.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ConnectionStatusDto {
    Unknown,
    Connecting,
    Connected,
    Failed { message: String },
}

/// A single process in the "top processes" panel (tech-gui.md §4.1).
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProcessDto {
    pub name: String,
    pub cpu_percent: f64,
    pub mem_percent: f64,
}

/// A metrics snapshot for a host (tech-gui.md §4.1). The core's `Instant` is
/// flattened to `ageSeconds` (seconds since the sample) so it can serialise.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MetricsDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ram_percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk_percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uptime: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_avg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_info: Option<String>,
    pub top_processes: Vec<ProcessDto>,
    pub age_seconds: u64,
}

impl From<&HostSource> for HostSourceDto {
    fn from(source: &HostSource) -> Self {
        match source {
            HostSource::SshConfig => Self::SshConfig,
            HostSource::Manual => Self::Manual,
        }
    }
}

impl From<&Host> for HostDto {
    fn from(host: &Host) -> Self {
        Self {
            name: host.name.clone(),
            hostname: host.hostname.clone(),
            user: host.user.clone(),
            port: host.port,
            tags: host.tags.clone(),
            notes: host.notes.clone(),
            source: (&host.source).into(),
            has_key: host.identity_file.is_some(),
            password_auth_disabled: host.password_auth_disabled,
        }
    }
}

impl From<&ConnectionStatus> for ConnectionStatusDto {
    fn from(status: &ConnectionStatus) -> Self {
        match status {
            ConnectionStatus::Unknown => Self::Unknown,
            ConnectionStatus::Connecting => Self::Connecting,
            ConnectionStatus::Connected => Self::Connected,
            ConnectionStatus::Failed(message) => Self::Failed {
                message: message.clone(),
            },
        }
    }
}

impl From<&ProcessInfo> for ProcessDto {
    fn from(process: &ProcessInfo) -> Self {
        Self {
            name: process.name.clone(),
            cpu_percent: process.cpu_percent,
            mem_percent: process.mem_percent,
        }
    }
}

impl From<&Metrics> for MetricsDto {
    fn from(metrics: &Metrics) -> Self {
        Self {
            cpu_percent: metrics.cpu_percent,
            ram_percent: metrics.ram_percent,
            disk_percent: metrics.disk_percent,
            uptime: metrics.uptime.clone(),
            load_avg: metrics.load_avg.clone(),
            os_info: metrics.os_info.clone(),
            top_processes: metrics
                .top_processes
                .as_deref()
                .unwrap_or_default()
                .iter()
                .map(ProcessDto::from)
                .collect(),
            age_seconds: metrics.last_updated.elapsed().as_secs(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host_with_secret() -> Host {
        Host {
            name: "web-prod-1".to_string(),
            hostname: "10.0.0.1".to_string(),
            user: "deploy".to_string(),
            port: 2222,
            identity_file: Some("/home/me/.ssh/id_ed25519".to_string()),
            password: Some("s3cr3t-p4ss".to_string()),
            tags: vec!["prod".to_string()],
            notes: Some("primary".to_string()),
            source: HostSource::Manual,
            password_auth_disabled: Some(true),
            ..Host::default()
        }
    }

    #[test]
    fn host_dto_never_serialises_a_password_or_key() {
        let dto = HostDto::from(&host_with_secret());
        let json = serde_json::to_string(&dto).expect("serialise HostDto");
        // The wire form must carry neither the secret field nor its value.
        // (`passwordAuthDisabled` is a public boolean flag, not the password.)
        assert!(
            !json.contains(r#""password""#),
            "password field leaked: {json}"
        );
        assert!(!json.contains("s3cr3t"), "password value leaked: {json}");
        assert!(!json.contains("identityFile"), "key field leaked: {json}");
        assert!(!json.contains("id_ed25519"), "key path leaked: {json}");
    }

    #[test]
    fn host_dto_maps_public_fields() {
        let dto = HostDto::from(&host_with_secret());
        assert_eq!(dto.name, "web-prod-1");
        assert_eq!(dto.hostname, "10.0.0.1");
        assert_eq!(dto.user, "deploy");
        assert_eq!(dto.port, 2222);
        assert_eq!(dto.tags, vec!["prod".to_string()]);
        assert_eq!(dto.notes.as_deref(), Some("primary"));
        assert!(matches!(dto.source, HostSourceDto::Manual));
        // `hasKey` is derived from the identity file, which itself stays backend-side.
        assert!(dto.has_key);
        assert_eq!(dto.password_auth_disabled, Some(true));
    }

    #[test]
    fn host_dto_has_key_is_false_without_identity_file() {
        let host = Host {
            identity_file: None,
            ..Host::default()
        };
        assert!(!HostDto::from(&host).has_key);
    }

    #[test]
    fn connection_status_dto_maps_every_variant() {
        assert!(matches!(
            ConnectionStatusDto::from(&ConnectionStatus::Unknown),
            ConnectionStatusDto::Unknown
        ));
        assert!(matches!(
            ConnectionStatusDto::from(&ConnectionStatus::Connecting),
            ConnectionStatusDto::Connecting
        ));
        assert!(matches!(
            ConnectionStatusDto::from(&ConnectionStatus::Connected),
            ConnectionStatusDto::Connected
        ));
        let failed = ConnectionStatusDto::from(&ConnectionStatus::Failed("boom".to_string()));
        match failed {
            ConnectionStatusDto::Failed { message } => assert_eq!(message, "boom"),
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn connection_status_dto_tags_on_kind() {
        let json = serde_json::to_string(&ConnectionStatusDto::from(&ConnectionStatus::Failed(
            "down".to_string(),
        )))
        .expect("serialise status");
        assert_eq!(json, r#"{"kind":"failed","message":"down"}"#);
        let connected = serde_json::to_string(&ConnectionStatusDto::Connected).unwrap();
        assert_eq!(connected, r#"{"kind":"connected"}"#);
    }

    #[test]
    fn metrics_dto_passes_none_through() {
        let dto = MetricsDto::from(&Metrics::default());
        assert!(dto.cpu_percent.is_none());
        assert!(dto.ram_percent.is_none());
        assert!(dto.disk_percent.is_none());
        assert!(dto.uptime.is_none());
        assert!(dto.load_avg.is_none());
        assert!(dto.os_info.is_none());
        assert!(dto.top_processes.is_empty());
        // A freshly stamped sample is age zero.
        assert_eq!(dto.age_seconds, 0);
    }

    #[test]
    fn metrics_dto_maps_populated_fields() {
        let metrics = Metrics {
            cpu_percent: Some(42.5),
            ram_percent: Some(70.0),
            disk_percent: Some(12.0),
            uptime: Some("3 days".to_string()),
            load_avg: Some("0.5 0.4 0.3".to_string()),
            os_info: Some("Ubuntu 22.04".to_string()),
            top_processes: Some(vec![ProcessInfo {
                name: "postgres".to_string(),
                cpu_percent: 30.0,
                mem_percent: 15.0,
            }]),
            ..Metrics::default()
        };
        let dto = MetricsDto::from(&metrics);
        assert_eq!(dto.cpu_percent, Some(42.5));
        assert_eq!(dto.ram_percent, Some(70.0));
        assert_eq!(dto.disk_percent, Some(12.0));
        assert_eq!(dto.uptime.as_deref(), Some("3 days"));
        assert_eq!(dto.os_info.as_deref(), Some("Ubuntu 22.04"));
        assert_eq!(dto.top_processes.len(), 1);
        assert_eq!(dto.top_processes[0].name, "postgres");
        assert_eq!(dto.top_processes[0].cpu_percent, 30.0);
        assert_eq!(dto.top_processes[0].mem_percent, 15.0);
    }
}
