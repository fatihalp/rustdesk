use super::*;
use hbb_common::{
    anyhow::{anyhow, Context, Result},
    compress,
};
use portable_pty::{Child, CommandBuilder, PtySize};
use std::{
    collections::{HashMap, VecDeque},
    io::{Read, Write},
    ops::{Deref, DerefMut},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

#[cfg(target_os = "windows")]
use super::terminal_helper::{
    configure_utf8_shell_command, create_named_pipe_server, encode_helper_message,
    encode_resize_message, is_helper_process_running, launch_terminal_helper_with_token,
    wait_for_pipe_connection, HelperProcessGuard, OwnedHandle, SendableHandle, WinCloseHandle,
    WinTerminateProcess, WinWaitForSingleObject, MSG_TYPE_DATA, PIPE_CONNECTION_TIMEOUT_MS,
    WIN_WAIT_OBJECT_0,
};

const MAX_OUTPUT_BUFFER_SIZE: usize = 1024 * 1024;
const MAX_BUFFER_LINES: usize = 10000;
const MAX_SERVICES: usize = 100;
const SERVICE_IDLE_TIMEOUT: Duration = Duration::from_secs(3600);
const CHANNEL_BUFFER_SIZE: usize = 500;
const COMPRESS_THRESHOLD: usize = 512;
const DEFAULT_RECONNECT_BUFFER_BYTES: usize = 8 * 1024;
const MAX_SIGWINCH_PHASE_ATTEMPTS: u8 = 3;

#[derive(Debug, Clone)]
enum SigwinchPhase {
    Idle,
    TempResize { retries: u8 },
    Restore { retries: u8 },
}

enum SigwinchAction {
    TempResize,
    Restore,
}

#[derive(Debug)]
enum SessionState {
    Closed,
    Active {
        pending_buffer: Option<Vec<u8>>,
        sigwinch: SigwinchPhase,
    },
}

lazy_static::lazy_static! {
    static ref TERMINAL_SERVICES: Arc<Mutex<HashMap<String, Arc<Mutex<PersistentTerminalService>>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    static ref CLEANUP_TASK: Arc<Mutex<Option<std::thread::JoinHandle<()>>>> = Arc::new(Mutex::new(None));

    static ref TERMINAL_TASKS: Arc<Mutex<Vec<Box<dyn Child + Send + Sync>>>> = Arc::new(Mutex::new(Vec::new()));
}

#[derive(Clone, Debug)]
pub struct ServiceMetadata {
    pub service_id: String,
    pub created_at: Instant,
    pub terminal_count: usize,
    pub is_persistent: bool,
}

pub fn generate_service_id() -> String {
    format!("ts_{}", uuid::Uuid::new_v4())
}

fn get_default_shell() -> String {
    #[cfg(target_os = "windows")]
    {
        super::terminal_helper::get_default_shell()
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(shell) = std::env::var("SHELL") {
            if !shell.is_empty() {
                return shell;
            }
        }

        let shells = ["/bin/bash", "/bin/zsh", "/bin/sh"];
        for shell in &shells {
            if std::path::Path::new(shell).exists() {
                return shell.to_string();
            }
        }

        "/bin/sh".to_string()
    }
}

#[cfg(target_os = "macos")]
fn locale_value_is_utf8(value: &str) -> bool {
    let value = value.to_ascii_uppercase();
    value.contains("UTF-8") || value.contains("UTF8")
}

#[cfg(target_os = "macos")]
fn should_force_process_utf8_ctype() -> bool {
    if let Ok(value) = std::env::var("LC_ALL") {
        return !locale_value_is_utf8(&value);
    }
    if let Ok(value) = std::env::var("LC_CTYPE") {
        return !locale_value_is_utf8(&value);
    }
    if let Ok(value) = std::env::var("LANG") {
        return !locale_value_is_utf8(&value);
    }
    true
}

pub fn is_service_specified_user(service_id: &str) -> Option<bool> {
    get_service(service_id).map(|s| s.lock().unwrap().is_specified_user)
}

fn get_or_create_service(
    service_id: String,
    is_persistent: bool,
    is_specified_user: bool,
) -> Result<Arc<Mutex<PersistentTerminalService>>> {
    let mut services = TERMINAL_SERVICES.lock().unwrap();

    if !services.contains_key(&service_id) && services.len() >= MAX_SERVICES {
        return Err(anyhow!(
            "Maximum number of terminal services ({}) reached",
            MAX_SERVICES
        ));
    }

    let service = services
        .entry(service_id.clone())
        .or_insert_with(|| {
            log::info!(
                "Creating new terminal service: {} (persistent: {})",
                service_id,
                is_persistent
            );
            Arc::new(Mutex::new(PersistentTerminalService::new(
                service_id.clone(),
                is_persistent,
                is_specified_user,
            )))
        })
        .clone();

    ensure_cleanup_task();

    service.lock().unwrap().reset_status(is_persistent);

    Ok(service)
}

fn remove_service(service_id: &str) {
    let mut services = TERMINAL_SERVICES.lock().unwrap();
    if let Some(service) = services.remove(service_id) {
        log::info!("Removed service: {}", service_id);
        let sessions = service.lock().unwrap().sessions.clone();
        for (_, session) in sessions.iter() {
            let mut session = session.lock().unwrap();
            session.stop();
        }
    }
}

pub fn list_services() -> Vec<ServiceMetadata> {
    let services = TERMINAL_SERVICES.lock().unwrap();
    services
        .iter()
        .filter_map(|(id, service)| {
            service.lock().ok().map(|svc| ServiceMetadata {
                service_id: id.clone(),
                created_at: svc.created_at,
                terminal_count: svc.sessions.len(),
                is_persistent: svc.is_persistent,
            })
        })
        .collect()
}

pub fn get_service(service_id: &str) -> Option<Arc<Mutex<PersistentTerminalService>>> {
    let services = TERMINAL_SERVICES.lock().unwrap();
    services.get(service_id).cloned()
}

pub fn cleanup_inactive_services() {
    let services = TERMINAL_SERVICES.lock().unwrap();
    let now = Instant::now();
    let mut to_remove = Vec::new();

    for (service_id, service) in services.iter() {
        if let Ok(svc) = service.lock() {
            if !svc.is_persistent && now.duration_since(svc.last_activity) > SERVICE_IDLE_TIMEOUT {
                to_remove.push(service_id.clone());
                log::info!("Cleaning up idle non-persistent service: {}", service_id);
            }
            else if svc.is_persistent
                && svc.sessions.is_empty()
                && now.duration_since(svc.last_activity) > SERVICE_IDLE_TIMEOUT * 2
            {
                to_remove.push(service_id.clone());
                log::info!("Cleaning up empty persistent service: {}", service_id);
            }
        }
    }

    drop(services);
    for id in to_remove {
        remove_service(&id);
    }
}

fn add_to_reaper(child: Box<dyn Child + Send + Sync>) {
    if let Ok(mut tasks) = TERMINAL_TASKS.lock() {
        tasks.push(child);
    }
}

fn check_zombie_terminals() {
    let mut tasks = match TERMINAL_TASKS.lock() {
        Ok(t) => t,
        Err(_) => return,
    };

    let mut i = 0;
    while i < tasks.len() {
        match tasks[i].try_wait() {
            Ok(Some(_)) => {
                log::info!("Process exited: {:?}", tasks[i].process_id());
                tasks.remove(i);
            }
            Ok(None) => {
                i += 1;
            }
            Err(err) => {
                log::info!(
                    "Process exited with error: {:?}, err: {err}",
                    tasks[i].process_id()
                );
                tasks.remove(i);
            }
        }
    }
}

fn ensure_cleanup_task() {
    let mut task_handle = CLEANUP_TASK.lock().unwrap();
    if task_handle.is_none() {
        let handle = std::thread::spawn(|| {
            log::info!("Started cleanup task");
            let mut last_service_cleanup = Instant::now();
            loop {
                check_zombie_terminals();

                if last_service_cleanup.elapsed() > Duration::from_secs(300) {
                    cleanup_inactive_services();
                    last_service_cleanup = Instant::now();
                }

                std::thread::sleep(Duration::from_millis(100));
            }
        });
        *task_handle = Some(handle);
    }
}

#[cfg(target_os = "linux")]
pub fn get_terminal_session_count(include_zombie_tasks: bool) -> usize {
    let mut c = TERMINAL_SERVICES.lock().unwrap().len();
    if include_zombie_tasks {
        c += TERMINAL_TASKS.lock().unwrap().len();
    }
    c
}

#[cfg(not(target_os = "windows"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserToken(pub usize);

#[cfg(not(target_os = "windows"))]
impl UserToken {
    pub fn new(handle: usize) -> Self {
        Self(handle)
    }

    pub fn as_raw(&self) -> usize {
        self.0
    }
}

#[cfg(target_os = "windows")]
pub use super::terminal_helper::UserToken;

#[derive(Clone)]
pub struct TerminalService {
    sp: GenericService,
    user_token: Option<UserToken>,
}

impl Deref for TerminalService {
    type Target = ServiceTmpl<ConnInner>;

    fn deref(&self) -> &Self::Target {
        &self.sp
    }
}

impl DerefMut for TerminalService {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.sp
    }
}

