use super::connection::{Connection, Sender};
use crate::port_forward_mux::{
    charge, close_msg, effective_window, opened_msg, run_channel, FrameSink, Inbound, RecvWindow,
    SendCredit, CHANNEL_WINDOW, INITIAL_WINDOW, MAX_CHANNELS,
};
use hbb_common::{
    bytes::Bytes,
    log,
    timeout,
    tokio::{self, net::TcpStream, sync::{mpsc, watch}},
};
use base::message_proto::*;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

const CONNECT_TIMEOUT_MS: u64 = 3000;

fn pending_fits(charged: usize, add_len: usize) -> bool {
    charged.saturating_add(charge(add_len) as usize) <= INITIAL_WINDOW as usize
}

struct Entry {
    inbound: mpsc::UnboundedSender<Inbound>,
    credit: Arc<SendCredit>,
    window: Arc<Mutex<RecvWindow>>,
}

pub struct PortForwardMux {
    channels: HashMap<i32, Entry>,
    tx: Sender,
    login_target: String,
    teardown: watch::Sender<bool>,
}

impl PortForwardMux {
    pub fn new(tx: Sender, login_target: String) -> Self {
        Self {
            channels: HashMap::new(),
            tx,
            login_target,
            teardown: watch::channel(false).0,
        }
    }

    pub fn handle(&mut self, frame: PortForwardChannel, tunnel_permitted: impl FnOnce() -> bool) {
        match frame.union {
            Some(port_forward_channel::Union::Open(open)) => {
                let permitted = tunnel_permitted();
                self.on_open(open, permitted)
            }
            Some(port_forward_channel::Union::Data(d)) => {
                let len = d.data.len();
                let Some(entry) = self.channels.get(&d.channel_id) else {
                    log::debug!("port forward data for unknown channel {}", d.channel_id);
                    return;
                };
                let accepted = entry.window.lock().unwrap().accept(len);
                let delivered = accepted && entry.inbound.send(Inbound::Data(d.data)).is_ok();
                if delivered {
                    return;
                }
                let Some(entry) = self.channels.remove(&d.channel_id) else {
                    return;
                };
                if !accepted {
                    log::warn!("port forward channel {} overran its window", d.channel_id);
                    entry.inbound.send(Inbound::Violation).ok();
                }
            }
            Some(port_forward_channel::Union::Close(c)) => {
                if let Some(entry) = self.channels.remove(&c.channel_id) {
                    entry.inbound.send(Inbound::Close).ok();
                } else {
                    log::debug!("port forward close for unknown channel {}", c.channel_id);
                }
            }
            Some(port_forward_channel::Union::WindowUpdate(u)) => {
                match self.channels.get(&u.channel_id) {
                    Some(entry) => entry.credit.add(u.add),
                    None => log::debug!(
                        "port forward window update for unknown channel {}",
                        u.channel_id
                    ),
                }
            }
            Some(port_forward_channel::Union::Opened(o)) => {
                log::debug!("ignoring opened for channel {} on the controlled side", o.channel_id);
            }
            _ => {}
        }
    }

    fn on_open(&mut self, open: PortForwardOpen, permitted: bool) {
        let id = open.channel_id;
        self.channels.retain(|_, e| !e.inbound.is_closed());
        if !permitted {
            self.reply(opened_msg(id, false, "No permission of IP tunneling", 0));
            return;
        }
        if self.channels.len() >= MAX_CHANNELS {
            self.reply(opened_msg(id, false, "Too many port forward channels", 0));
            return;
        }
        if self.channels.contains_key(&id) {
            log::debug!("ignoring open for live channel {}", id);
            return;
        }
        let mut pf = PortForward {
            host: open.host,
            port: open.port,
            ..Default::default()
        };
        let (addr, is_rdp) = Connection::normalize_port_forward_target(&mut pf);
        if addr != self.login_target {
            log::warn!(
                "port forward channel {} asked for {} on a tunnel logged in for {}",
                id,
                addr,
                self.login_target
            );
            self.reply(opened_msg(id, false, "Port forward target not authorized", 0));
            return;
        }
        let (inbound_tx, inbound_rx) = mpsc::unbounded_channel();
        let credit = Arc::new(SendCredit::new(effective_window(open.window)));
        let window = Arc::new(Mutex::new(RecvWindow::new(INITIAL_WINDOW)));
        self.channels.insert(
            id,
            Entry {
                inbound: inbound_tx,
                credit: credit.clone(),
                window: window.clone(),
            },
        );
        tokio::spawn(run_controlled_channel(
            id,
            addr,
            is_rdp,
            credit,
            window,
            inbound_rx,
            FrameSink::Direct(self.tx.clone()),
            self.teardown.subscribe(),
        ));
    }

    fn reply(&self, msg: Message) {
        self.tx
            .send((tokio::time::Instant::now(), Arc::new(msg)))
            .ok();
    }

    pub fn close_all(&mut self) {
        self.channels.clear();
        self.teardown.send_replace(true);
    }
}

async fn run_controlled_channel(
    id: i32,
    addr: String,
    is_rdp: bool,
    credit: Arc<SendCredit>,
    window: Arc<Mutex<RecvWindow>>,
    mut inbound: mpsc::UnboundedReceiver<Inbound>,
    sink: FrameSink,
    teardown: watch::Receiver<bool>,
) {
    let mut pending: Vec<Bytes> = Vec::new();
    let mut pending_len = 0usize;
    let connect = timeout(CONNECT_TIMEOUT_MS, TcpStream::connect(&addr));
    tokio::pin!(connect);
    let socket = loop {
        tokio::select! {
            biased;
            cmd = inbound.recv() => match cmd {
                Some(Inbound::Data(b)) => {
                    if !pending_fits(pending_len, b.len()) {
                        log::warn!("port forward channel {} sent more than INITIAL_WINDOW before opened", id);
                        sink.send_ordered(close_msg(id)).await.ok();
                        return;
                    }
                    pending_len += charge(b.len()) as usize;
                    pending.push(b);
                }
                Some(Inbound::Close) | None => return,
                Some(Inbound::Violation) => {
                    sink.send_ordered(close_msg(id)).await.ok();
                    return;
                }
            },
            res = &mut connect => {
                let err = match res {
                    Ok(Ok(s)) => break s,
                    Ok(Err(e)) => e.to_string(),
                    Err(e) => e.to_string(),
                };
                log::debug!("port forward channel {} connect {} failed: {}", id, addr, err);
                sink.send_ordered(opened_msg(id, false, &unreachable_message(&addr, is_rdp), 0)).await.ok();
                return;
            }
        }
    };
    window.lock().unwrap().grant(CHANNEL_WINDOW - INITIAL_WINDOW);
    if sink
        .send_ordered(opened_msg(id, true, "", CHANNEL_WINDOW))
        .await
        .is_err()
    {
        return;
    }
    let (reader, writer) = socket.into_split();
    run_channel(id, reader, writer, Vec::new(), pending, credit, window, inbound, sink, teardown).await;
}

fn unreachable_message(addr: &str, is_rdp: bool) -> String {
    format!(
        "Failed to access remote {}. Please make sure it is reachable/open.",
        if is_rdp { "RDP" } else { addr }
    )
}
