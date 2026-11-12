use std::{
    collections::HashMap,
    future::Future,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Context, Result};
use bytes::Bytes;
use tokio::{
    net::{TcpListener, TcpStream, UdpSocket},
    signal::unix::{signal, SignalKind},
    sync::mpsc,
    time::{interval, sleep, Instant},
};

use crate::{
    config::Config,
    controllers::{ControlController, RelayController, RendezvousController, Reply, Target},
    models::{PeerStore, ServerKeys},
    net::{self, normalize, Conn},
    views,
};

const SESSION_IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const EXPIRY_SWEEP_INTERVAL: Duration = Duration::from_secs(5);
const DATABASE_FILE: &str = "db_v2.sqlite3";
const MAX_DATAGRAM: usize = 64 * 1024;

type Outbox = mpsc::UnboundedSender<Bytes>;

struct Hub {
    rendezvous: RendezvousController,
    relay: RelayController,
    control: ControlController,
    udp: UdpSocket,
    waiting: Mutex<HashMap<SocketAddr, Outbox>>,
    trust_proxy_headers: bool,
}

pub async fn run(config: Config) -> Result<()> {
    let config = Arc::new(config);
    std::fs::create_dir_all(&config.data_dir).context("failed to create data directory")?;
    let keys = ServerKeys::load_or_create(&config.data_dir)?;
    log::info!("Public key: {}", keys.public_key());
    let access_key = if config.encrypted_only { keys.public_key().to_owned() } else { String::new() };
    let peers = Arc::new(PeerStore::open(&config.data_dir.join(DATABASE_FILE))?);

    let udp = net::bind_udp(config.bind, config.port).context("failed to bind rendezvous UDP port")?;
    let hub = Arc::new(Hub {
        udp,
        waiting: Mutex::default(),
        trust_proxy_headers: config.trust_proxy_headers,
        relay: RelayController::new(access_key.clone(), peers.clone()),
        rendezvous: RendezvousController::new(config.clone(), access_key, peers, keys),
        control: ControlController::new(config.control_secret.clone()),
    });

    for (port, websocket) in [(config.nat_test_port(), false), (config.port, false), (config.websocket_port(), true)] {
        let hub = hub.clone();
        listen(&config, port, "rendezvous", websocket, move |stream, addr| {
            let hub = hub.clone();
            async move { hub.rendezvous_session(stream, addr, websocket).await }
        })?;
    }
    for (port, websocket) in [(config.relay_port, false), (config.relay_websocket_port(), true)] {
        let hub = hub.clone();
        listen(&config, port, "relay", websocket, move |stream, addr| {
            let hub = hub.clone();
            async move { hub.relay_session(stream, addr, websocket).await }
        })?;
    }
    {
        let hub = hub.clone();
        listen(&config, config.control_port, "control (http+ws)", false, move |stream, _addr| {
            let hub = hub.clone();
            async move { hub.control.serve(stream, &hub.rendezvous, &hub.relay).await }
        })?;
    }
    {
        let hub = hub.clone();
        tokio::spawn(async move {
            let mut ticker = interval(EXPIRY_SWEEP_INTERVAL);
            loop {
                ticker.tick().await;
                hub.rendezvous.peers().sweep_expired();
            }
        });
    }

    tokio::select! {
        result = hub.udp_loop() => result,
        _ = shutdown_signal() => {
            log::info!("Shutting down");
            Ok(())
        }
    }
}

fn listen<F, Fut>(config: &Config, port: u16, name: &str, websocket: bool, handler: F) -> Result<()>
where
    F: Fn(TcpStream, SocketAddr) -> Fut + Send + 'static,
    Fut: Future<Output = Result<()>> + Send + 'static,
{
    let listener = net::bind_tcp(config.bind, port).with_context(|| format!("failed to bind {name} port {port}"))?;
    log::info!("Listening for {name} on tcp {port}{}", if websocket { " (websocket)" } else { "" });
    tokio::spawn(accept_loop(listener, name.to_owned(), handler));
    Ok(())
}

