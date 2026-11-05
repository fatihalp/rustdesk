use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    path::Path,
    sync::{RwLock, RwLockReadGuard, RwLockWriteGuard},
    time::{Duration, Instant},
};

use anyhow::Result;
use bytes::Bytes;
use serde_json::{json, Value};
use tokio::sync::broadcast;

use super::repository::PeerRepository;
use crate::net::{host_of, normalize};

const ONLINE_TIMEOUT: Duration = Duration::from_secs(30);
const KEY_REGISTRATION_WINDOW: Duration = Duration::from_secs(6);
const MAX_KEY_REGISTRATIONS_PER_WINDOW: u32 = 2;

/// A registered remote-desktop endpoint.
#[derive(Clone)]
pub struct Peer {
    /// Last UDP address the peer registered from (port 0 until it registers after a restart).
    pub addr: SocketAddr,
    pub pk: Bytes,
    uuid: Bytes,
    ip: String,
    last_seen: Option<Instant>,
    online_notified: bool,
    key_registrations: u32,
    key_window_start: Instant,
}

impl Peer {
    fn new() -> Self {
        Self {
            addr: SocketAddr::from(([0, 0, 0, 0], 0)),
            pk: Bytes::new(),
            uuid: Bytes::new(),
            ip: String::new(),
            last_seen: None,
            online_notified: false,
            key_registrations: 0,
            key_window_start: Instant::now(),
        }
    }

    pub fn is_online(&self) -> bool {
        self.last_seen.is_some_and(|seen| seen.elapsed() < ONLINE_TIMEOUT)
    }

    pub fn idle_ms(&self) -> Option<u64> {
        self.last_seen.map(|seen| seen.elapsed().as_millis() as u64)
    }
}

pub enum PkRegistration {
    Registered,
    UuidMismatch,
    TooFrequent,
    ServerError,
}

/// All known peers: live state in memory, identity persisted in SQLite, plus real-time event bus.
pub struct PeerStore {
    peers: RwLock<HashMap<String, Peer>>,
    blocked: RwLock<HashSet<String>>,
    repository: PeerRepository,
    events: broadcast::Sender<String>,
}

