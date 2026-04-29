//! 设备模块
//!
//! 管理运行时 peer 发现、连接类型识别和统一设备查询。
//! [`DeviceManager`] 维护 peer 列表并提供统一的设备输出接口。

pub mod manager;
mod utils;

pub use manager::{DeviceFilter, DeviceManager};

use serde::{Deserialize, Serialize};

/// 连接类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionType {
    Lan,
    Dcutr,
    Relay,
}

/// 设备状态
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeviceStatus {
    Online,
    Offline,
}

/// 统一的设备输出类型（发送给前端）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub peer_id: String,
    pub name: Option<String>,
    pub hostname: String,
    pub os: String,
    pub platform: String,
    pub arch: String,
    pub status: DeviceStatus,
    pub connection: Option<ConnectionType>,
    pub latency: Option<u64>,
    pub is_paired: bool,
    pub paired_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_seen: Option<chrono::DateTime<chrono::Utc>>,
}

/// 设备列表查询结果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceListResult {
    pub devices: Vec<Device>,
    pub total: usize,
}
