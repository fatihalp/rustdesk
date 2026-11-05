use std::{collections::HashSet, path::Path, sync::Mutex};

use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

pub struct StoredPeer {
    pub uuid: Vec<u8>,
    pub pk: Vec<u8>,
    pub ip: String,
    pub blocked: bool,
}

pub struct PeerRepository {
    conn: Mutex<Connection>,
}

impl PeerRepository {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "pragma journal_mode = WAL;
             pragma synchronous = NORMAL;
             pragma temp_store = MEMORY;
             pragma busy_timeout = 5000;
             create table if not exists peer (
                guid blob primary key not null,
                id varchar(100) not null,
                uuid blob not null,
                pk blob not null,
                created_at datetime not null default(current_timestamp),
                user blob,
                status tinyint,
                note varchar(300),
                info text not null
            ) without rowid;
            create unique index if not exists index_peer_id on peer (id);",
        )?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn load_blocked_ids(&self) -> HashSet<String> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let Ok(mut stmt) = conn.prepare("select id from peer where status = 1") else {
            return HashSet::new();
        };
        let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) else {
            return HashSet::new();
        };
        rows.flatten().collect()
    }

    pub fn set_blocked(&self, id: &str, blocked: bool) {
        let status: i32 = if blocked { 1 } else { 0 };
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let _ = conn.execute("update peer set status = ? where id = ?", params![status, id]);
    }

    pub fn find(&self, id: &str) -> Result<Option<StoredPeer>> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let row = conn
            .query_row("select uuid, pk, info, coalesce(status, 0) from peer where id = ?", [id], |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i32>(3)?,
                ))
            })
            .optional()?;
        Ok(row.map(|(uuid, pk, info, status)| StoredPeer {
            uuid,
            pk,
            ip: serde_json::from_str::<serde_json::Value>(&info)
                .ok()
                .and_then(|value| value.get("ip")?.as_str().map(str::to_owned))
                .unwrap_or_default(),
            blocked: status == 1,
        }))
    }

    pub fn save(&self, id: &str, uuid: &[u8], pk: &[u8], ip: &str) -> Result<()> {
        let info = serde_json::json!({ "ip": ip }).to_string();
        let guid = rand::random::<[u8; 16]>();
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.execute(
            "insert into peer(guid, id, uuid, pk, status, info) values(?, ?, ?, ?, 0, ?)
             on conflict(id) do update set pk = excluded.pk, info = excluded.info",
            params![guid.as_slice(), id, uuid, pk, info],
        )?;
        Ok(())
    }
}
