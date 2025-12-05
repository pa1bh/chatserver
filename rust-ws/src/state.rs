use std::{
    sync::{atomic::AtomicU64, Arc},
    time::{Instant, SystemTime},
};

use axum::extract::ws::Message;
use dashmap::DashMap;
use sysinfo::{ProcessesToUpdate, System};
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

use crate::protocol::{Outgoing, UserInfo};

pub type Clients = Arc<DashMap<Uuid, Client>>;

#[derive(Clone)]
pub struct AppState {
    pub clients: Clients,
    pub started_at: Instant,
    pub messages_sent: Arc<AtomicU64>,
    pub system_info: Arc<RwLock<System>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(DashMap::new()),
            started_at: Instant::now(),
            messages_sent: Arc::new(AtomicU64::new(0)),
            system_info: Arc::new(RwLock::new(System::new())),
        }
    }

    pub fn user_count(&self) -> usize {
        self.clients.len()
    }

    pub fn uptime_seconds(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }

    pub fn messages_sent(&self) -> u64 {
        self.messages_sent.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn increment_messages(&self) {
        self.messages_sent.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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
    pub tx: mpsc::UnboundedSender<Message>,
    #[allow(dead_code)]
    pub connected_at: SystemTime,
}

impl Client {
    pub fn new(name: String, ip: String, tx: mpsc::UnboundedSender<Message>) -> Self {
        Self {
            name,
            ip,
            tx,
            connected_at: SystemTime::now(),
        }
    }

    pub fn send(&self, payload: &Outgoing) -> bool {
        if let Ok(text) = serde_json::to_string(payload) {
            self.tx.send(Message::Text(text.into())).is_ok()
        } else {
            false
        }
    }
}
