use std::{
    collections::VecDeque,
    sync::{atomic::AtomicU64, Arc, Mutex},
    time::{Instant, SystemTime},
};

use axum::extract::ws::Message;
use dashmap::DashMap;
use sysinfo::{ProcessesToUpdate, System};
use tokio::sync::{mpsc, RwLock};
use tracing::info;
use uuid::Uuid;

use crate::ai::AiClient;
use crate::protocol::{Outgoing, UserInfo};

#[derive(Clone)]
pub struct RateLimitConfig {
    pub enabled: bool,
    pub messages_per_minute: u32,
}

impl RateLimitConfig {
    pub fn from_env() -> Self {
        let enabled = std::env::var("RATE_LIMIT_ENABLED")
            .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
            .unwrap_or(false);
        let messages_per_minute = std::env::var("RATE_LIMIT_MSG_PER_MIN")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);

        if enabled {
            info!(messages_per_minute, "Rate limiting enabled");
        }

        Self {
            enabled,
            messages_per_minute,
        }
    }
}

pub type Clients = Arc<DashMap<Uuid, Client>>;

#[derive(Clone)]
pub struct AppState {
    pub clients: Clients,
    pub started_at: Instant,
    pub messages_sent: Arc<AtomicU64>,
    pub connections_total: Arc<AtomicU64>,
    pub peak_users: Arc<AtomicU64>,
    pub system_info: Arc<RwLock<System>>,
    pub ai: Arc<AiClient>,
    pub rate_limit: RateLimitConfig,
}

impl AppState {
    pub fn new(ai_client: AiClient, rate_limit: RateLimitConfig) -> Self {
        Self {
            clients: Arc::new(DashMap::new()),
            started_at: Instant::now(),
            messages_sent: Arc::new(AtomicU64::new(0)),
            connections_total: Arc::new(AtomicU64::new(0)),
            peak_users: Arc::new(AtomicU64::new(0)),
            system_info: Arc::new(RwLock::new(System::new())),
            ai: Arc::new(ai_client),
            rate_limit,
        }
    }

    pub fn user_count(&self) -> usize {
        self.clients.len()
    }

    pub fn uptime_seconds(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }

    pub fn messages_sent(&self) -> u64 {
        self.messages_sent
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn increment_messages(&self) {
        self.messages_sent
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn increment_connections(&self) {
        self.connections_total
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Update peak users if current count is higher
        let current = self.clients.len() as u64;
        self.peak_users
            .fetch_max(current, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn connections_total(&self) -> u64 {
        self.connections_total
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn peak_users(&self) -> u64 {
        self.peak_users.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub async fn memory_mb(&self) -> f64 {
        let mut sys = self.system_info.write().await;
        let pid = sysinfo::Pid::from_u32(std::process::id());
        sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
        sys.process(pid)
            .map(|p| p.memory() as f64 / 1024.0 / 1024.0)
            .unwrap_or(0.0)
    }

    pub fn list_users(&self) -> Vec<UserInfo> {
        self.clients
            .iter()
            .map(|entry| UserInfo {
                id: entry.key().to_string(),
                name: entry.value().name.clone(),
                ip: entry.value().ip.clone(),
            })
            .collect()
    }
}

#[derive(Clone)]
pub struct Client {
    pub name: String,
    pub ip: String,
    pub tx: mpsc::Sender<Message>,
    #[allow(dead_code)]
    pub connected_at: SystemTime,
    /// Timestamps of recent messages for rate limiting (sliding window)
    pub message_timestamps: Arc<Mutex<VecDeque<Instant>>>,
}

impl Client {
    pub fn new(name: String, ip: String, tx: mpsc::Sender<Message>) -> Self {
        Self {
            name,
            ip,
            tx,
            connected_at: SystemTime::now(),
            message_timestamps: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Check if this client is rate limited. Returns Ok(()) if allowed, Err with seconds until next allowed message if rate limited.
    pub fn check_rate_limit(&self, config: &RateLimitConfig) -> Result<(), u64> {
        if !config.enabled {
            return Ok(());
        }

        let mut timestamps = self.message_timestamps.lock().unwrap();
        let now = Instant::now();
        let window = std::time::Duration::from_secs(60);

        // Remove timestamps older than 1 minute
        while let Some(front) = timestamps.front() {
            if now.duration_since(*front) > window {
                timestamps.pop_front();
            } else {
                break;
            }
        }

        if timestamps.len() >= config.messages_per_minute as usize {
            // Calculate how long until the oldest message expires
            if let Some(oldest) = timestamps.front() {
                let elapsed = now.duration_since(*oldest);
                let wait_secs = (window.as_secs()).saturating_sub(elapsed.as_secs());
                return Err(wait_secs.max(1));
            }
        }

        // Record this message
        timestamps.push_back(now);
        Ok(())
    }

    /// Send a message to this client. Uses try_send to avoid blocking.
    /// Returns false if the client's buffer is full (slow client) or channel closed.
    pub fn send(&self, payload: &Outgoing) -> bool {
        if let Ok(text) = serde_json::to_string(payload) {
            self.tx.try_send(Message::Text(text.into())).is_ok()
        } else {
            false
        }
    }
}