pub fn get_service_name(source: VideoSource, idx: usize) -> String {
    format!("{}{}", source.service_name_prefix(), idx)
}

pub fn new(
    service_id: String,
    is_persistent: bool,
    user_token: Option<UserToken>,
) -> GenericService {
    allow_err!(get_or_create_service(
        service_id.clone(),
        is_persistent,
        user_token.is_some()
    ));
    let svc = TerminalService {
        sp: GenericService::new(service_id.clone(), false),
        user_token,
    };
    GenericService::run(&svc.clone(), move |sp| run(sp, service_id.clone()));
    svc.sp
}

fn run(sp: TerminalService, service_id: String) -> ResultType<()> {
    while sp.ok() {
        let responses = TerminalServiceProxy::new(service_id.clone(), None, sp.user_token.clone())
            .read_outputs();
        for response in responses {
            let mut msg_out = Message::new();
            msg_out.set_terminal_response(response);
            sp.send(msg_out);
        }

        thread::sleep(Duration::from_millis(30));
    }

    if let Some(service) = get_service(&service_id) {
        let should_remove = !service.lock().unwrap().is_persistent;
        if should_remove {
            remove_service(&service_id);
        }
    }

    Ok(())
}

struct OutputBuffer {
    lines: VecDeque<Vec<u8>>,
    total_size: usize,
    last_line_incomplete: bool,
}

impl OutputBuffer {
    fn new() -> Self {
        Self {
            lines: VecDeque::new(),
            total_size: 0,
            last_line_incomplete: false,
        }
    }

    fn append(&mut self, data: &[u8]) {
        if data.is_empty() {
            return;
        }

        let mut start = 0;
        if self.last_line_incomplete {
            if let Some(last_line) = self.lines.back_mut() {
                if let Some(newline_pos) = data.iter().position(|&b| b == b'\n') {
                    last_line.extend_from_slice(&data[..=newline_pos]);
                    self.total_size += newline_pos + 1;
                    start = newline_pos + 1;
                    self.last_line_incomplete = false;
                } else {
                    last_line.extend_from_slice(data);
                    self.total_size += data.len();
                    start = data.len();
                }
            }
        }

        let remaining = &data[start..];
        let ends_with_newline = remaining.last() == Some(&b'\n');

        let lines: Vec<&[u8]> = remaining.split(|&b| b == b'\n').collect();

        for (i, line) in lines.iter().enumerate() {
            if i == lines.len() - 1 && !ends_with_newline && !line.is_empty() {
                self.last_line_incomplete = true;
            }

            if !line.is_empty() || i < lines.len() - 1 {
                let mut line_data = line.to_vec();
                if i < lines.len() - 1 || ends_with_newline {
                    line_data.push(b'\n');
                }

                self.total_size += line_data.len();
                self.lines.push_back(line_data);
            }
        }

        while self.total_size > MAX_OUTPUT_BUFFER_SIZE || self.lines.len() > MAX_BUFFER_LINES {
            if self.lines.len() == 1 && self.last_line_incomplete {
                let line = &mut self.lines[0];
                line.drain(..line.len().saturating_sub(MAX_OUTPUT_BUFFER_SIZE / 2));
                self.total_size = line.len();
                break;
            }
            if let Some(removed) = self.lines.pop_front() {
                if removed.len() > self.total_size {
                    log::error!(
                        "OutputBuffer total_size underflow avoided: total_size={}, removed_len={}, lines_len={}",
                        self.total_size,
                        removed.len(),
                        self.lines.len()
                    );
                    self.total_size = self.lines.iter().map(|line| line.len()).sum();
                } else {
                    self.total_size -= removed.len();
                }
                if self.lines.is_empty() {
                    self.last_line_incomplete = false;
                }
            } else {
                log::error!(
                    "OutputBuffer trim invariant broken: total_size={}, lines_len=0",
                    self.total_size
                );
                self.total_size = 0;
                self.last_line_incomplete = false;
                break;
            }
        }
    }

