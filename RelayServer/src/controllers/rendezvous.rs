use std::{net::SocketAddr, sync::Arc};

use bytes::Bytes;
use protobuf::Message;

use crate::{
    config::Config,
    models::{IpGuard, PeerStore, PkRegistration, ServerKeys},
    net::{decode_addr, encode_addr, same_host},
    proto::rendezvous::{
        punch_hole_response::Failure, register_pk_response, rendezvous_message::Union, LocalAddr,
        NatType, OnlineRequest, PunchHoleRequest, PunchHoleResponse, PunchHoleSent, RegisterPk,
        RelayResponse, RendezvousMessage, RequestRelay,
    },
    views,
};

const MIN_REGISTRATION_ID_LEN: usize = 6;
const MAX_ID_LEN: usize = 253;

/// Where a reply must be delivered.
pub enum Target {
    /// Back over the connection the request arrived on.
    Caller,
    /// A registered peer, reached by UDP.
    Udp(SocketAddr),
    /// A client with an open TCP/WebSocket connection, identified by its public address.
    Waiting(SocketAddr),
}

pub struct Reply {
    pub target: Target,
    pub message: RendezvousMessage,
}

pub struct Outcome {
    pub reply: Option<Reply>,
    /// Keep the connection open to receive a later answer from the remote peer.
    pub keep_open: bool,
}

impl Outcome {
    fn close(reply: Option<Reply>) -> Self {
        Self { reply, keep_open: false }
    }

    fn keep_open(reply: Option<Reply>) -> Self {
        Self { reply, keep_open: true }
    }
}

/// Turns incoming rendezvous messages into replies: peer registration and connection brokering.
pub struct RendezvousController {
    config: Arc<Config>,
    access_key: String,
    peers: Arc<PeerStore>,
    keys: ServerKeys,
    guard: IpGuard,
}

impl RendezvousController {
    pub fn new(config: Arc<Config>, access_key: String, peers: Arc<PeerStore>, keys: ServerKeys) -> Self {
        Self {
            config,
            access_key,
            peers,
            keys,
            guard: IpGuard::default(),
        }
    }

    pub fn peers(&self) -> &Arc<PeerStore> {
        &self.peers
    }

    pub fn public_key(&self) -> &str {
        self.keys.public_key()
    }

    /// Handles a UDP datagram; peers only register over UDP.
    pub fn handle_udp(&self, bytes: &[u8], addr: SocketAddr) -> Option<RendezvousMessage> {
        match RendezvousMessage::parse_from_bytes(bytes).ok()?.union? {
            Union::RegisterPeer(request) if is_valid_id(&request.id) => Some(
                views::register_peer_response(self.peers.heartbeat(&request.id, addr)),
            ),
            Union::RegisterPk(request) => self.register_pk(request, addr),
            _ => None,
        }
    }

    /// Handles one message from a TCP or WebSocket client.
    pub fn handle_tcp(&self, bytes: &[u8], addr: SocketAddr, websocket: bool) -> Outcome {
        let Some(union) = RendezvousMessage::parse_from_bytes(bytes)
            .ok()
            .and_then(|message| message.union)
        else {
            return Outcome::close(None);
        };
        match union {
            Union::PunchHoleRequest(request) => {
                Outcome::keep_open(Some(self.punch_hole_request(addr, request, websocket)))
            }
            Union::RequestRelay(request) => Outcome::keep_open(self.request_relay(addr, request)),
            Union::RelayResponse(response) => Outcome::close(self.relay_response(response)),
            Union::PunchHoleSent(sent) => Outcome::close(self.punch_hole_sent(addr, sent)),
            Union::LocalAddr(local) => Outcome::close(self.local_addr(local)),
            Union::TestNatRequest(_) => Outcome::close(Some(caller(views::test_nat_response(addr.port())))),
            Union::OnlineRequest(request) => Outcome::close(Some(caller(self.online_response(request)))),
            Union::RegisterPk(_) => Outcome::close(Some(caller(views::register_pk_response(
                register_pk_response::Result::NOT_SUPPORT,
            )))),
            _ => Outcome::close(None),
        }
    }

    fn register_pk(&self, request: RegisterPk, addr: SocketAddr) -> Option<RendezvousMessage> {
        use register_pk_response::Result as Code;
        if request.uuid.is_empty() || request.pk.is_empty() {
            return None;
        }
        let ip = crate::net::host_of(addr);
        let code = if request.id.len() < MIN_REGISTRATION_ID_LEN || request.id.len() > MAX_ID_LEN {
            Code::UUID_MISMATCH
        } else if !self.guard.allow(&ip, &request.id) {
            Code::TOO_FREQUENT
        } else {
            match self.peers.register_pk(&request.id, addr, request.uuid, request.pk) {
                PkRegistration::Registered => Code::OK,
                PkRegistration::UuidMismatch => Code::UUID_MISMATCH,
                PkRegistration::TooFrequent => Code::TOO_FREQUENT,
                PkRegistration::ServerError => Code::SERVER_ERROR,
            }
        };
        Some(views::register_pk_response(code))
    }

