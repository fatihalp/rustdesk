use std::{
    collections::HashMap,
    io,
    net::SocketAddr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use protobuf::Message;
use serde_json::{json, Value};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::tcp::{OwnedReadHalf, OwnedWriteHalf},
    sync::{oneshot, watch},
    time::{sleep, timeout},
};

use crate::{
    models::PeerStore,
    net::Conn,
    proto::rendezvous::{rendezvous_message::Union, RendezvousMessage},
};

const PAIRING_TIMEOUT: Duration = Duration::from_secs(30);
const IDLE_TIMEOUT: Duration = Duration::from_secs(300);
const MAX_PENDING: usize = 10_000;
const COPY_BUFFER: usize = 32 * 1024;

struct ActiveSession {
    peer_id: String,
    client_addr: String,
    started_at: Instant,
    bytes: Arc<AtomicU64>,
    cancel: watch::Sender<bool>,
}

pub struct RelayController {
    access_key: String,
    peers: Arc<PeerStore>,
    pending: Mutex<HashMap<String, (String, oneshot::Sender<(Conn, String)>)>>,
    active: Mutex<HashMap<String, ActiveSession>>,
}

impl RelayController {
    pub fn new(access_key: String, peers: Arc<PeerStore>) -> Self {
        Self {
            access_key,
            peers,
            pending: Mutex::default(),
            active: Mutex::default(),
        }
    }

    pub fn active_sessions(&self) -> Vec<Value> {
        self.active
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .map(|(uuid, s)| {
                json!({
                    "uuid": uuid,
                    "peer_id": s.peer_id,
                    "client_addr": s.client_addr,
                    "duration_secs": s.started_at.elapsed().as_secs(),
                    "bytes": s.bytes.load(Ordering::Relaxed),
                })
            })
            .collect()
    }

    pub fn disconnect(&self, target: &str) -> usize {
        let mut active = self.active.lock().unwrap_or_else(|e| e.into_inner());
        let mut count = 0;
        for (uuid, session) in active.iter() {
            if session.peer_id == target || uuid == target {
                let _ = session.cancel.send(true);
                count += 1;
            }
        }
        active.retain(|uuid, session| session.peer_id != target && uuid != target);
        count
    }

    pub async fn serve(&self, mut conn: Conn, addr: SocketAddr) {
        let Ok(Some(Ok(first))) = timeout(PAIRING_TIMEOUT, conn.recv()).await else {
            return;
        };
        let Some(Union::RequestRelay(request)) = RendezvousMessage::parse_from_bytes(&first)
            .ok()
            .and_then(|message| message.union)
        else {
            return;
        };
        if !self.access_key.is_empty() && request.licence_key != self.access_key {
            log::warn!("Relay request from {addr} rejected: invalid key");
            return;
        }
        if request.uuid.is_empty() || (!request.id.is_empty() && self.peers.is_blocked(&request.id)) {
            return;
        }

        if let Some((peer_id, partner)) = self.take_pending(&request.uuid) {
            if !peer_id.is_empty() && self.peers.is_blocked(&peer_id) {
                return;
            }
            let _ = partner.send((conn, request.id));
            return;
        }
        let Some((paired, partner_id)) = self.wait_for_partner(&request.uuid, &request.id).await else {
            return;
        };
        let peer_id = if request.id.is_empty() { partner_id } else { request.id.clone() };
        if !peer_id.is_empty() && self.peers.is_blocked(&peer_id) {
            return;
        }

        let (cancel_tx, cancel_rx) = watch::channel(false);
        let bytes = Arc::new(AtomicU64::new(0));
        self.active.lock().unwrap_or_else(|e| e.into_inner()).insert(
            request.uuid.clone(),
            ActiveSession {
                peer_id: peer_id.clone(),
                client_addr: addr.to_string(),
                started_at: Instant::now(),
                bytes: bytes.clone(),
                cancel: cancel_tx,
            },
        );
        self.peers.emit(json!({
            "type": "session.started",
            "uuid": request.uuid,
            "peer_id": peer_id,
            "addr": addr.to_string(),
        }));

        log::info!("Relay session {} paired for {addr}", request.uuid);
        match forward(conn, paired, bytes.clone(), cancel_rx).await {
            Ok(()) => log::info!("Relay session {} closed", request.uuid),
            Err(error) => log::info!("Relay session {} closed: {error}", request.uuid),
        }

        self.active.lock().unwrap_or_else(|e| e.into_inner()).remove(&request.uuid);
        self.peers.emit(json!({
            "type": "session.ended",
            "uuid": request.uuid,
            "peer_id": peer_id,
            "bytes": bytes.load(Ordering::Relaxed),
        }));
    }