    fn get_recent(&self, max_bytes: usize) -> Vec<u8> {
        if max_bytes == 0 {
            return Vec::new();
        }
        let mut chunks: Vec<&[u8]> = Vec::new();
        let mut size = 0;

        for line in self.lines.iter().rev() {
            if size + line.len() > max_bytes {
                if size == 0 && line.len() > max_bytes {
                    let mut offset = line.len() - max_bytes;
                    let mut skipped = 0u8;
                    while skipped < 3
                        && offset < line.len()
                        && (line[offset] & 0b1100_0000) == 0b1000_0000
                    {
                        offset += 1;
                        skipped += 1;
                    }
                    if offset < line.len() {
                        chunks.push(&line[offset..]);
                        size = line.len() - offset;
                    }
                }
                break;
            }
            size += line.len();
            chunks.push(line);
        }

        chunks.reverse();
        let mut result = Vec::with_capacity(size);
        for chunk in chunks {
            result.extend_from_slice(chunk);
        }

        result
    }
}

fn find_utf8_split_point(buf: &[u8]) -> usize {
    if buf.is_empty() {
        return 0;
    }

    let start = buf.len().saturating_sub(3);
    for i in (start..buf.len()).rev() {
        let b = buf[i];
        if b & 0x80 == 0 {
            return buf.len();
        }
        if b & 0xC0 == 0x80 {
            continue;
        }

        let seq_len = if b & 0xE0 == 0xC0 {
            2
        } else if b & 0xF0 == 0xE0 {
            3
        } else if b & 0xF8 == 0xF0 {
            4
        } else {
            return buf.len();
        };

        return if buf.len() - i >= seq_len {
            buf.len()
        } else {
            i
        };
    }

    buf.len()
}

#[derive(Default)]
struct Utf8ChunkAccumulator {
    remainder: Vec<u8>,
}

impl Utf8ChunkAccumulator {
    fn push_chunk(&mut self, mut data: Vec<u8>) -> Option<Vec<u8>> {
        if data.is_empty() {
            return None;
        }

        let had_remainder = !self.remainder.is_empty();
        if had_remainder {
            let mut combined = std::mem::take(&mut self.remainder);
            combined.extend_from_slice(&data);
            data = combined;
        }

        let split = find_utf8_split_point(&data);
        if split == data.len() {
            return Some(data);
        }

        if !had_remainder && split > 0 && std::str::from_utf8(&data[..split]).is_err() {
            return Some(data);
        }

        self.remainder = data.split_off(split);
        if data.is_empty() {
            None
        } else {
            Some(data)
        }
    }

    fn finish(&mut self) -> Option<Vec<u8>> {
        if self.remainder.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.remainder))
        }
    }
}

fn try_send_output(
    output_tx: &mpsc::SyncSender<Vec<u8>>,
    data: Vec<u8>,
    terminal_id: i32,
    label: &str,
    drop_count: &mut u64,
    last_drop_warn: &mut Instant,
) -> bool {
    match output_tx.try_send(data) {
        Ok(_) => {
            if *drop_count > 0 {
                log::trace!(
                    "Terminal {}{} output channel recovered, dropped {} chunks since last report",
                    terminal_id,
                    label,
                    *drop_count
                );
                *drop_count = 0;
            }
            false
        }
        Err(mpsc::TrySendError::Full(_)) => {
            *drop_count += 1;
            if last_drop_warn.elapsed() >= Duration::from_secs(5) {
                log::trace!(
                    "Terminal {}{} output channel full, dropped {} chunks in last {:?}",
                    terminal_id,
                    label,
                    *drop_count,
                    last_drop_warn.elapsed()
                );
                *drop_count = 0;
                *last_drop_warn = Instant::now();
            }
            false
        }
        Err(mpsc::TrySendError::Disconnected(_)) => {
            log::debug!(
                "Terminal {}{} output channel disconnected",
                terminal_id,
                label
            );
            true
        }
    }
}

pub struct TerminalSession {
    pub created_at: Instant,
    last_activity: Instant,
    pty_pair: Option<portable_pty::PtyPair>,
    child: Option<Box<dyn Child + std::marker::Send + Sync>>,
    input_tx: Option<SyncSender<Vec<u8>>>,
    output_rx: Option<Receiver<Vec<u8>>>,
    exiting: Arc<AtomicBool>,
    reader_thread: Option<thread::JoinHandle<()>>,
    writer_thread: Option<thread::JoinHandle<()>>,
    output_buffer: OutputBuffer,
    title: String,
    pid: u32,
    rows: u16,
    cols: u16,
    closed_message_sent: bool,
    state: SessionState,
    #[cfg(target_os = "windows")]
    is_helper_mode: bool,
    #[cfg(target_os = "windows")]
    helper_process_handle: Option<SendableHandle>,
}

impl TerminalSession {
    fn new(terminal_id: i32, rows: u16, cols: u16) -> Self {
        Self {
            created_at: Instant::now(),
            last_activity: Instant::now(),
            pty_pair: None,
            child: None,
            input_tx: None,
            output_rx: None,
            exiting: Arc::new(AtomicBool::new(false)),
            reader_thread: None,
            writer_thread: None,
            output_buffer: OutputBuffer::new(),
            title: format!("Terminal {}", terminal_id),
            pid: 0,
            rows,
            cols,
            closed_message_sent: false,
            state: SessionState::Closed,
            #[cfg(target_os = "windows")]
            is_helper_mode: false,
            #[cfg(target_os = "windows")]
            helper_process_handle: None,
        }
    }

    fn update_activity(&mut self) {
        self.last_activity = Instant::now();
    }