    /// Client A wants to reach peer B: ask B to punch a hole, or to expose its local address.
    fn punch_hole_request(&self, addr: SocketAddr, request: PunchHoleRequest, websocket: bool) -> Reply {
        if !self.access_key.is_empty() && request.licence_key != self.access_key {
            log::warn!("Rejected {addr}: invalid key for peer {}", request.id);
            return caller(views::punch_hole_failure(Failure::LICENSE_MISMATCH));
        }
        let Some(peer) = self.peers.find(&request.id) else {
            return caller(views::punch_hole_failure(Failure::ID_NOT_EXIST));
        };
        if !peer.is_online() {
            return caller(views::punch_hole_failure(Failure::OFFLINE));
        }

        let nat_type = if self.config.always_use_relay {
            NatType::SYMMETRIC.into()
        } else {
            request.nat_type
        };
        let socket_addr = encode_addr(addr);
        let relay_server = self.config.relay_host.clone();
        // Behind the same router a hole cannot be punched, so B reports its local address instead.
        let message = if !websocket && same_host(peer.addr, addr) {
            views::fetch_local_addr(socket_addr, relay_server)
        } else {
            views::punch_hole(socket_addr, nat_type, relay_server)
        };
        Reply {
            target: Target::Udp(peer.addr),
            message,
        }
    }

    fn request_relay(&self, addr: SocketAddr, mut request: RequestRelay) -> Option<Reply> {
        let peer = self.peers.find(&request.id).filter(|peer| peer.is_online())?;
        request.socket_addr = encode_addr(addr).into();
        Some(Reply {
            target: Target::Udp(peer.addr),
            message: views::request_relay(request),
        })
    }

    /// Peer B accepted a relay request; forward its answer to the requester.
    fn relay_response(&self, mut response: RelayResponse) -> Option<Reply> {
        let requester = decode_addr(&response.socket_addr)?;
        response.socket_addr = Bytes::new();
        let id = response.id().to_owned();
        if !id.is_empty() {
            response.set_pk(self.signed_pk(&response.version, &id));
        }
        Some(Reply {
            target: Target::Waiting(requester),
            message: views::relay_response(response),
        })
    }

    /// Peer B punched its hole; tell requester A where to connect.
    fn punch_hole_sent(&self, addr: SocketAddr, sent: PunchHoleSent) -> Option<Reply> {
        let requester = decode_addr(&sent.socket_addr)?;
        let mut response = PunchHoleResponse {
            socket_addr: encode_addr(addr).into(),
            pk: self.signed_pk(&sent.version, &sent.id),
            relay_server: sent.relay_server,
            ..Default::default()
        };
        if let Ok(nat_type) = sent.nat_type.enum_value() {
            response.set_nat_type(nat_type);
        }
        Some(Reply {
            target: Target::Waiting(requester),
            message: views::punch_hole_response(response),
        })
    }

    /// Peer B shared its LAN address; tell requester A to connect directly.
    fn local_addr(&self, local: LocalAddr) -> Option<Reply> {
        let requester = decode_addr(&local.socket_addr)?;
        let mut response = PunchHoleResponse {
            socket_addr: local.local_addr,
            pk: self.signed_pk(&local.version, &local.id),
            relay_server: local.relay_server,
            ..Default::default()
        };
        response.set_is_local(true);
        Some(Reply {
            target: Target::Waiting(requester),
            message: views::punch_hole_response(response),
        })
    }

    fn online_response(&self, request: OnlineRequest) -> RendezvousMessage {
        let mut states = vec![0u8; request.peers.len().div_ceil(8)];
        for (index, id) in request.peers.iter().enumerate() {
            if self.peers.is_online(id) {
                states[index / 8] |= 0x80 >> (index % 8);
            }
        }
        views::online_response(states)
    }

    /// Server-signed `(id, pk)` proof for clients that announce a version; empty for legacy clients.
    fn signed_pk(&self, client_version: &str, id: &str) -> Bytes {
        if client_version.is_empty() {
            return Bytes::new();
        }
        self.peers
            .find(id)
            .map(|peer| self.keys.sign_id_pk(id, &peer.pk))
            .unwrap_or_default()
    }
}

fn caller(message: RendezvousMessage) -> Reply {
    Reply {
        target: Target::Caller,
        message,
    }
}

fn is_valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= MAX_ID_LEN
}
