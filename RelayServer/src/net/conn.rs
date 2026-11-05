use std::{io, net::SocketAddr};

use anyhow::Result;
use bytes::{Buf, Bytes, BytesMut};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    tungstenite::{
        handshake::server::{Request, Response},
        Message,
    },
    WebSocketStream,
};
use tokio_util::codec::{Decoder, Encoder, Framed};

const MAX_FRAME_LEN: usize = 64 * 1024 * 1024;
const MAX_PREALLOCATION: usize = 256 * 1024;

pub enum Conn {
    Tcp(Framed<TcpStream, FrameCodec>),
    WebSocket(WebSocketStream<TcpStream>),
}

impl Conn {
    pub async fn accept(
        stream: TcpStream,
        addr: SocketAddr,
        websocket: bool,
        trust_proxy_headers: bool,
    ) -> Result<(Self, SocketAddr)> {
        stream.set_nodelay(true).ok();
        if !websocket {
            return Ok((Self::Tcp(Framed::new(stream, FrameCodec::default())), addr));
        }
        let mut client = addr;
        let capture_real_ip = |request: &Request, response: Response| {
            if trust_proxy_headers {
                let headers = request.headers();
                let forwarded = headers
                    .get("X-Real-IP")
                    .or_else(|| headers.get("X-Forwarded-For"))
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.trim().parse().ok());
                if let Some(ip) = forwarded {
                    client = SocketAddr::new(ip, 0);
                }
            }
            Ok(response)
        };
        let socket = tokio_tungstenite::accept_hdr_async(stream, capture_real_ip).await?;
        Ok((Self::WebSocket(socket), client))
    }

    pub async fn recv(&mut self) -> Option<io::Result<Bytes>> {
        match self {
            Self::Tcp(framed) => Some(framed.next().await?.map(BytesMut::freeze)),
            Self::WebSocket(socket) => loop {
                match socket.next().await? {
                    Ok(Message::Binary(data)) => return Some(Ok(data)),
                    Ok(Message::Close(_)) => return None,
                    Ok(_) => continue,
                    Err(error) => return Some(Err(io::Error::other(error))),
                }
            },
        }
    }

    pub async fn send(&mut self, data: Bytes) -> io::Result<()> {
        match self {
            Self::Tcp(framed) => framed.send(data).await,
            Self::WebSocket(socket) => socket
                .send(Message::Binary(data))
                .await
                .map_err(io::Error::other),
        }
    }
}

#[derive(Default)]
pub struct FrameCodec {
    pending: Option<usize>,
}

impl Decoder for FrameCodec {
    type Item = BytesMut;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> io::Result<Option<BytesMut>> {
        let len = match self.pending {
            Some(len) => len,
            None => {
                let Some(&first) = src.first() else {
                    return Ok(None);
                };
                let header_len = (first & 0x3) as usize + 1;
                if src.len() < header_len {
                    return Ok(None);
                }
                let raw = src[..header_len]
                    .iter()
                    .enumerate()
                    .fold(0usize, |acc, (i, byte)| acc | (*byte as usize) << (8 * i));
                let len = raw >> 2;
                if len > MAX_FRAME_LEN {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "frame too large"));
                }
                src.advance(header_len);
                src.reserve(len.saturating_sub(src.len()).min(MAX_PREALLOCATION));
                self.pending = Some(len);
                len
            }
        };
        if src.len() < len {
            return Ok(None);
        }
        self.pending = None;
        Ok(Some(src.split_to(len)))
    }
}

impl Encoder<Bytes> for FrameCodec {
    type Error = io::Error;

    fn encode(&mut self, data: Bytes, dst: &mut BytesMut) -> io::Result<()> {
        let len = data.len();
        if len > 0x3FFF_FFFF {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "frame too large"));
        }
        let header_len = match len {
            0..=0x3F => 1,
            0x40..=0x3FFF => 2,
            0x4000..=0x3F_FFFF => 3,
            _ => 4,
        };
        let header = ((len as u32) << 2) | (header_len as u32 - 1);
        dst.reserve(header_len + len);
        dst.extend_from_slice(&header.to_le_bytes()[..header_len]);
        dst.extend_from_slice(&data);
        Ok(())
    }
}