    fn stop(&mut self) {
        self.state = SessionState::Closed;
        self.exiting.store(true, Ordering::SeqCst);

        if let Some(input_tx) = self.input_tx.take() {
            #[cfg(target_os = "windows")]
            let final_msg = if self.is_helper_mode {
                encode_helper_message(MSG_TYPE_DATA, b"\r\n")
            } else {
                b"\r\n".to_vec()
            };
            #[cfg(not(target_os = "windows"))]
            let final_msg = b"\r\n".to_vec();

            if let Err(e) = input_tx.send(final_msg) {
                log::warn!("Failed to send final newline to the terminal: {}", e);
            }
            drop(input_tx);
        }
        self.output_rx = None;

        #[cfg(target_os = "windows")]
        if self.is_helper_mode {
            if let Some(helper_handle) = self.helper_process_handle.take() {
                let handle = helper_handle.as_raw();
                log::debug!("Helper mode: terminating helper process before joining threads...");

                let wait_result = unsafe { WinWaitForSingleObject(handle, 100) };

                if wait_result == WIN_WAIT_OBJECT_0 {
                    log::debug!("Helper process exited gracefully");
                } else {
                    log::debug!("Force terminating helper process to unblock reader thread");
                    unsafe {
                        let _ = WinTerminateProcess(handle, 0);
                    }
                }

                unsafe {
                    let _ = WinCloseHandle(handle);
                }
            }
        }

        #[cfg(any(target_os = "windows", target_os = "linux"))]
        {
            self.pty_pair = None;
        }

        if let Some(reader_thread) = self.reader_thread.take() {
            let _ = reader_thread.join();
        }

        if let Some(writer_thread) = self.writer_thread.take() {
            let _ = writer_thread.join();
        }

        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            add_to_reaper(child);
        }
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        self.stop();
    }
}

pub struct PersistentTerminalService {
    service_id: String,
    sessions: HashMap<i32, Arc<Mutex<TerminalSession>>>,
    pub created_at: Instant,
    last_activity: Instant,
    pub is_persistent: bool,
    needs_session_sync: bool,
    is_specified_user: bool,
}

impl PersistentTerminalService {
    pub fn new(service_id: String, is_persistent: bool, is_specified_user: bool) -> Self {
        Self {
            service_id,
            sessions: HashMap::new(),
            created_at: Instant::now(),
            last_activity: Instant::now(),
            is_persistent,
            needs_session_sync: false,
            is_specified_user,
        }
    }

    fn update_activity(&mut self) {
        self.last_activity = Instant::now();
    }

    pub fn list_terminals(&self) -> Vec<(i32, String, u32, Instant)> {
        self.sessions
            .iter()
            .map(|(id, session)| {
                let s = session.lock().unwrap();
                (*id, s.title.clone(), s.pid, s.created_at)
            })
            .collect()
    }

    pub fn get_terminal_buffer(&self, terminal_id: i32, max_bytes: usize) -> Option<Vec<u8>> {
        self.sessions.get(&terminal_id).map(|session| {
            let session = session.lock().unwrap();
            session.output_buffer.get_recent(max_bytes)
        })
    }

    pub fn get_terminal_info(&self, terminal_id: i32) -> Option<(u16, u16, Vec<u8>)> {
        self.sessions.get(&terminal_id).map(|session| {
            let session = session.lock().unwrap();
            (
                session.rows,
                session.cols,
                session
                    .output_buffer
                    .get_recent(DEFAULT_RECONNECT_BUFFER_BYTES),
            )
        })
    }

    pub fn has_active_terminals(&self) -> bool {
        !self.sessions.is_empty()
    }

    fn reset_status(&mut self, is_persistent: bool) {
        self.is_persistent = is_persistent;
        self.needs_session_sync = true;
        for session in self.sessions.values() {
            let mut session = session.lock().unwrap();
            session.state = SessionState::Closed;
        }
    }
}

pub struct TerminalServiceProxy {
    service_id: String,
    is_persistent: bool,
    #[cfg(target_os = "windows")]
    user_token: Option<UserToken>,
}

pub fn set_persistent(service_id: &str, is_persistent: bool) -> Result<()> {
    if let Some(service) = get_service(service_id) {
        service.lock().unwrap().is_persistent = is_persistent;
        Ok(())
    } else {
        Err(anyhow!("Service {} not found", service_id))
    }
}

impl TerminalServiceProxy {
    pub fn new(
        service_id: String,
        is_persistent: Option<bool>,
        _user_token: Option<UserToken>,
    ) -> Self {
        let is_persistent =
            is_persistent.unwrap_or(if let Some(service) = get_service(&service_id) {
                service.lock().unwrap().is_persistent
            } else {
                false
            });
        TerminalServiceProxy {
            service_id,
            is_persistent,
            #[cfg(target_os = "windows")]
            user_token: _user_token,
        }
    }

    pub fn get_service_id(&self) -> &str {
        &self.service_id
    }

    pub fn handle_action(&mut self, action: &TerminalAction) -> Result<Option<TerminalResponse>> {
        let service = match get_service(&self.service_id) {
            Some(s) => s,
            None => {
                let mut response = TerminalResponse::new();
                let mut error = TerminalError::new();
                error.message = format!("Terminal service {} not found", self.service_id);
                response.set_error(error);
                return Ok(Some(response));
            }
        };
        service.lock().unwrap().update_activity();
        match &action.union {
            Some(terminal_action::Union::Open(open)) => {
                self.handle_open(&mut service.lock().unwrap(), open)
            }
            Some(terminal_action::Union::Resize(resize)) => {
                let session = service
                    .lock()
                    .unwrap()
                    .sessions
                    .get(&resize.terminal_id)
                    .cloned();
                self.handle_resize(session, resize)
            }
            Some(terminal_action::Union::Data(data)) => {
                let session = service
                    .lock()
                    .unwrap()
                    .sessions
                    .get(&data.terminal_id)
                    .cloned();
                self.handle_data(session, data)
            }
            Some(terminal_action::Union::Close(close)) => {
                self.handle_close(&mut service.lock().unwrap(), close)
            }
            _ => Ok(None),
        }
    }

