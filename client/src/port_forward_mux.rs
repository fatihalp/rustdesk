use hbb_common::{
    bytes::Bytes,
    log,
    tokio::{
        self,
        io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
        sync::{mpsc, watch, Notify},
        time::Instant,
    },
    ResultType,
};
use base::message_proto::*;
use std::sync::{Arc, Mutex};

pub const INITIAL_WINDOW: u32 = 64 * 1024;
pub const MIN_FRAME_CHARGE: u32 = 64;
pub const CHANNEL_WINDOW: u32 = 256 * 1024;
pub const MAX_FRAME: usize = 64 * 1024;
pub const MAX_PACKET: usize = 2 * MAX_FRAME;
pub const UPDATE_THRESHOLD: u32 = CHANNEL_WINDOW / 2;
pub const MAX_CHANNELS: usize = 256;
pub const DATA_QUEUE_FRAMES: usize = 128;
pub const MAX_SEND_CREDIT: u32 = CHANNEL_WINDOW;

pub fn effective_window(advertised: u32) -> u32 {
    advertised.max(INITIAL_WINDOW)
}

pub fn cap_packet_size(stream: &mut hbb_common::Stream) {
    if let hbb_common::Stream::Tcp(s) = stream {
        s.0.codec_mut().set_max_packet_length(MAX_PACKET);
    }
}

pub fn charge(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX).max(MIN_FRAME_CHARGE)
}

pub struct RecvWindow {
    remaining: u32,
    drained_since_update: u32,
}

impl RecvWindow {
    pub fn new(granted: u32) -> Self {
        Self {
            remaining: effective_window(granted),
            drained_since_update: 0,
        }
    }

    pub fn accept(&mut self, len: usize) -> bool {
        match self.remaining.checked_sub(charge(len)) {
            Some(left) => {
                self.remaining = left;
                true
            }
            None => false,
        }
    }

    pub fn grant(&mut self, n: u32) {
        self.remaining = self.remaining.saturating_add(n);
    }

    pub fn drained(&mut self, n: usize) -> Option<u32> {
        self.drained_since_update = self.drained_since_update.saturating_add(charge(n));
        if self.drained_since_update < UPDATE_THRESHOLD {
            return None;
        }
        let add = self.drained_since_update;
        self.drained_since_update = 0;
        self.remaining = self.remaining.saturating_add(add);
        Some(add)
    }
}

pub struct SendCredit {
    credit: Mutex<u32>,
    notify: Notify,
}

impl SendCredit {
    pub fn new(initial: u32) -> Self {
        Self {
            credit: Mutex::new(initial.min(MAX_SEND_CREDIT)),
            notify: Notify::new(),
        }
    }

    pub async fn take(&self, max: usize) -> usize {
        debug_assert!(max >= MIN_FRAME_CHARGE as usize);
        loop {
            {
                let mut credit = self.credit.lock().unwrap();
                if *credit >= MIN_FRAME_CHARGE {
                    let n = (*credit as usize).min(max);
                    *credit -= n as u32;
                    return n;
                }
            }
            self.notify.notified().await;
        }
    }

    pub fn add(&self, n: u32) {
        {
            let mut credit = self.credit.lock().unwrap();
            *credit = credit.saturating_add(n).min(MAX_SEND_CREDIT);
        }
        self.notify.notify_one();
    }

    pub fn raise_initial(&self, total: u32) {
        let extra = effective_window(total) - INITIAL_WINDOW;
        if extra > 0 {
            self.add(extra);
        }
    }
}

fn channel_msg(union: port_forward_channel::Union) -> Message {
    let mut ch = PortForwardChannel::new();
    ch.union = Some(union);
    let mut msg = Message::new();
    msg.set_port_forward_channel(ch);
    msg
}

pub fn open_msg(id: i32, host: &str, port: i32, window: u32) -> Message {
    channel_msg(port_forward_channel::Union::Open(PortForwardOpen {
        channel_id: id,
        host: host.to_owned(),
        port,
        window,
        ..Default::default()
    }))
}

pub fn opened_msg(id: i32, success: bool, message: &str, window: u32) -> Message {
    channel_msg(port_forward_channel::Union::Opened(PortForwardOpened {
        channel_id: id,
        success,
        message: message.to_owned(),
        window,
        ..Default::default()
    }))
}

pub fn data_msg(id: i32, data: Bytes) -> Message {
    channel_msg(port_forward_channel::Union::Data(PortForwardData {
        channel_id: id,
        data,
        ..Default::default()
    }))
}

