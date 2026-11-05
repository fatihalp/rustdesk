use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4},
    time::{SystemTime, UNIX_EPOCH},
};

pub fn normalize(addr: SocketAddr) -> SocketAddr {
    match addr {
        SocketAddr::V6(v6) if !addr.ip().is_loopback() => match v6.ip().to_ipv4() {
            Some(v4) => SocketAddr::new(IpAddr::V4(v4), addr.port()),
            None => addr,
        },
        _ => addr,
    }
}

pub fn host_of(addr: SocketAddr) -> String {
    normalize(addr).ip().to_string()
}

pub fn same_host(a: SocketAddr, b: SocketAddr) -> bool {
    normalize(a).ip() == normalize(b).ip()
}

pub fn encode_addr(addr: SocketAddr) -> Vec<u8> {
    match normalize(addr) {
        SocketAddr::V4(v4) => {
            let tm = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_micros() as u32)
                .unwrap_or_default() as u128;
            let ip = u32::from_le_bytes(v4.ip().octets()) as u128;
            let value = ((ip + tm) << 49) | (tm << 17) | (v4.port() as u128 + (tm & 0xFFFF));
            let bytes = value.to_le_bytes();
            let used = bytes.iter().rposition(|b| *b != 0).map_or(0, |i| i + 1);
            bytes[..used].to_vec()
        }
        SocketAddr::V6(v6) => {
            let mut out = v6.ip().octets().to_vec();
            out.extend_from_slice(&v6.port().to_le_bytes());
            out
        }
    }
}

pub fn decode_addr(bytes: &[u8]) -> Option<SocketAddr> {
    if bytes.len() > 16 {
        if bytes.len() != 18 {
            return None;
        }
        let ip: [u8; 16] = bytes[..16].try_into().ok()?;
        let port = u16::from_le_bytes(bytes[16..].try_into().ok()?);
        return Some(SocketAddr::new(IpAddr::V6(Ipv6Addr::from(ip)), port));
    }
    let mut padded = [0u8; 16];
    padded[..bytes.len()].copy_from_slice(bytes);
    let number = u128::from_le_bytes(padded);
    let tm = (number >> 17) & u32::MAX as u128;
    let ip = ((number >> 49).wrapping_sub(tm) as u32).to_le_bytes();
    let port = ((number & 0xFF_FFFF).wrapping_sub(tm & 0xFFFF)) as u16;
    Some(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::from(ip), port)))
}
