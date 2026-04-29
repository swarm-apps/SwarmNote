//! Device OS info — exchanged automatically through libp2p `agent_version`
//! via the Identify protocol.

use serde::{Deserialize, Serialize};

/// Device operating system + user-facing name, embedded in the
/// `agent_version` string libp2p advertises via Identify.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OsInfo {
    /// User-set device name, propagated via the `agent_version` `name=` field.
    pub name: Option<String>,
    pub hostname: String,
    pub os: String,
    pub platform: String,
    pub arch: String,
}

impl OsInfo {
    /// SwarmNote client `agent_version` prefix.
    pub const AGENT_PREFIX: &str = "swarmnote/";

    pub fn is_swarmnote_agent(agent_version: &str) -> bool {
        agent_version.starts_with(Self::AGENT_PREFIX)
    }

    /// Fallback used when the `agent_version` can't be parsed — uses the
    /// last 8 chars of the PeerId as a pseudo-hostname so the UI has
    /// *something* to show.
    pub fn unknown_from_peer_id(peer_id: &swarm_p2p_core::libp2p::PeerId) -> Self {
        let s = peer_id.to_string();
        Self {
            name: None,
            hostname: s[s.len().saturating_sub(8)..].to_string(),
            os: "unknown".to_string(),
            platform: "unknown".to_string(),
            arch: "unknown".to_string(),
        }
    }

    /// Encode as an `agent_version` string.
    ///
    /// With name: `swarmnote/{version}; name={name}; host=...; os=...; platform=...; arch=...`
    /// Without:   `swarmnote/{version}; host=...; os=...; platform=...; arch=...`
    pub fn to_agent_version(&self, version: &str) -> String {
        let name_part = self
            .name
            .as_deref()
            .map(|n| format!("; name={n}"))
            .unwrap_or_default();
        format!(
            "swarmnote/{version}{name_part}; host={}; os={}; platform={}; arch={}",
            self.hostname, self.os, self.platform, self.arch
        )
    }

    /// Parse an `agent_version` string. Returns `None` if the prefix doesn't
    /// match (non-SwarmNote agent).
    pub fn from_agent_version(agent_version: &str) -> Option<Self> {
        if !agent_version.starts_with("swarmnote/") {
            return None;
        }

        let mut name = None;
        let mut hostname = String::new();
        let mut os = String::new();
        let mut platform = String::new();
        let mut arch = String::new();

        for part in agent_version.split(';').map(str::trim) {
            if let Some(val) = part.strip_prefix("name=") {
                name = Some(val.to_string());
            } else if let Some(val) = part.strip_prefix("host=") {
                hostname = val.to_string();
            } else if let Some(val) = part.strip_prefix("os=") {
                os = val.to_string();
            } else if let Some(val) = part.strip_prefix("platform=") {
                platform = val.to_string();
            } else if let Some(val) = part.strip_prefix("arch=") {
                arch = val.to_string();
            }
        }

        Some(OsInfo {
            name,
            hostname,
            os,
            platform,
            arch,
        })
    }
}

impl Default for OsInfo {
    fn default() -> Self {
        Self {
            name: None,
            hostname: hostname::get()
                .map(|h| h.to_string_lossy().to_string())
                .unwrap_or_default(),
            os: std::env::consts::OS.to_string(),
            platform: std::env::consts::FAMILY.to_string(),
            arch: std::env::consts::ARCH.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_version_roundtrip() {
        let info = OsInfo {
            name: None,
            hostname: "MacBook-Pro".to_string(),
            os: "macos".to_string(),
            platform: "macos".to_string(),
            arch: "aarch64".to_string(),
        };

        let agent = info.to_agent_version("0.2.0");
        assert_eq!(
            agent,
            "swarmnote/0.2.0; host=MacBook-Pro; os=macos; platform=macos; arch=aarch64"
        );
        assert_eq!(OsInfo::from_agent_version(&agent).unwrap(), info);
    }

    #[test]
    fn agent_version_with_name() {
        let info = OsInfo {
            name: Some("光印-华为410".to_string()),
            hostname: "DESKTOP-GQ0OBT2".to_string(),
            os: "windows".to_string(),
            platform: "windows".to_string(),
            arch: "x86_64".to_string(),
        };

        let agent = info.to_agent_version("0.2.2");
        assert_eq!(
            agent,
            "swarmnote/0.2.2; name=光印-华为410; host=DESKTOP-GQ0OBT2; os=windows; platform=windows; arch=x86_64"
        );
        assert_eq!(OsInfo::from_agent_version(&agent).unwrap(), info);
    }

    #[test]
    fn non_swarmnote_agent_rejected() {
        assert!(OsInfo::from_agent_version("swarmdrop/0.4.0; os=windows").is_none());
        assert!(OsInfo::from_agent_version("some-other-app/1.0").is_none());
    }

    #[test]
    fn default_fills_host_metadata() {
        let info = OsInfo::default();
        assert!(!info.os.is_empty());
        assert!(!info.arch.is_empty());
    }
}
