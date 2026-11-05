use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
    time::{Duration, Instant},
};

const BURST_WINDOW: Duration = Duration::from_secs(60);
const MAX_REQUESTS_PER_WINDOW: u32 = 30;
const ID_WINDOW: Duration = Duration::from_secs(24 * 3600);
const MAX_IDS_PER_WINDOW: usize = 300;
const PURGE_THRESHOLD: usize = 10_000;

#[derive(Default)]
pub struct IpGuard {
    records: Mutex<HashMap<String, Record>>,
}

struct Record {
    burst_start: Instant,
    burst_count: u32,
    ids_start: Instant,
    ids: HashSet<String>,
}

impl IpGuard {
    pub fn allow(&self, ip: &str, id: &str) -> bool {
        let now = Instant::now();
        let mut records = self.records.lock().unwrap_or_else(|e| e.into_inner());
        if records.len() > PURGE_THRESHOLD {
            records.retain(|_, record| now.duration_since(record.ids_start) < ID_WINDOW);
        }
        let record = records.entry(ip.to_owned()).or_insert_with(|| Record {
            burst_start: now,
            burst_count: 0,
            ids_start: now,
            ids: HashSet::new(),
        });

        if now.duration_since(record.burst_start) > BURST_WINDOW {
            record.burst_start = now;
            record.burst_count = 0;
        }
        if record.burst_count >= MAX_REQUESTS_PER_WINDOW {
            return false;
        }
        record.burst_count += 1;

        if now.duration_since(record.ids_start) > ID_WINDOW {
            record.ids_start = now;
            record.ids.clear();
        }
        if record.ids.contains(id) {
            return true;
        }
        if record.ids.len() >= MAX_IDS_PER_WINDOW {
            return false;
        }
        record.ids.insert(id.to_owned());
        true
    }
}
