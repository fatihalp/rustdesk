mod addr;
mod conn;

pub use addr::{decode_addr, encode_addr, host_of, normalize, same_host};
pub use conn::Conn;

use std::{
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
};

use socket2::{Domain, Socket, Type};
use tokio::net::{TcpListener, UdpSocket};

pub fn bind_tcp(bind: Option<IpAddr>, port: u16) -> io::Result<TcpListener> {
    let socket = bind_socket(bind, port, Type::STREAM)?;
    socket.listen(1024)?;
    TcpListener::from_std(socket.into())
}

pub fn bind_udp(bind: Option<IpAddr>, port: u16) -> io::Result<UdpSocket> {
    UdpSocket::from_std(bind_socket(bind, port, Type::DGRAM)?.into())
}

/// Binds dual-stack on `[::]` when no address is configured, falling back to `0.0.0.0`.
fn bind_socket(bind: Option<IpAddr>, port: u16, kind: Type) -> io::Result<Socket> {
    let candidates = match bind {
        Some(ip) => vec![ip],
        None => vec![IpAddr::V6(Ipv6Addr::UNSPECIFIED), IpAddr::V4(Ipv4Addr::UNSPECIFIED)],
    };
    let mut last_error = None;
    for ip in candidates {
        match bind_one(SocketAddr::new(ip, port), kind) {
            Ok(socket) => return Ok(socket),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| io::Error::new(io::ErrorKind::AddrNotAvailable, "no address to bind")))
}

fn bind_one(addr: SocketAddr, kind: Type) -> io::Result<Socket> {
    let socket = Socket::new(Domain::for_address(addr), kind, None)?;
    if kind == Type::STREAM {
        socket.set_reuse_address(true)?;
    }
    if addr.is_ipv6() {
        socket.set_only_v6(false)?;
    }
    socket.set_nonblocking(true)?;
    socket.bind(&addr.into())?;
    Ok(socket)
}