    fn handle_open(
        &self,
        service: &mut PersistentTerminalService,
        open: &OpenTerminal,
    ) -> Result<Option<TerminalResponse>> {
        let mut response = TerminalResponse::new();

        if !service.sessions.contains_key(&open.terminal_id)
            && service.needs_session_sync
            && !service.sessions.is_empty()
        {
            if let Some(&lowest_id) = service.sessions.keys().min() {
                log::info!(
                    "Remapping persistent session {} -> {} for reconnection",
                    lowest_id,
                    open.terminal_id
                );
                if let Some(session_arc) = service.sessions.remove(&lowest_id) {
                    service.sessions.insert(open.terminal_id, session_arc);
                }
            }
        }

        if let Some(session_arc) = service.sessions.get(&open.terminal_id) {
            let mut session = session_arc.lock().unwrap();
            let mut buffer = session
                .output_buffer
                .get_recent(DEFAULT_RECONNECT_BUFFER_BYTES);
            let mut reconnect_backlog = Vec::new();
            if let Some(output_rx) = &session.output_rx {
                for _ in 0..CHANNEL_BUFFER_SIZE {
                    let Ok(data) = output_rx.try_recv() else {
                        break;
                    };
                    reconnect_backlog.push(data);
                }
            }
            let has_reconnect_backlog = !reconnect_backlog.is_empty();
            for data in reconnect_backlog {
                session.output_buffer.append(&data);
            }
            if has_reconnect_backlog {
                buffer = session
                    .output_buffer
                    .get_recent(DEFAULT_RECONNECT_BUFFER_BYTES);
            }
            let has_pending = !buffer.is_empty();
            session.state = SessionState::Active {
                pending_buffer: if has_pending { Some(buffer) } else { None },
                sigwinch: SigwinchPhase::TempResize {
                    retries: MAX_SIGWINCH_PHASE_ATTEMPTS,
                },
            };
            let mut opened = TerminalOpened::new();
            opened.terminal_id = open.terminal_id;
            opened.success = true;
            opened.message = if has_pending {
                "Reconnected to existing terminal with pending output".to_string()
            } else {
                "Reconnected to existing terminal".to_string()
            };
            opened.pid = session.pid;
            opened.service_id = self.service_id.clone();
            opened.replay_terminal_output = has_pending;
            if service.needs_session_sync {
                if service.sessions.len() > 1 {
                    opened.persistent_sessions = service
                        .sessions
                        .keys()
                        .filter(|&id| *id != open.terminal_id)
                        .cloned()
                        .collect();
                }
                service.needs_session_sync = false;
            }
            response.set_opened(opened);

            return Ok(Some(response));
        }

        #[cfg(target_os = "windows")]
        if self.user_token.is_some() {
            return self.handle_open_with_helper(service, open);
        }

        log::info!(
            "Creating new terminal {} for service {}",
            open.terminal_id,
            service.service_id
        );
        let mut session =
            TerminalSession::new(open.terminal_id, open.rows as u16, open.cols as u16);

        let pty_size = PtySize {
            rows: open.rows as u16,
            cols: open.cols as u16,
            pixel_width: 0,
            pixel_height: 0,
        };

        log::debug!("Opening PTY with size: {}x{}", open.rows, open.cols);
        let pty_system = portable_pty::native_pty_system();
        let pty_pair = pty_system.openpty(pty_size).context("Failed to open PTY")?;

        let shell = get_default_shell();
        log::debug!("Using shell: {}", shell);

        #[allow(unused_mut)]
        let mut cmd = CommandBuilder::new(&shell);

        #[cfg(target_os = "windows")]
        configure_utf8_shell_command(&shell, &mut cmd);

        #[cfg(target_os = "macos")]
        {
            cmd.arg("-l");
            log::debug!("Added -l flag for macOS login shell");

            let term = if std::path::Path::new("/usr/share/terminfo/78/xterm-256color").exists() {
                "xterm-256color"
            } else {
                "xterm"
            };
            cmd.env("TERM", term);
            log::debug!("Set TERM={} for macOS PTY", term);

            if should_force_process_utf8_ctype() {
                cmd.env_remove("LC_ALL");
                cmd.env("LC_CTYPE", "en_US.UTF-8");
                log::debug!("Set LC_CTYPE=en_US.UTF-8 for macOS PTY");
            }
        }


        log::debug!("Spawning shell process...");
        let child = pty_pair
            .slave
            .spawn_command(cmd)
            .context("Failed to spawn command")?;

        let writer = pty_pair
            .master
            .take_writer()
            .context("Failed to get writer")?;

        let reader = pty_pair
            .master
            .try_clone_reader()
            .context("Failed to get reader")?;

        session.pid = child.process_id().unwrap_or(0) as u32;

        let (input_tx, input_rx) = mpsc::sync_channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);
        let (output_tx, output_rx) = mpsc::sync_channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);

        let terminal_id = open.terminal_id;
        let writer_thread = thread::spawn(move || {
            let mut writer = writer;
            while let Ok(data) = input_rx.recv() {
                if let Err(e) = writer.write_all(&data) {
                    log::error!("Terminal {} write error: {}", terminal_id, e);
                    break;
                }
                if let Err(e) = writer.flush() {
                    log::error!("Terminal {} flush error: {}", terminal_id, e);
                }
            }
            log::debug!("Terminal {} writer thread exiting", terminal_id);
        });

        let exiting = session.exiting.clone();
        let terminal_id = open.terminal_id;
        let reader_thread = thread::spawn(move || {
            let mut reader = reader;
            let mut buf = vec![0u8; 4096];
            let mut utf8_chunks = Utf8ChunkAccumulator::default();
            let mut drop_count: u64 = 0;
            let mut last_drop_warn = Instant::now() - Duration::from_secs(6);
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        if let Some(data) = utf8_chunks.finish() {
                            let _ = try_send_output(
                                &output_tx,
                                data,
                                terminal_id,
                                "",
                                &mut drop_count,
                                &mut last_drop_warn,
                            );
                        }
                        break;
                    }
                    Ok(n) => {
                        if exiting.load(Ordering::SeqCst) {
                            break;
                        }
                        let Some(data) = utf8_chunks.push_chunk(buf[..n].to_vec()) else {
                            continue;
                        };
                        if try_send_output(
                            &output_tx,
                            data,
                            terminal_id,
                            "",
                            &mut drop_count,
                            &mut last_drop_warn,
                        ) {
                            break;
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        if exiting.load(Ordering::SeqCst) {
                            break;
                        }
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(e) => {
                        log::error!("Terminal {} read error: {}", terminal_id, e);
                        break;
                    }
                }
            }
            log::debug!("Terminal {} reader thread exiting", terminal_id);
        });

        session.pty_pair = Some(pty_pair);
        session.child = Some(child);
        session.input_tx = Some(input_tx);
        session.output_rx = Some(output_rx);
        session.reader_thread = Some(reader_thread);
        session.writer_thread = Some(writer_thread);
        session.state = SessionState::Active {
            pending_buffer: None,
            sigwinch: SigwinchPhase::Idle,
        };

        let mut opened = TerminalOpened::new();
        opened.terminal_id = open.terminal_id;
        opened.success = true;
        opened.message = "Terminal opened".to_string();
        opened.pid = session.pid;
        opened.service_id = service.service_id.clone();
        if service.needs_session_sync {
            if !service.sessions.is_empty() {
                opened.persistent_sessions = service.sessions.keys().cloned().collect();
            }
            service.needs_session_sync = false;
        }
        response.set_opened(opened);

        log::info!(
            "Terminal {} opened successfully with PID {}",
            open.terminal_id,
            session.pid
        );

        service
            .sessions
            .insert(open.terminal_id, Arc::new(Mutex::new(session)));

        Ok(Some(response))
    }

    #[cfg(target_os = "windows")]
    fn handle_open_with_helper(
        &self,
        service: &mut PersistentTerminalService,
        open: &OpenTerminal,
    ) -> Result<Option<TerminalResponse>> {
        let mut response = TerminalResponse::new();

        log::info!(
            "Creating new terminal {} using helper process for service: {}",
            open.terminal_id,
            service.service_id
        );

        let mut session =
            TerminalSession::new(open.terminal_id, open.rows as u16, open.cols as u16);

        let pipe_id = uuid::Uuid::new_v4();
        let input_pipe_name = format!(r"\\.\pipe\supportdesk_term_in_{}", pipe_id);
        let output_pipe_name = format!(r"\\.\pipe\supportdesk_term_out_{}", pipe_id);

        log::debug!(
            "Creating pipes: input={}, output={}",
            input_pipe_name,
            output_pipe_name
        );

        let user_token = self
            .user_token
            .ok_or_else(|| anyhow!("user_token is required for helper mode"))?;

        let input_pipe_handle = OwnedHandle::new(create_named_pipe_server(
            &input_pipe_name,
            false,
            user_token,
        )?);
        let output_pipe_handle = OwnedHandle::new(create_named_pipe_server(
            &output_pipe_name,
            true,
            user_token,
        )?);

        let helper_process_info = launch_terminal_helper_with_token(
            user_token,
            &input_pipe_name,
            &output_pipe_name,
            open.terminal_id,
            open.rows as u16,
            open.cols as u16,
        )?;

        let helper_process_guard =
            HelperProcessGuard::new(helper_process_info.handle, helper_process_info.pid);
        let helper_pid = helper_process_guard.pid();

        let mut input_pipe = wait_for_pipe_connection(
            input_pipe_handle,
            &input_pipe_name,
            PIPE_CONNECTION_TIMEOUT_MS,
        )?;
        let mut output_pipe = wait_for_pipe_connection(
            output_pipe_handle,
            &output_pipe_name,
            PIPE_CONNECTION_TIMEOUT_MS,
        )?;

        if !is_helper_process_running(helper_process_guard.as_raw()) {
            return Err(anyhow!(
                "Helper process (PID {}) exited unexpectedly after pipe connection",
                helper_pid
            ));
        }

        let helper_raw_handle = helper_process_guard.disarm();

        session.pid = helper_pid;

        let (input_tx, input_rx) = mpsc::sync_channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);
        let (output_tx, output_rx) = mpsc::sync_channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);

        let terminal_id = open.terminal_id;
        let writer_thread = thread::spawn(move || {
            while let Ok(data) = input_rx.recv() {
                if let Err(e) = input_pipe.write_all(&data) {
                    log::error!("Terminal {} pipe write error: {}", terminal_id, e);
                    break;
                }
                if let Err(e) = input_pipe.flush() {
                    log::error!("Terminal {} pipe flush error: {}", terminal_id, e);
                }
            }
            log::debug!(
                "Terminal {} writer thread (helper mode) exiting",
                terminal_id
            );
        });

        let exiting = session.exiting.clone();
        let terminal_id = open.terminal_id;
        let reader_thread = thread::spawn(move || {
            let mut buf = vec![0u8; 4096];
            let mut utf8_chunks = Utf8ChunkAccumulator::default();
            let mut drop_count: u64 = 0;
            let mut last_drop_warn = Instant::now() - Duration::from_secs(6);
            loop {
                match output_pipe.read(&mut buf) {
                    Ok(0) => {
                        if let Some(data) = utf8_chunks.finish() {
                            let _ = try_send_output(
                                &output_tx,
                                data,
                                terminal_id,
                                " (helper)",
                                &mut drop_count,
                                &mut last_drop_warn,
                            );
                        }
                        log::debug!("Terminal {} helper output EOF", terminal_id);
                        break;
                    }
                    Ok(n) => {
                        if exiting.load(Ordering::SeqCst) {
                            break;
                        }
                        let Some(data) = utf8_chunks.push_chunk(buf[..n].to_vec()) else {
                            continue;
                        };
                        if try_send_output(
                            &output_tx,
                            data,
                            terminal_id,
                            " (helper)",
                            &mut drop_count,
                            &mut last_drop_warn,
                        ) {
                            break;
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        if exiting.load(Ordering::SeqCst) {
                            break;
                        }
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(e) => {
                        log::error!("Terminal {} pipe read error: {}", terminal_id, e);
                        break;
                    }
                }
            }
            log::debug!(
                "Terminal {} reader thread (helper mode) exiting",
                terminal_id
            );
        });

        session.pty_pair = None;
        session.child = None;
        session.input_tx = Some(input_tx);
        session.output_rx = Some(output_rx);
        session.reader_thread = Some(reader_thread);
        session.writer_thread = Some(writer_thread);
        session.state = SessionState::Active {
            pending_buffer: None,
            sigwinch: SigwinchPhase::Idle,
        };
        session.is_helper_mode = true;
        session.helper_process_handle = Some(SendableHandle::new(helper_raw_handle));

        let mut opened = TerminalOpened::new();
        opened.terminal_id = open.terminal_id;
        opened.success = true;
        opened.message = "Terminal opened (helper mode)".to_string();
        opened.pid = session.pid;
        opened.service_id = service.service_id.clone();
        if service.needs_session_sync {
            if !service.sessions.is_empty() {
                opened.persistent_sessions = service.sessions.keys().cloned().collect();
            }
            service.needs_session_sync = false;
        }
        response.set_opened(opened);

        log::info!(
            "Terminal {} opened successfully using helper process (PID {})",
            open.terminal_id,
            session.pid
        );

        service
            .sessions
            .insert(open.terminal_id, Arc::new(Mutex::new(session)));

        Ok(Some(response))
    }

    fn handle_resize(
        &self,
        session: Option<Arc<Mutex<TerminalSession>>>,
        resize: &ResizeTerminal,
    ) -> Result<Option<TerminalResponse>> {
        if let Some(session_arc) = session {
            let mut session = session_arc.lock().unwrap();
            session.update_activity();
            session.rows = resize.rows as u16;
            session.cols = resize.cols as u16;


            #[cfg(target_os = "windows")]
            {
                if session.is_helper_mode {
                    if let Some(input_tx) = &session.input_tx {
                        let msg = encode_resize_message(resize.rows as u16, resize.cols as u16);
                        if let Err(e) = input_tx.send(msg) {
                            log::error!("Failed to send resize to helper: {}", e);
                        }
                    } else {
                        log::warn!(
                            "Terminal {} is in helper mode but input_tx is None, cannot send resize",
                            resize.terminal_id
                        );
                    }
                } else {
                    Self::resize_pty(&session, resize)?;
                }
            }

            #[cfg(not(target_os = "windows"))]
            {
                Self::resize_pty(&session, resize)?;
            }
        }
        Ok(None)
    }

    fn resize_pty(session: &TerminalSession, resize: &ResizeTerminal) -> Result<()> {
        if let Some(pty_pair) = &session.pty_pair {
            pty_pair.master.resize(PtySize {
                rows: resize.rows as u16,
                cols: resize.cols as u16,
                pixel_width: 0,
                pixel_height: 0,
            })?;
        }
        Ok(())
    }

    fn handle_data(
        &self,
        session: Option<Arc<Mutex<TerminalSession>>>,
        data: &TerminalData,
    ) -> Result<Option<TerminalResponse>> {
        if let Some(session_arc) = session {
            let input = {
                let mut session = session_arc.lock().unwrap();
                session.update_activity();
                if let Some(input_tx) = session.input_tx.clone() {
                    #[cfg(target_os = "windows")]
                    let msg = if session.is_helper_mode {
                        encode_helper_message(MSG_TYPE_DATA, &data.data)
                    } else {
                        data.data.to_vec()
                    };
                    #[cfg(not(target_os = "windows"))]
                    let msg = data.data.to_vec();

                    Some((input_tx, msg))
                } else {
                    None
                }
            };

            if let Some((input_tx, msg)) = input {
                if let Err(e) = input_tx.send(msg) {
                    log::error!(
                        "Failed to send data to terminal {}: {}",
                        data.terminal_id,
                        e
                    );
                }
            }
        }

        Ok(None)
    }

    fn handle_close(
        &self,
        service: &mut PersistentTerminalService,
        close: &CloseTerminal,
    ) -> Result<Option<TerminalResponse>> {
        let mut response = TerminalResponse::new();

        if let Some(session_arc) = service.sessions.remove(&close.terminal_id) {
            let mut session = session_arc.lock().unwrap();
            let exit_code = if let Some(mut child) = session.child.take() {
                child.kill()?;
                add_to_reaper(child);
                -1
            } else {
                0
            };

            let mut closed = TerminalClosed::new();
            closed.terminal_id = close.terminal_id;
            closed.exit_code = exit_code;
            response.set_closed(closed);
            Ok(Some(response))
        } else {
            Ok(None)
        }
    }

    fn do_sigwinch_resize(
        terminal_id: i32,
        rows: u16,
        cols: u16,
        pty_pair: &Option<portable_pty::PtyPair>,
        input_tx: &Option<SyncSender<Vec<u8>>>,
        _is_helper_mode: bool,
        action: &SigwinchAction,
    ) -> bool {
        if rows == 0 || cols == 0 {
            return false;
        }

        let target_rows = match action {
            SigwinchAction::TempResize => {
                if rows > 2 {
                    rows.saturating_sub(1)
                } else {
                    rows.saturating_add(1)
                }
            }
            SigwinchAction::Restore => rows,
        };

        let phase_name = match action {
            SigwinchAction::TempResize => "temp resize",
            SigwinchAction::Restore => "restore",
        };

        #[cfg(target_os = "windows")]
        let use_helper = _is_helper_mode;
        #[cfg(not(target_os = "windows"))]
        let use_helper = false;

        if use_helper {
            #[cfg(target_os = "windows")]
            {
                let input_tx = match input_tx {
                    Some(tx) => tx,
                    None => return false,
                };
                let msg = encode_resize_message(target_rows, cols);
                if let Err(e) = input_tx.try_send(msg) {
                    log::warn!(
                        "Terminal {} SIGWINCH {} via helper failed: {}",
                        terminal_id,
                        phase_name,
                        e
                    );
                    return false;
                }
                true
            }
            #[cfg(not(target_os = "windows"))]
            {
                let _ = (input_tx, phase_name);
                false
            }
        } else if let Some(pty_pair) = pty_pair {
            if let Err(e) = pty_pair.master.resize(PtySize {
                rows: target_rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            }) {
                log::warn!(
                    "Terminal {} SIGWINCH {} failed: {}",
                    terminal_id,
                    phase_name,
                    e
                );
                return false;
            }
            true
        } else {
            false
        }
    }

    fn create_terminal_data_response(terminal_id: i32, data: Vec<u8>) -> TerminalResponse {
        let mut response = TerminalResponse::new();
        let mut terminal_data = TerminalData::new();
        terminal_data.terminal_id = terminal_id;

        if data.len() > COMPRESS_THRESHOLD {
            let compressed = compress::compress(&data);
            if compressed.len() < data.len() {
                terminal_data.data = bytes::Bytes::from(compressed);
                terminal_data.compressed = true;
            } else {
                terminal_data.data = bytes::Bytes::from(data);
            }
        } else {
            terminal_data.data = bytes::Bytes::from(data);
        }

        response.set_data(terminal_data);
        response
    }

    pub fn read_outputs(&self) -> Vec<TerminalResponse> {
        let service = match get_service(&self.service_id) {
            Some(s) => s,
            None => {
                return vec![];
            }
        };

        let sessions: Vec<(i32, Arc<Mutex<TerminalSession>>)> = {
            let service = service.lock().unwrap();
            service
                .sessions
                .iter()
                .map(|(id, session)| (*id, session.clone()))
                .collect()
        };

        let mut responses = Vec::new();
        let mut closed_terminals = Vec::new();

        for (terminal_id, session_arc) in sessions {
            if let Ok(mut session) = session_arc.try_lock() {
                let mut should_send_closed = false;
                if !session.closed_message_sent {
                    if let Some(thread) = &session.reader_thread {
                        if thread.is_finished() {
                            should_send_closed = true;
                        }
                    }
                    if !should_send_closed {
                        if let Some(child) = &mut session.child {
                            match child.try_wait() {
                                Ok(Some(_)) => {
                                    should_send_closed = true;
                                }
                                Ok(None) => {}
                                Err(e) => {
                                    log::warn!("Terminal {} child wait error: {}", terminal_id, e);
                                }
                            }
                        }
                    }
                    if should_send_closed {
                        session.closed_message_sent = true;
                    }
                }
                if should_send_closed {
                    closed_terminals.push(terminal_id);
                }

                let mut has_activity = false;
                let mut received_data = Vec::new();
                if let Some(output_rx) = &session.output_rx {
                    while let Ok(data) = output_rx.try_recv() {
                        has_activity = true;
                        received_data.push(data);
                    }
                }

                for data in &received_data {
                    session.output_buffer.append(data);
                }

                let (replay_buffer, sigwinch_action) = {
                    let (pending_buffer, sigwinch) = match &mut session.state {
                        SessionState::Active {
                            pending_buffer,
                            sigwinch,
                        } => (pending_buffer, sigwinch),
                        _ => continue,
                    };

                    let replay_buffer = pending_buffer.take();

                    let sigwinch_action = match sigwinch {
                        SigwinchPhase::TempResize { retries } => {
                            if *retries == 0 {
                                log::warn!(
                                    "Terminal {} SIGWINCH phase 1 (temp resize) failed after {} attempts, giving up",
                                    terminal_id, MAX_SIGWINCH_PHASE_ATTEMPTS
                                );
                                *sigwinch = SigwinchPhase::Idle;
                                None
                            } else {
                                *retries -= 1;
                                Some(SigwinchAction::TempResize)
                            }
                        }
                        SigwinchPhase::Restore { retries } => {
                            if *retries == 0 {
                                log::warn!(
                                    "Terminal {} SIGWINCH phase 2 (restore) failed after {} attempts, giving up",
                                    terminal_id, MAX_SIGWINCH_PHASE_ATTEMPTS
                                );
                                *sigwinch = SigwinchPhase::Idle;
                                None
                            } else {
                                *retries -= 1;
                                Some(SigwinchAction::Restore)
                            }
                        }
                        SigwinchPhase::Idle => None,
                    };
                    (replay_buffer, sigwinch_action)
                };

                if let Some(buffer) = replay_buffer {
                    if !buffer.is_empty() {
                        responses.push(Self::create_terminal_data_response(terminal_id, buffer));
                    }
                }

                if has_activity {
                    session.update_activity();
                }

                if let Some(action) = sigwinch_action {
                    #[cfg(target_os = "windows")]
                    let is_helper = session.is_helper_mode;
                    #[cfg(not(target_os = "windows"))]
                    let is_helper = false;
                    let resize_ok = Self::do_sigwinch_resize(
                        terminal_id,
                        session.rows,
                        session.cols,
                        &session.pty_pair,
                        &session.input_tx,
                        is_helper,
                        &action,
                    );
                    if let SessionState::Active { sigwinch, .. } = &mut session.state {
                        match action {
                            SigwinchAction::TempResize => {
                                if resize_ok {
                                    *sigwinch = SigwinchPhase::Restore {
                                        retries: MAX_SIGWINCH_PHASE_ATTEMPTS,
                                    };
                                }
                            }
                            SigwinchAction::Restore => {
                                if resize_ok {
                                    *sigwinch = SigwinchPhase::Idle;
                                }
                            }
                        }
                    }
                }

                for data in received_data {
                    responses.push(Self::create_terminal_data_response(terminal_id, data));
                }
            }
        }

        if !closed_terminals.is_empty() {
            let mut sessions = service.lock().unwrap().sessions.clone();
            for terminal_id in closed_terminals {
                let mut exit_code = 0;

                if !self.is_persistent {
                    if let Some(session_arc) = sessions.remove(&terminal_id) {
                        service.lock().unwrap().sessions.remove(&terminal_id);
                        let mut session = session_arc.lock().unwrap();
                        if let Some(mut child) = session.child.take() {
                            if let Ok(Some(status)) = child.try_wait() {
                                exit_code = status.exit_code() as i32;
                            }
                            add_to_reaper(child);
                        }
                    }
                } else {
                    if let Some(session_arc) = sessions.get(&terminal_id) {
                        let mut session = session_arc.lock().unwrap();
                        if let Some(mut child) = session.child.take() {
                            if let Ok(Some(status)) = child.try_wait() {
                                exit_code = status.exit_code() as i32;
                            }
                            add_to_reaper(child);
                        }
                        if session.closed_message_sent {
                            drop(session);
                            sessions.remove(&terminal_id);
                            service.lock().unwrap().sessions.remove(&terminal_id);
                        }
                    }
                }

                let mut response = TerminalResponse::new();
                let mut closed = TerminalClosed::new();
                closed.terminal_id = terminal_id;
                closed.exit_code = exit_code;
                response.set_closed(closed);
                responses.push(response);
            }
        }

        responses
    }

    pub fn on_disconnect(&self) {
        if !self.is_persistent {
            remove_service(&self.service_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{OutputBuffer, Utf8ChunkAccumulator, MAX_BUFFER_LINES};

    #[test]
    fn utf8_chunk_accumulator_buffers_leading_split_multibyte_output() {
        let mut chunker = Utf8ChunkAccumulator::default();

        assert!(chunker.push_chunk(vec![0xE4]).is_none());
        assert!(chunker.push_chunk(vec![0xB8]).is_none());
        assert_eq!(
            chunker.push_chunk(vec![0xAD]),
            Some("中".as_bytes().to_vec())
        );
        assert!(chunker.finish().is_none());
    }

    #[test]
    fn utf8_chunk_accumulator_flushes_incomplete_tail_on_finish() {
        let mut chunker = Utf8ChunkAccumulator::default();
        assert_eq!(chunker.push_chunk(vec![b'a', 0xE4]), Some(vec![b'a']));
        assert_eq!(chunker.finish(), Some(vec![0xE4]));
        assert!(chunker.finish().is_none());
    }

    #[test]
    fn output_buffer_trim_after_incomplete_merge_does_not_underflow() {
        let mut buffer = OutputBuffer::new();

        buffer.append(b"hello");

        let mut large = vec![b'a'; 30_000];
        large.push(b'\n');
        buffer.append(&large);

        for _ in 0..=MAX_BUFFER_LINES {
            buffer.append(b"x\n");
        }

        let actual_size: usize = buffer.lines.iter().map(|line| line.len()).sum();
        assert_eq!(buffer.total_size, actual_size);
    }

    #[test]
    fn output_buffer_trims_output_without_newlines() {
        let mut buffer = OutputBuffer::new();

        let chunk = vec![b'a'; 4096];
        for _ in 0..(super::MAX_OUTPUT_BUFFER_SIZE / chunk.len()) {
            buffer.append(&chunk);
        }
        buffer.append(b"TAIL");

        assert!(buffer.get_recent(usize::MAX).len() <= super::MAX_OUTPUT_BUFFER_SIZE);
        assert_eq!(buffer.get_recent(4), b"TAIL");
    }
}