pub fn close_msg(id: i32) -> Message {
    channel_msg(port_forward_channel::Union::Close(PortForwardClose {
        channel_id: id,
        ..Default::default()
    }))
}

pub fn window_update_msg(id: i32, add: u32) -> Message {
    channel_msg(port_forward_channel::Union::WindowUpdate(PortForwardWindowUpdate {
        channel_id: id,
        add,
        ..Default::default()
    }))
}

#[derive(Clone)]
pub enum FrameSink {
    Queued {
        data: mpsc::Sender<Message>,
        control: mpsc::UnboundedSender<Message>,
    },
    Direct(mpsc::UnboundedSender<(Instant, Arc<Message>)>),
}

fn writer_gone(what: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::BrokenPipe, format!("{} gone", what))
}

impl FrameSink {
    pub async fn send_ordered(&self, msg: Message) -> ResultType<()> {
        match self {
            FrameSink::Queued { data, .. } => data
                .send(msg)
                .await
                .map_err(|_| writer_gone("tunnel writer").into()),
            FrameSink::Direct(tx) => tx
                .send((Instant::now(), Arc::new(msg)))
                .map_err(|_| writer_gone("connection writer").into()),
        }
    }

    pub fn send_control(&self, msg: Message) -> ResultType<()> {
        match self {
            FrameSink::Queued { control, .. } => control
                .send(msg)
                .map_err(|_| writer_gone("tunnel writer").into()),
            FrameSink::Direct(tx) => tx
                .send((Instant::now(), Arc::new(msg)))
                .map_err(|_| writer_gone("connection writer").into()),
        }
    }

    pub fn is_closed(&self) -> bool {
        match self {
            FrameSink::Queued { data, .. } => data.is_closed(),
            FrameSink::Direct(tx) => tx.is_closed(),
        }
    }
}

pub enum Inbound {
    Data(Bytes),
    Close,
    Violation,
}

#[derive(Debug, PartialEq)]
pub enum RelayEnd {
    LocalEof,
    PeerClosed,
    Violation,
    Cancelled,
    TunnelGone,
}

async fn relay_socket_to_tunnel<R: AsyncRead + Unpin>(
    id: i32,
    reader: R,
    prebuf: Vec<u8>,
    credit: Arc<SendCredit>,
    sink: FrameSink,
    mut cancel: watch::Receiver<bool>,
) -> RelayEnd {
    let mut reader = std::io::Cursor::new(prebuf).chain(reader);
    let mut scratch = vec![0u8; MAX_FRAME];
    loop {
        let allow = tokio::select! {
            n = credit.take(MAX_FRAME) => n,
            _ = cancel.changed() => return RelayEnd::Cancelled,
        };
        let got = tokio::select! {
            r = reader.read(&mut scratch[..allow]) => r.unwrap_or(0),
            _ = cancel.changed() => {
                credit.add(allow as u32);
                return RelayEnd::Cancelled;
            }
        };
        let spent = if got == 0 { 0 } else { charge(got) };
        if (spent as usize) < allow {
            credit.add(allow as u32 - spent);
        }
        if got == 0 {
            return RelayEnd::LocalEof;
        }
        let frame = data_msg(id, Bytes::copy_from_slice(&scratch[..got]));
        if sink.send_ordered(frame).await.is_err() {
            return RelayEnd::TunnelGone;
        }
    }
}

async fn relay_tunnel_to_socket<W: AsyncWrite + Unpin>(
    id: i32,
    mut writer: W,
    initial: Vec<Bytes>,
    mut inbound: mpsc::UnboundedReceiver<Inbound>,
    window: Arc<Mutex<RecvWindow>>,
    sink: FrameSink,
    mut cancel: watch::Receiver<bool>,
) -> RelayEnd {
    let mut pending: std::collections::VecDeque<Bytes> = initial.into();
    loop {
        let chunk = match pending.pop_front() {
            Some(c) => c,
            None => {
                let next = tokio::select! {
                    n = inbound.recv() => n,
                    _ = cancel.changed() => return RelayEnd::Cancelled,
                };
                match next {
                    Some(Inbound::Data(c)) => c,
                    Some(Inbound::Close) => return RelayEnd::PeerClosed,
                    Some(Inbound::Violation) => return RelayEnd::Violation,
                    None => return RelayEnd::TunnelGone,
                }
            }
        };
        let written = tokio::select! {
            r = writer.write_all(&chunk) => r.is_ok(),
            _ = cancel.changed() => return RelayEnd::Cancelled,
        };
        if !written {
            return RelayEnd::LocalEof;
        }
        let update = window.lock().unwrap().drained(chunk.len());
        if let Some(add) = update {
            if sink.send_control(window_update_msg(id, add)).is_err() {
                return RelayEnd::TunnelGone;
            }
        }
    }
}

