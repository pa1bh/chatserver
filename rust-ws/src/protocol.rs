use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum Incoming {
    #[serde(rename = "chat")]
    Chat { text: String },
    #[serde(rename = "setName")]
    SetName { name: String },
    #[serde(rename = "status")]
    Status,
    #[serde(rename = "listUsers")]
    ListUsers,
    #[serde(rename = "ping")]
    Ping { token: Option<String> },
    #[serde(rename = "ai")]
    Ai { prompt: String },
}

#[derive(Debug, Serialize, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Outgoing {
    #[serde(rename = "chat")]
    Chat {
        from: String,
        text: String,
        at: u128,
    },
    #[serde(rename = "system")]
    System { text: String, at: u128 },
    #[serde(rename = "ackName")]
    AckName { name: String, at: u128 },
    #[serde(rename = "status")]
    Status {
        version: &'static str,
        #[serde(rename = "rustVersion")]
        rust_version: &'static str,
        os: &'static str,
        #[serde(rename = "cpuCores")]
        cpu_cores: usize,
        #[serde(rename = "uptimeSeconds")]
        uptime_seconds: u64,
        #[serde(rename = "userCount")]
        user_count: usize,
        #[serde(rename = "peakUsers")]
        peak_users: usize,
        #[serde(rename = "connectionsTotal")]
        connections_total: u64,
        #[serde(rename = "messagesSent")]
        messages_sent: u64,
        #[serde(rename = "messagesPerSecond")]
        messages_per_second: f64,
        #[serde(rename = "memoryMb")]
        memory_mb: f64,
        #[serde(rename = "aiEnabled")]
        ai_enabled: bool,
        #[serde(rename = "aiModel", skip_serializing_if = "Option::is_none")]
        ai_model: Option<String>,
    },
    #[serde(rename = "listUsers")]
    ListUsers { users: Vec<UserInfo> },
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "pong")]
    Pong { token: Option<String>, at: u128 },
    #[serde(rename = "ai")]
    Ai {
        from: String,
        prompt: String,
        response: String,
        #[serde(rename = "responseMs")]
        response_ms: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        tokens: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        cost: Option<f64>,
        at: u128,
    },
}

#[derive(Debug, Serialize, Clone)]
pub struct UserInfo {
    pub id: String,
    pub name: String,
    pub ip: String,
}

impl Outgoing {
    pub fn kind(&self) -> &'static str {
        match self {
            Outgoing::Chat { .. } => "chat",
            Outgoing::System { .. } => "system",
            Outgoing::AckName { .. } => "ackName",
            Outgoing::Status { .. } => "status",
            Outgoing::ListUsers { .. } => "listUsers",
            Outgoing::Error { .. } => "error",
            Outgoing::Pong { .. } => "pong",
            Outgoing::Ai { .. } => "ai",
        }
    }
}