async fn accept_loop<F, Fut>(listener: TcpListener, name: String, handler: F)
where
    F: Fn(TcpStream, SocketAddr) -> Fut + Send + 'static,
    Fut: Future<Output = Result<()>> + Send + 'static,
{
    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                let _ = stream.set_nodelay(true);
                let session = handler(stream, addr);
                tokio::spawn(async move {
                    if let Err(error) = session.await {
                        log::debug!("session from {addr} ended: {error}");
                    }
                });
            }
            Err(error) => {
                log::error!("{name} accept failed: {error}");
                sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

impl Hub {
    async fn udp_loop(&self) -> Result<()> {
        let mut buffer = vec![0u8; MAX_DATAGRAM];
        loop {
            let (len, addr) = match self.udp.recv_from(&mut buffer).await {
                Ok(received) => received,
                Err(error) => {
                    log::error!("udp receive failed: {error}");
                    sleep(Duration::from_millis(100)).await;
                    continue;
                }
            };
            if let Some(reply) = self.rendezvous.handle_udp(&buffer[..len], addr) {
                self.send_udp(&reply, addr).await;
            }
        }
    }

    async fn rendezvous_session(&self, stream: TcpStream, addr: SocketAddr, websocket: bool) -> Result<()> {
        let (mut conn, addr) = Conn::accept(stream, addr, websocket, self.trust_proxy_headers).await?;
        let key = normalize(addr);
        let (outbox, mut inbox) = mpsc::unbounded_channel();
        self.waiting().insert(key, outbox.clone());

        let result = self.pump(&mut conn, &mut inbox, addr, websocket).await;

        let mut waiting = self.waiting();
        if waiting.get(&key).is_some_and(|registered| registered.same_channel(&outbox)) {
            waiting.remove(&key);
        }
        result
    }

    async fn pump(
        &self,
        conn: &mut Conn,
        inbox: &mut mpsc::UnboundedReceiver<Bytes>,
        addr: SocketAddr,
        websocket: bool,
    ) -> Result<()> {
        let idle = sleep(SESSION_IDLE_TIMEOUT);
        tokio::pin!(idle);
        loop {
            tokio::select! {
                incoming = conn.recv() => {
                    let Some(Ok(bytes)) = incoming else { return Ok(()) };
                    let outcome = self.rendezvous.handle_tcp(&bytes, addr, websocket);
                    if let Some(reply) = outcome.reply {
                        self.deliver(reply, conn).await?;
                    }
                    if !outcome.keep_open {
                        return Ok(());
                    }
                    idle.as_mut().reset(Instant::now() + SESSION_IDLE_TIMEOUT);
                }
                Some(bytes) = inbox.recv() => conn.send(bytes).await?,
                _ = &mut idle => return Ok(()),
            }
        }
    }

    async fn relay_session(&self, stream: TcpStream, addr: SocketAddr, websocket: bool) -> Result<()> {
        let (conn, addr) = Conn::accept(stream, addr, websocket, self.trust_proxy_headers).await?;
        self.relay.serve(conn, addr).await;
        Ok(())
    }

    async fn deliver(&self, reply: Reply, caller: &mut Conn) -> Result<()> {
        match reply.target {
            Target::Caller => caller.send(views::encode(&reply.message)).await?,
            Target::Udp(addr) => self.send_udp(&reply.message, addr).await,
            Target::Waiting(addr) => {
                if let Some(outbox) = self.waiting().remove(&normalize(addr)) {
                    let _ = outbox.send(views::encode(&reply.message));
                }
            }
        }
        Ok(())
    }

    async fn send_udp(&self, message: &crate::proto::rendezvous::RendezvousMessage, addr: SocketAddr) {
        if let Err(error) = self.udp.send_to(&views::encode(message), addr).await {
            log::debug!("udp send to {addr} failed: {error}");
        }
    }

    fn waiting(&self) -> std::sync::MutexGuard<'_, HashMap<SocketAddr, Outbox>> {
        self.waiting.lock().unwrap_or_else(|e| e.into_inner())
    }
}

async fn shutdown_signal() {
    let (Ok(mut terminate), Ok(mut interrupt)) =
        (signal(SignalKind::terminate()), signal(SignalKind::interrupt()))
    else {
        return std::future::pending().await;
    };
    tokio::select! {
        _ = terminate.recv() => {}
        _ = interrupt.recv() => {}
    }
}