pub async fn run_channel<R, W>(
    id: i32,
    reader: R,
    writer: W,
    prebuf: Vec<u8>,
    initial_out: Vec<Bytes>,
    credit: Arc<SendCredit>,
    window: Arc<Mutex<RecvWindow>>,
    inbound: mpsc::UnboundedReceiver<Inbound>,
    sink: FrameSink,
    mut teardown: watch::Receiver<bool>,
) where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let (cancel_tx, cancel_rx) = watch::channel(false);
    let mut to_tunnel = tokio::spawn(relay_socket_to_tunnel(
        id, reader, prebuf, credit, sink.clone(), cancel_rx.clone(),
    ));
    let mut to_socket = tokio::spawn(relay_tunnel_to_socket(
        id, writer, initial_out, inbound, window, sink.clone(), cancel_rx,
    ));
    let (first, second) = tokio::select! {
        r = &mut to_tunnel => {
            let _ = cancel_tx.send(true);
            (r.unwrap_or(RelayEnd::Cancelled), to_socket.await.unwrap_or(RelayEnd::Cancelled))
        }
        r = &mut to_socket => {
            let _ = cancel_tx.send(true);
            (r.unwrap_or(RelayEnd::Cancelled), to_tunnel.await.unwrap_or(RelayEnd::Cancelled))
        }
        _ = async { teardown.wait_for(|down| *down).await.is_ok() } => {
            let _ = cancel_tx.send(true);
            (to_tunnel.await.unwrap_or(RelayEnd::Cancelled), to_socket.await.unwrap_or(RelayEnd::Cancelled))
        }
    };
    let peer_closed = first == RelayEnd::PeerClosed || second == RelayEnd::PeerClosed;
    let tunnel_gone = first == RelayEnd::TunnelGone || second == RelayEnd::TunnelGone;
    let local_reason = matches!(first, RelayEnd::LocalEof | RelayEnd::Violation)
        || matches!(second, RelayEnd::LocalEof | RelayEnd::Violation);
    if !peer_closed && !tunnel_gone && local_reason {
        if let Err(e) = sink.send_ordered(close_msg(id)).await {
            log::debug!("port forward channel {} close not sent: {}", id, e);
        }
    }
    log::debug!("port forward channel {} ended: {:?} / {:?}", id, first, second);
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub use tunnel::{Claim, Tunnel};

#[cfg(not(any(target_os = "android", target_os = "ios")))]
mod tunnel {
    use super::*;
    use crate::client::Interface;
    use hbb_common::{
        config::READ_TIMEOUT,
        protobuf::Message as _,
        tokio::net::TcpStream,
        Stream,
    };
    use std::{
        collections::HashMap,
        sync::atomic::{AtomicI32, Ordering},
        time::Duration,
    };

    const REPORT_AGAIN_AFTER: Duration = Duration::from_secs(10);
    const MAX_REPORTED_OPEN_ERRORS: usize = 8;

    enum TunnelState {
        Unset,
        Muxed(Arc<TunnelHandle>),
        Legacy,
        Failed,
    }

    pub enum Claim {
        Claimed,
        Muxed(Arc<TunnelHandle>),
        Legacy,
    }

    pub struct Tunnel {
        state: watch::Sender<TunnelState>,
        lifetime: watch::Sender<()>,
    }

    impl Tunnel {
        pub fn new() -> Self {
            let (state, _) = watch::channel(TunnelState::Unset);
            let (lifetime, _) = watch::channel(());
            Self { state, lifetime }
        }

        pub fn claim(&self) -> Claim {
            match &*self.state.borrow() {
                TunnelState::Unset | TunnelState::Failed => Claim::Claimed,
                TunnelState::Muxed(h) => Claim::Muxed(h.clone()),
                TunnelState::Legacy => Claim::Legacy,
            }
        }

