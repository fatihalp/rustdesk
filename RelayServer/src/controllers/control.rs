use std::time::Duration;

use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};
use tokio_tungstenite::tungstenite::Message;

use super::{RelayController, RendezvousController};

const HTTP_READ_TIMEOUT: Duration = Duration::from_secs(5);

pub struct ControlController {
    secret: String,
}

impl ControlController {
    pub fn new(secret: String) -> Self {
        Self { secret }
    }

    pub async fn serve(
        &self,
        mut stream: TcpStream,
        rendezvous: &RendezvousController,
        relay: &RelayController,
    ) -> Result<()> {
        let mut peek_buf = [0u8; 2048];
        let n = timeout(HTTP_READ_TIMEOUT, stream.peek(&mut peek_buf)).await??;
        let header_str = String::from_utf8_lossy(&peek_buf[..n]);

        if header_str.to_ascii_lowercase().contains("upgrade: websocket") {
            return self.serve_websocket(stream, rendezvous, relay).await;
        }

        let mut buf = vec![0u8; 4096];
        let n = timeout(HTTP_READ_TIMEOUT, stream.read(&mut buf)).await??;
        let req = String::from_utf8_lossy(&buf[..n]);
        let mut lines = req.lines();
        let request_line = lines.next().unwrap_or_default();
        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap_or("GET");
        let path = parts.next().unwrap_or("/");

        if method == "OPTIONS" {
            return write_json(&mut stream, 204, &json!({})).await;
        }

        if method == "POST" && !self.secret.is_empty() && !authorized(&req, &self.secret) {
            return write_json(&mut stream, 401, &json!({ "error": "unauthorized" })).await;
        }

        let (status, body) = self.route(method, path, rendezvous, relay);
        write_json(&mut stream, status, &body).await
    }

    fn route(
        &self,
        method: &str,
        path: &str,
        rendezvous: &RendezvousController,
        relay: &RelayController,
    ) -> (u16, Value) {
        let clean_path = path.split('?').next().unwrap_or(path);
        match (method, clean_path) {
            ("GET", "/" | "/status" | "/peers") => (200, Self::snapshot(rendezvous, relay)),
            ("POST", p) if p.starts_with("/peers/") => {
                let segments: Vec<&str> = p.trim_matches('/').split('/').collect();
                if segments.len() == 3 {
                    let id = segments[1];
                    let action = segments[2];
                    return (200, Self::apply_action(action, id, rendezvous, relay));
                }
                (404, json!({ "error": "not_found" }))
            }
            _ => (404, json!({ "error": "not_found" })),
        }
    }

    fn snapshot(rendezvous: &RendezvousController, relay: &RelayController) -> Value {
        rendezvous.peers().sweep_expired();
        let peers = rendezvous.peers().snapshot();
        let sessions = relay.active_sessions();
        let online_count = peers
            .iter()
            .filter(|p| p.get("online").and_then(Value::as_bool).unwrap_or(false))
            .count();
        json!({
            "type": "snapshot",
            "public_key": rendezvous.public_key(),
            "online_count": online_count,
            "active_sessions": sessions.len(),
            "peers": peers,
            "sessions": sessions,
        })
    }

    fn apply_action(
        action: &str,
        id: &str,
        rendezvous: &RendezvousController,
        relay: &RelayController,
    ) -> Value {
        match action {
            "block" => {
                rendezvous.peers().set_blocked(id, true);
                let dropped = relay.disconnect(id);
                json!({ "ok": true, "id": id, "blocked": true, "disconnected_sessions": dropped })
            }
            "unblock" => {
                rendezvous.peers().set_blocked(id, false);
                json!({ "ok": true, "id": id, "blocked": false })
            }
            "disconnect" => {
                let dropped = relay.disconnect(id);
                let offlined = rendezvous.peers().force_offline(id);
                json!({ "ok": true, "id": id, "disconnected_sessions": dropped, "offlined": offlined })
            }
            _ => json!({ "ok": false, "error": "unknown_action" }),
        }
    }

    async fn serve_websocket(
        &self,
        stream: TcpStream,
        rendezvous: &RendezvousController,
        relay: &RelayController,
    ) -> Result<()> {
        let mut ws = tokio_tungstenite::accept_async(stream).await?;
        let initial = Self::snapshot(rendezvous, relay).to_string();
        ws.send(Message::Text(initial.into())).await?;

        let mut events: tokio::sync::broadcast::Receiver<String> = rendezvous.peers().subscribe();
        loop {
            tokio::select! {
                Ok(event) = events.recv() => {
                    if ws.send(Message::Text(event.into())).await.is_err() {
                        break;
                    }
                }
                incoming = ws.next() => {
                    let Some(Ok(msg)) = incoming else { break };
                    match msg {
                        Message::Text(text) => {
                            if let Ok(cmd) = serde_json::from_str::<Value>(&text) {
                                let action = cmd.get("action").and_then(Value::as_str).unwrap_or("");
                                if action == "snapshot" {
                                    let snap = Self::snapshot(rendezvous, relay).to_string();
                                    let _ = ws.send(Message::Text(snap.into())).await;
                                } else if let Some(id) = cmd.get("id").and_then(Value::as_str) {
                                    let secret_ok = !self.secret.is_empty()
                                        && cmd.get("secret").and_then(Value::as_str) == Some(self.secret.as_str());
                                    if secret_ok {
                                        let res = Self::apply_action(action, id, rendezvous, relay).to_string();
                                        let _ = ws.send(Message::Text(res.into())).await;
                                    }
                                }
                            }
                        }
                        Message::Close(_) => break,
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }
}

fn authorized(raw_request: &str, secret: &str) -> bool {
    for line in raw_request.lines() {
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("x-relay-secret") && value.trim() == secret {
                return true;
            }
        }
    }
    false
}

async fn write_json(stream: &mut TcpStream, status: u16, body: &Value) -> Result<()> {
    let reason = match status {
        200 => "OK",
        204 => "No Content",
        401 => "Unauthorized",
        404 => "Not Found",
        _ => "Error",
    };
    let payload = if status == 204 { String::new() } else { body.to_string() };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Type: application/json\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
         Access-Control-Allow-Headers: Content-Type, X-Relay-Secret\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\r\n\
         {payload}",
        payload.len()
    );
    stream.write_all(response.as_bytes()).await?;
    Ok(())
}
