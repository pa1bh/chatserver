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
}

#[derive(Debug, Serialize, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Outgoing {
    #[serde(rename = "chat")]
    Chat { from: String, text: String, at: u128 },
    #[serde(rename = "system")]
    System { text: String, at: u128 },
    #[serde(rename = "ackName")]
    AckName { name: String, at: u128 },
    #[serde(rename = "status")]
    Status {
        #[serde(rename = "uptimeSeconds")]
        uptime_seconds: u64,
        #[serde(rename = "userCount")]
        user_count: usize,
        #[serde(rename = "messagesSent")]
        messages_sent: u64,
        #[serde(rename = "messagesPerSecond")]
        messages_per_second: f64,
        #[serde(rename = "memoryMb")]
        memory_mb: f64,
    },
    #[serde(rename = "listUsers")]
    ListUsers { users: Vec<UserInfo> },
    #[serde(rename = "error")]
    Error { message: String },
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
        }
    }
}