        pub fn set_muxed(&self, mut stream: Stream, kcp: Option<crate::kcp_stream::KcpStream>, interface: impl Interface) -> Arc<TunnelHandle> {
            cap_packet_size(&mut stream);
            let (data_tx, data_rx) = mpsc::channel(DATA_QUEUE_FRAMES);
            let (control_tx, control_rx) = mpsc::unbounded_channel();
            let handle = Arc::new(TunnelHandle {
                sink: FrameSink::Queued { data: data_tx, control: control_tx },
                channels: Mutex::new(HashMap::new()),
                next_id: AtomicI32::new(1),
                reported: Default::default(),
                teardown: watch::channel(false).0,
            });
            let state = self.state.clone();
            self.state.send_replace(TunnelState::Muxed(handle.clone()));
            tokio::spawn(tunnel_loop(
                stream,
                kcp,
                handle.clone(),
                data_rx,
                control_rx,
                interface,
                state,
                self.lifetime.subscribe(),
            ));
            handle
        }

        pub fn set_legacy(&self) {
            self.state.send_replace(TunnelState::Legacy);
        }

        pub fn set_failed(&self) {
            self.state.send_replace(TunnelState::Failed);
        }
    }

    struct ChannelEntry {
        inbound: mpsc::UnboundedSender<Inbound>,
        credit: Arc<SendCredit>,
        window: Arc<Mutex<RecvWindow>>,
        opened: bool,
    }

    pub struct TunnelHandle {
        sink: FrameSink,
        channels: Mutex<HashMap<i32, ChannelEntry>>,
        next_id: AtomicI32,
        reported: Mutex<HashMap<String, Instant>>,
        teardown: watch::Sender<bool>,
    }

    impl TunnelHandle {
        pub fn open(
            self: &Arc<Self>,
            host: &str,
            port: i32,
            socket: TcpStream,
            prebuf: Vec<u8>,
        ) -> ResultType<()> {
            if self.sink.is_closed() {
                hbb_common::bail!("port forward tunnel is gone");
            }
            let (inbound_tx, inbound_rx) = mpsc::unbounded_channel();
            let credit = Arc::new(SendCredit::new(INITIAL_WINDOW));
            let window = Arc::new(Mutex::new(RecvWindow::new(CHANNEL_WINDOW)));
            let id = {
                let mut channels = self.channels.lock().unwrap();
                if channels.len() >= MAX_CHANNELS {
                    hbb_common::bail!("too many port forward channels");
                }
                let mut id = self.next_id.fetch_add(1, Ordering::Relaxed);
                while channels.contains_key(&id) {
                    id = self.next_id.fetch_add(1, Ordering::Relaxed);
                }
                channels.insert(
                    id,
                    ChannelEntry {
                        inbound: inbound_tx,
                        credit: credit.clone(),
                        window: window.clone(),
                        opened: false,
                    },
                );
                id
            };
            let open = open_msg(id, host, port, CHANNEL_WINDOW);
            let (reader, writer) = socket.into_split();
            let sink = self.sink.clone();
            let teardown = self.teardown.subscribe();
            let handle = self.clone();
            tokio::spawn(async move {
                if sink.send_ordered(open).await.is_ok() {
                    run_channel(id, reader, writer, prebuf, Vec::new(), credit, window, inbound_rx, sink, teardown).await;
                }
                handle.channels.lock().unwrap().remove(&id);
            });
            Ok(())
        }

        pub(super) fn on_frame(&self, ch: PortForwardChannel) -> Option<String> {
            match ch.union {
                Some(port_forward_channel::Union::Opened(o)) => {
                    let refused = {
                        let mut channels = self.channels.lock().unwrap();
                        if o.success {
                            if let Some(e) = channels.get_mut(&o.channel_id) {
                                if !e.opened {
                                    e.opened = true;
                                    e.credit.raise_initial(o.window);
                                }
                            }
                            None
                        } else if let Some(e) = channels.remove(&o.channel_id) {
                            log::debug!("port forward channel {} refused: {}", o.channel_id, o.message);
                            e.inbound.send(Inbound::Close).ok();
                            Some(o.message)
                        } else {
                            None
                        }
                    };
                    refused.and_then(|message| self.first_report(message))
                }
                Some(port_forward_channel::Union::Data(d)) => {
                    let mut channels = self.channels.lock().unwrap();
                    let Some(e) = channels.get(&d.channel_id) else {
                        log::debug!("port forward data for unknown channel {}", d.channel_id);
                        return None;
                    };
                    let accepted = e.window.lock().unwrap().accept(d.data.len());
                    let delivered = accepted && e.inbound.send(Inbound::Data(d.data)).is_ok();
                    if delivered {
                        return None;
                    }
                    if let Some(e) = channels.remove(&d.channel_id) {
                        if !accepted {
                            log::warn!("port forward channel {} overran its window", d.channel_id);
                            e.inbound.send(Inbound::Violation).ok();
                        }
                    }
                    None
                }
                Some(port_forward_channel::Union::Close(c)) => {
                    if let Some(e) = self.channels.lock().unwrap().remove(&c.channel_id) {
                        e.inbound.send(Inbound::Close).ok();
                    }
                    None
                }
                Some(port_forward_channel::Union::WindowUpdate(u)) => {
                    if let Some(e) = self.channels.lock().unwrap().get(&u.channel_id) {
                        e.credit.add(u.add);
                    }
                    None
                }
                Some(port_forward_channel::Union::Open(o)) => {
                    log::debug!("ignoring open for channel {} on the controller", o.channel_id);
                    None
                }
                _ => None,
            }
        }