    async fn wait_for_partner(&self, uuid: &str, peer_id: &str) -> Option<(Conn, String)> {
        let (sender, receiver) = oneshot::channel();
        {
            let mut pending = self.pending();
            if pending.len() >= MAX_PENDING {
                return None;
            }
            pending.insert(uuid.to_owned(), (peer_id.to_owned(), sender));
        }
        match timeout(PAIRING_TIMEOUT, receiver).await {
            Ok(Ok(partner)) => Some(partner),
            _ => {
                self.take_pending(uuid);
                None
            }
        }
    }

    fn take_pending(&self, uuid: &str) -> Option<(String, oneshot::Sender<(Conn, String)>)> {
        self.pending().remove(uuid)
    }

    fn pending(&self) -> std::sync::MutexGuard<'_, HashMap<String, (String, oneshot::Sender<(Conn, String)>)>> {
        self.pending.lock().unwrap_or_else(|e| e.into_inner())
    }
}

async fn forward(
    a: Conn,
    b: Conn,
    bytes: Arc<AtomicU64>,
    mut cancel: watch::Receiver<bool>,
) -> io::Result<()> {
    match (a, b) {
        (Conn::Tcp(a), Conn::Tcp(b)) => {
            let (a, b) = (a.into_parts(), b.into_parts());
            tokio::select! {
                res = forward_raw(a.io, a.read_buf.to_vec(), b.io, b.read_buf.to_vec(), bytes) => res,
                _ = cancel.changed() => Ok(()),
            }
        }
        (a, b) => {
            tokio::select! {
                res = forward_messages(a, b, bytes) => res,
                _ = cancel.changed() => Ok(()),
            }
        }
    }
}

async fn forward_raw(
    a: tokio::net::TcpStream,
    a_buffered: Vec<u8>,
    b: tokio::net::TcpStream,
    b_buffered: Vec<u8>,
    bytes: Arc<AtomicU64>,
) -> io::Result<()> {
    let ((a_read, mut a_write), (b_read, mut b_write)) = (a.into_split(), b.into_split());
    b_write.write_all(&a_buffered).await?;
    a_write.write_all(&b_buffered).await?;

    let activity = Activity::new(bytes);
    tokio::select! {
        result = copy(a_read, b_write, &activity) => result,
        result = copy(b_read, a_write, &activity) => result,
        _ = activity.wait_until_idle() => Err(io::ErrorKind::TimedOut.into()),
    }
}

async fn copy(mut from: OwnedReadHalf, mut to: OwnedWriteHalf, activity: &Activity) -> io::Result<()> {
    let mut buffer = vec![0u8; COPY_BUFFER];
    loop {
        let read = from.read(&mut buffer).await?;
        if read == 0 {
            return Ok(());
        }
        to.write_all(&buffer[..read]).await?;
        activity.record(read as u64);
    }
}

async fn forward_messages(mut a: Conn, mut b: Conn, bytes: Arc<AtomicU64>) -> io::Result<()> {
    let mut last_activity = Instant::now();
    loop {
        tokio::select! {
            message = a.recv() => match message {
                Some(message) => {
                    let data = message?;
                    bytes.fetch_add(data.len() as u64, Ordering::Relaxed);
                    b.send(data).await?;
                }
                None => return Ok(()),
            },
            message = b.recv() => match message {
                Some(message) => {
                    let data = message?;
                    bytes.fetch_add(data.len() as u64, Ordering::Relaxed);
                    a.send(data).await?;
                }
                None => return Ok(()),
            },
            _ = sleep(IDLE_TIMEOUT.saturating_sub(last_activity.elapsed())) => {
                return Err(io::ErrorKind::TimedOut.into());
            }
        }
        last_activity = Instant::now();
    }
}

struct Activity {
    started: Instant,
    last_millis: AtomicU64,
    bytes: Arc<AtomicU64>,
}

impl Activity {
    fn new(bytes: Arc<AtomicU64>) -> Self {
        Self {
            started: Instant::now(),
            last_millis: AtomicU64::new(0),
            bytes,
        }
    }

    fn record(&self, count: u64) {
        self.bytes.fetch_add(count, Ordering::Relaxed);
        self.last_millis
            .store(self.started.elapsed().as_millis() as u64, Ordering::Relaxed);
    }

    async fn wait_until_idle(&self) {
        loop {
            let last = Duration::from_millis(self.last_millis.load(Ordering::Relaxed));
            let idle = self.started.elapsed().saturating_sub(last);
            if idle >= IDLE_TIMEOUT {
                return;
            }
            sleep(IDLE_TIMEOUT - idle).await;
        }
    }
}
