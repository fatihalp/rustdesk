//! Builders for every message the server sends; controllers decide *what* to say, this decides *how*.

use bytes::Bytes;
use protobuf::{EnumOrUnknown, Message};

use crate::proto::rendezvous::{
    punch_hole_response::Failure, register_pk_response, rendezvous_message::Union, FetchLocalAddr,
    NatType, OnlineResponse, PunchHole, PunchHoleResponse, RegisterPeerResponse, RegisterPkResponse,
    RelayResponse, RendezvousMessage, RequestRelay, TestNatResponse,
};

pub fn encode(message: &RendezvousMessage) -> Bytes {
    message.write_to_bytes().unwrap_or_default().into()
}

pub fn register_peer_response(request_pk: bool) -> RendezvousMessage {
    wrap(Union::RegisterPeerResponse(RegisterPeerResponse {
        request_pk,
        ..Default::default()
    }))
}

pub fn register_pk_response(result: register_pk_response::Result) -> RendezvousMessage {
    wrap(Union::RegisterPkResponse(RegisterPkResponse {
        result: result.into(),
        ..Default::default()
    }))
}

pub fn punch_hole_failure(failure: Failure) -> RendezvousMessage {
    wrap(Union::PunchHoleResponse(PunchHoleResponse {
        failure: failure.into(),
        ..Default::default()
    }))
}

pub fn punch_hole(
    socket_addr: Vec<u8>,
    nat_type: EnumOrUnknown<NatType>,
    relay_server: String,
) -> RendezvousMessage {
    wrap(Union::PunchHole(PunchHole {
        socket_addr: socket_addr.into(),
        nat_type,
        relay_server,
        ..Default::default()
    }))
}

pub fn fetch_local_addr(socket_addr: Vec<u8>, relay_server: String) -> RendezvousMessage {
    wrap(Union::FetchLocalAddr(FetchLocalAddr {
        socket_addr: socket_addr.into(),
        relay_server,
        ..Default::default()
    }))
}

pub fn punch_hole_response(response: PunchHoleResponse) -> RendezvousMessage {
    wrap(Union::PunchHoleResponse(response))
}

pub fn request_relay(request: RequestRelay) -> RendezvousMessage {
    wrap(Union::RequestRelay(request))
}

pub fn relay_response(response: RelayResponse) -> RendezvousMessage {
    wrap(Union::RelayResponse(response))
}

pub fn test_nat_response(port: u16) -> RendezvousMessage {
    wrap(Union::TestNatResponse(TestNatResponse {
        port: port as i32,
        ..Default::default()
    }))
}

/// `states` holds one bit per requested peer, most significant bit first.
pub fn online_response(states: Vec<u8>) -> RendezvousMessage {
    wrap(Union::OnlineResponse(OnlineResponse {
        states: states.into(),
        ..Default::default()
    }))
}

fn wrap(union: Union) -> RendezvousMessage {
    RendezvousMessage {
        union: Some(union),
        ..Default::default()
    }
}
