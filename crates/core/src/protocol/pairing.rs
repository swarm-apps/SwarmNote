//! Pairing sub-protocol — exchange of device pairing codes.

use serde::{Deserialize, Serialize};

use super::OsInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingRequest {
    pub os_info: OsInfo,
    pub timestamp: i64,
    pub method: PairingMethod,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PairingMethod {
    Code { code: String },
    Direct,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum PairingResponse {
    Success,
    Refused { reason: PairingRefuseReason },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PairingRefuseReason {
    UserRejected,
    CodeExpired,
    CodeInvalid,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_serde_tag() {
        let method = PairingMethod::Code {
            code: "123456".to_string(),
        };
        let json = serde_json::to_string(&method).unwrap();
        assert!(json.contains("\"type\":\"Code\""));
        assert!(json.contains("\"code\":\"123456\""));

        let direct = PairingMethod::Direct;
        let json = serde_json::to_string(&direct).unwrap();
        assert!(json.contains("\"type\":\"Direct\""));
    }
}