        fn first_report(&self, message: String) -> Option<String> {
            self.first_report_at(message, Instant::now())
        }

        fn first_report_at(&self, message: String, now: Instant) -> Option<String> {
            if message.is_empty() {
                return None;
            }
            let mut reported = self.reported.lock().unwrap();
            reported.retain(|_, last| now.duration_since(*last) < REPORT_AGAIN_AFTER);
            if let Some(last) = reported.get_mut(&message) {
                *last = now;
                return None;
            }
            if reported.len() >= MAX_REPORTED_OPEN_ERRORS {
                return None;
            }
            reported.insert(message.clone(), now);
            Some(message)
        }

        fn close_all(&self) {
            self.channels.lock().unwrap().clear();
            self.teardown.send_replace(true);
        }
    }

    async fn tunnel_loop(
        mut stream: Stream,
        kcp: Option<crate::kcp_stream::KcpStream>,
        handle: Arc<TunnelHandle>,
        mut data_rx: mpsc::Receiver<Message>,
        mut control_rx: mpsc::UnboundedReceiver<Message>,
        interface: impl Interface,
        state: watch::Sender<TunnelState>,
        mut lifetime: watch::Receiver<()>,
    ) {
        let err = loop {
            tokio::select! {
                Some(msg) = control_rx.recv() => {
                    if let Err(e) = stream.send(&msg).await {
                        break format!("send failed: {}", e);
                    }
                }
                res = stream.next_timeout(READ_TIMEOUT) => match res {
                    Some(Ok(bytes)) => {
                        let Ok(msg) = Message::parse_from_bytes(&bytes) else { continue };
                        match msg.union {
                            Some(message::Union::PortForwardChannel(ch)) => {
                                if let Some(err) = handle.on_frame(ch) {
                                    interface.on_error(&err);
                                }
                            }
                            Some(message::Union::TestDelay(t)) => {
                                interface.handle_test_delay(t, &mut stream).await;
                            }
                            Some(message::Union::Misc(misc)) => {
                                if let Some(misc::Union::CloseReason(r)) = misc.union {
                                    break format!("closed by peer: {}", r);
                                }
                            }
                            _ => {}
                        }
                    }
                    Some(Err(e)) => break format!("read failed: {}", e),
                    None => break "timeout or reset by the peer".to_owned(),
                },
                Some(msg) = data_rx.recv() => {
                    if let Err(e) = stream.send(&msg).await {
                        break format!("send failed: {}", e);
                    }
                }
                _ = lifetime.changed() => {
                    if kcp.is_some() {
                        send_kcp_close_reason(&mut stream).await;
                    }
                    break "window closed".to_owned()
                }
            }
        };
        log::info!("port forward tunnel ended: {}", err);
        handle.close_all();
        state.send_replace(TunnelState::Unset);
    }

    const KCP_CLOSE_REASON_FLUSH_DELAY: Duration = Duration::from_millis(30);

    async fn send_kcp_close_reason(stream: &mut Stream) {
        let mut misc = Misc::new();
        misc.set_close_reason("kcp".to_owned());
        let mut msg = Message::new();
        msg.set_misc(misc);
        match tokio::time::timeout(KCP_CLOSE_REASON_FLUSH_DELAY, stream.send(&msg)).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => log::debug!("port forward close reason not sent: {}", e),
            Err(_) => log::debug!("port forward close reason not sent: send timed out"),
        }
        tokio::time::sleep(KCP_CLOSE_REASON_FLUSH_DELAY).await;
    }
}