impl PeerStore {
    pub fn open(db_path: &Path) -> Result<Self> {
        let (events, _) = broadcast::channel(512);
        let repository = PeerRepository::open(db_path)?;
        let initial_blocked = repository.load_blocked_ids();
        Ok(Self {
            peers: RwLock::default(),
            blocked: RwLock::new(initial_blocked),
            repository,
            events,
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.events.subscribe()
    }

    pub fn emit(&self, event: Value) {
        let _ = self.events.send(event.to_string());
    }

    pub fn is_blocked(&self, id: &str) -> bool {
        self.blocked
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .contains(id)
    }

    pub fn set_blocked(&self, id: &str, block: bool) {
        let mut blocked = self.blocked.write().unwrap_or_else(|e| e.into_inner());
        if block {
            blocked.insert(id.to_owned());
            if let Some(peer) = self.write().get_mut(id) {
                peer.last_seen = None;
                peer.online_notified = false;
            }
        } else {
            blocked.remove(id);
        }
        drop(blocked);
        self.repository.set_blocked(id, block);
        self.emit(json!({
            "type": if block { "peer.blocked" } else { "peer.unblocked" },
            "id": id,
            "blocked": block,
        }));
    }

    /// Marks a peer offline immediately and notifies subscribers.
    pub fn force_offline(&self, id: &str) -> bool {
        let mut peers = self.write();
        let Some(peer) = peers.get_mut(id) else {
            return false;
        };
        peer.last_seen = None;
        peer.online_notified = false;
        let ip = peer.ip.clone();
        drop(peers);
        self.emit(json!({
            "type": "peer.offline",
            "id": id,
            "ip": ip,
        }));
        true
    }

    /// Sweeps peers whose keep-alive expired and broadcasts `peer.offline` events.
    pub fn sweep_expired(&self) {
        let mut newly_offline = Vec::new();
        {
            let mut peers = self.write();
            for (id, peer) in peers.iter_mut() {
                if peer.online_notified && !peer.is_online() {
                    peer.online_notified = false;
                    newly_offline.push((id.clone(), peer.ip.clone()));
                }
            }
        }
        for (id, ip) in newly_offline {
            self.emit(json!({
                "type": "peer.offline",
                "id": id,
                "ip": ip,
            }));
        }
    }

    /// Snapshot of all in-memory peers and blocked IDs for the management API.
    pub fn snapshot(&self) -> Vec<Value> {
        let blocked = self.blocked.read().unwrap_or_else(|e| e.into_inner());
        let peers = self.read();
        let mut result: Vec<Value> = peers
            .iter()
            .map(|(id, peer)| {
                let is_blocked = blocked.contains(id);
                json!({
                    "id": id,
                    "ip": if peer.ip.is_empty() { host_of(peer.addr) } else { peer.ip.clone() },
                    "addr": peer.addr.to_string(),
                    "online": !is_blocked && peer.is_online(),
                    "blocked": is_blocked,
                    "idle_ms": peer.idle_ms(),
                })
            })
            .collect();
        for id in blocked.iter() {
            if !peers.contains_key(id) {
                result.push(json!({
                    "id": id,
                    "ip": "",
                    "addr": "",
                    "online": false,
                    "blocked": true,
                    "idle_ms": null,
                }));
            }
        }
        result
    }

    /// Records a keep-alive. Returns whether the peer must (re)send its public key.
    pub fn heartbeat(&self, id: &str, addr: SocketAddr) -> bool {
        if self.is_blocked(id) {
            return false;
        }
        let mut peers = self.write();
        let Some(peer) = peers.get_mut(id) else {
            return true;
        };
        let ip = normalize(addr).ip();
        let known_ip_differs = if peer.addr.port() != 0 {
            normalize(peer.addr).ip() != ip
        } else {
            host_of(addr) != peer.ip
        };
        let ip_changed = known_ip_differs && !ip.is_loopback();
        let request_pk = peer.pk.is_empty() || ip_changed;
        if ip_changed {
            log::info!("IP change of {id} from {} to {addr}", peer.addr);
        }
        if !request_pk {
            let became_online = !peer.is_online() || !peer.online_notified;
            peer.addr = addr;
            peer.ip = host_of(addr);
            peer.last_seen = Some(Instant::now());
            peer.online_notified = true;
            let peer_ip = peer.ip.clone();
            drop(peers);
            if became_online {
                self.emit(json!({
                    "type": "peer.online",
                    "id": id,
                    "ip": peer_ip,
                    "addr": addr.to_string(),
                }));
            }
        }
        request_pk
    }

    /// Registers or refreshes a peer's public key, rejecting identity theft and floods.
    pub fn register_pk(&self, id: &str, addr: SocketAddr, uuid: Bytes, pk: Bytes) -> PkRegistration {
        if self.is_blocked(id) {
            return PkRegistration::UuidMismatch;
        }
        let ip = host_of(addr);
        let preloaded = if self.read().contains_key(id) {
            None
        } else {
            self.load(id)
        };
        let mut peers = self.write();
        let peer = peers
            .entry(id.to_owned())
            .or_insert_with(|| preloaded.unwrap_or_else(Peer::new));

        let is_new = peer.uuid.is_empty();
        let mut changed = is_new;
        if !is_new {
            if peer.uuid != uuid || (peer.ip != ip && peer.pk != pk) {
                log::warn!("Peer {id} uuid or ip/key mismatch from {ip}");
                return PkRegistration::UuidMismatch;
            }
            changed = peer.pk != pk || peer.ip != ip;
        }

        if peer.key_window_start.elapsed() > KEY_REGISTRATION_WINDOW {
            peer.key_registrations = 0;
        } else if peer.key_registrations > MAX_KEY_REGISTRATIONS_PER_WINDOW {
            return PkRegistration::TooFrequent;
        }
        peer.key_registrations += 1;
        peer.key_window_start = Instant::now();

        let became_online = !peer.is_online() || !peer.online_notified;
        peer.addr = addr;
        peer.last_seen = Some(Instant::now());
        peer.online_notified = true;
        if changed {
            peer.uuid = uuid;
            peer.pk = pk;
            peer.ip = ip.clone();
            let saved_uuid = peer.uuid.clone();
            let saved_pk = peer.pk.clone();
            drop(peers);
            if let Err(error) = self.repository.save(id, &saved_uuid, &saved_pk, &ip) {
                log::error!("failed to persist peer {id}: {error}");
                return PkRegistration::ServerError;
            }
            log::info!("Registered key of peer {id} from {addr}");
        } else {
            drop(peers);
        }
        if became_online || changed {
            self.emit(json!({
                "type": "peer.online",
                "id": id,
                "ip": ip,
                "addr": addr.to_string(),
            }));
        }
        PkRegistration::Registered
    }

    /// Looks a peer up in memory first, then loads from SQLite without holding the memory lock.
    pub fn find(&self, id: &str) -> Option<Peer> {
        if self.is_blocked(id) {
            return None;
        }
        if let Some(peer) = self.read().get(id) {
            return Some(peer.clone());
        }
        let peer = self.load(id)?;
        let mut peers = self.write();
        Some(peers.entry(id.to_owned()).or_insert(peer).clone())
    }

    pub fn is_online(&self, id: &str) -> bool {
        !self.is_blocked(id) && self.read().get(id).is_some_and(Peer::is_online)
    }

    fn load(&self, id: &str) -> Option<Peer> {
        let stored = self.repository.find(id).ok().flatten()?;
        if stored.blocked {
            self.blocked
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .insert(id.to_owned());
        }
        Some(Peer {
            uuid: stored.uuid.into(),
            pk: stored.pk.into(),
            ip: stored.ip,
            ..Peer::new()
        })
    }

    fn read(&self) -> RwLockReadGuard<'_, HashMap<String, Peer>> {
        self.peers.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write(&self) -> RwLockWriteGuard<'_, HashMap<String, Peer>> {
        self.peers.write().unwrap_or_else(|e| e.into_inner())
    }
}
