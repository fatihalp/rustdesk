
use hbb_common::{
    anyhow::{anyhow, Context, Result},
    log,
};
use portable_pty::{CommandBuilder, MasterPty, PtySize};
use std::{
    ffi::{c_void, OsStr},
    fs::File,
    io::{Read, Write},
    os::windows::{ffi::OsStrExt, io::FromRawHandle, raw::HANDLE as RawHandle},
    ptr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

use windows::{
    core::{PCWSTR, PWSTR},
    Win32::{
        Foundation::{
            CloseHandle, LocalFree, ERROR_IO_PENDING, ERROR_PIPE_CONNECTED, HANDLE, HLOCAL,
            INVALID_HANDLE_VALUE, WAIT_OBJECT_0,
        },
        Security::{
            Authorization::{
                SetEntriesInAclW, EXPLICIT_ACCESS_W, SET_ACCESS, TRUSTEE_IS_SID, TRUSTEE_IS_USER,
                TRUSTEE_W,
            },
            CreateWellKnownSid, GetLengthSid, GetTokenInformation, InitializeSecurityDescriptor,
            SetSecurityDescriptorDacl, TokenUser, WinLocalSystemSid, ACE_FLAGS, ACL,
            PSECURITY_DESCRIPTOR, PSID, SECURITY_ATTRIBUTES, TOKEN_USER,
        },
        Storage::FileSystem::{
            CreateFileW, FILE_ALL_ACCESS, FILE_FLAGS_AND_ATTRIBUTES, FILE_FLAG_OVERLAPPED,
            FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_READ, FILE_SHARE_WRITE,
            OPEN_EXISTING,
        },
        System::{
            Environment::{CreateEnvironmentBlock, DestroyEnvironmentBlock},
            Pipes::{
                ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE, PIPE_WAIT,
            },
            Threading::{
                CreateEventW, CreateProcessAsUserW, WaitForSingleObject, CREATE_NO_WINDOW,
                CREATE_UNICODE_ENVIRONMENT, PROCESS_CREATION_FLAGS, PROCESS_INFORMATION,
                STARTUPINFOW,
            },
            IO::{GetOverlappedResult, OVERLAPPED},
        },
    },
};

pub use windows::Win32::{
    Foundation::{
        CloseHandle as WinCloseHandle, HANDLE as WinHANDLE, WAIT_OBJECT_0 as WIN_WAIT_OBJECT_0,
    },
    System::Threading::{
        GetExitCodeProcess as WinGetExitCodeProcess, TerminateProcess as WinTerminateProcess,
        WaitForSingleObject as WinWaitForSingleObject,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserToken(pub usize);

impl UserToken {
    pub fn new(handle: usize) -> Self {
        Self(handle)
    }

    pub fn as_raw(&self) -> usize {
        self.0
    }
}

const PIPE_ACCESS_INBOUND: u32 = 0x00000001;
const PIPE_ACCESS_OUTBOUND: u32 = 0x00000002;

const PIPE_BUFFER_SIZE: u32 = 65536;
const PIPE_DEFAULT_TIMEOUT_MS: u32 = 5000;
pub const PIPE_CONNECTION_TIMEOUT_MS: u32 = 10000;

pub const MSG_TYPE_DATA: u8 = 0x01;
pub const MSG_TYPE_RESIZE: u8 = 0x02;

pub const MSG_HEADER_SIZE: usize = 5;

const MAX_PAYLOAD_SIZE: usize = 16 * 1024 * 1024;

pub const HELPER_GRACEFUL_EXIT_TIMEOUT_MS: u64 = 500;

#[derive(Debug)]
pub struct HelperProcessInfo {
    pub handle: HANDLE,
    pub pid: u32,
}

#[derive(Debug)]
pub struct SendableHandle(HANDLE);

impl SendableHandle {
    pub fn new(handle: HANDLE) -> Self {
        Self(handle)
    }

    pub fn as_raw(&self) -> HANDLE {
        self.0
    }
}

unsafe impl Send for SendableHandle {}

pub struct OwnedHandle(HANDLE);

impl OwnedHandle {
    pub fn new(handle: HANDLE) -> Self {
        Self(handle)
    }

    pub fn into_raw(self) -> HANDLE {
        let handle = self.0;
        std::mem::forget(self);
        handle
    }

    pub fn as_raw(&self) -> HANDLE {
        self.0
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if self.0 != INVALID_HANDLE_VALUE && !self.0.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

pub struct HelperProcessGuard {
    handle: HANDLE,
    pid: u32,
    armed: bool,
}

impl HelperProcessGuard {
    pub fn new(handle: HANDLE, pid: u32) -> Self {
        Self {
            handle,
            pid,
            armed: true,
        }
    }

    pub fn as_raw(&self) -> HANDLE {
        self.handle
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn disarm(self) -> HANDLE {
        let handle = self.handle;
        std::mem::forget(self);
        handle
    }
}

impl Drop for HelperProcessGuard {
    fn drop(&mut self) {
        if self.armed && self.handle != INVALID_HANDLE_VALUE && !self.handle.is_invalid() {
            log::warn!(
                "HelperProcessGuard: terminating leaked helper process (PID {})",
                self.pid
            );
            unsafe {
                let _ = WinTerminateProcess(self.handle, 1);
                let _ = CloseHandle(self.handle);
            }
        }
    }
}

pub fn encode_helper_message(msg_type: u8, payload: &[u8]) -> Vec<u8> {
    let mut msg = Vec::with_capacity(MSG_HEADER_SIZE + payload.len());
    msg.push(msg_type);
    msg.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    msg.extend_from_slice(payload);
    msg
}

pub fn encode_resize_message(rows: u16, cols: u16) -> Vec<u8> {
    let mut payload = Vec::with_capacity(4);
    payload.extend_from_slice(&rows.to_le_bytes());
    payload.extend_from_slice(&cols.to_le_bytes());
    encode_helper_message(MSG_TYPE_RESIZE, &payload)
}

pub fn get_default_shell() -> String {
    let pwsh_paths = [
        "pwsh.exe",
        r"C:\Program Files\PowerShell\7\pwsh.exe",
        r"C:\Program Files\PowerShell\6\pwsh.exe",
    ];

    for path in &pwsh_paths {
        if std::path::Path::new(path).exists() {
            log::debug!("Found PowerShell Core: {}", path);
            return path.to_string();
        }
    }

    let powershell_path = r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe";
    if std::path::Path::new(powershell_path).exists() {
        return powershell_path.to_string();
    }

    std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string())
}

fn utf8_shell_args(shell: &str) -> Vec<String> {
    let name = std::path::Path::new(shell)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(shell)
        .to_ascii_lowercase();

    if name == "cmd.exe" || name == "cmd" {
        return vec!["/K".to_string(), "chcp 65001 >NUL".to_string()];
    }

    if name == "pwsh.exe" || name == "pwsh" || name == "powershell.exe" {
        return vec![
            "-NoLogo".to_string(),
            "-NoExit".to_string(),
            "-Command".to_string(),
            "chcp.com 65001 > $null; [Console]::InputEncoding = [System.Text.Encoding]::UTF8; [Console]::OutputEncoding = [System.Text.Encoding]::UTF8".to_string(),
        ];
    }

    Vec::new()
}

pub fn configure_utf8_shell_command(shell: &str, cmd: &mut CommandBuilder) {
    for arg in utf8_shell_args(shell) {
        cmd.arg(arg);
    }
}

pub fn get_user_sid_from_token(user_token: UserToken) -> Result<Vec<u8>> {
    let token_handle = HANDLE(user_token.as_raw() as _);

    let mut return_length = 0u32;
    let _ = unsafe { GetTokenInformation(token_handle, TokenUser, None, 0, &mut return_length) };

    if return_length == 0 {
        return Err(anyhow!(
            "Failed to get token information size: {}",
            std::io::Error::last_os_error()
        ));
    }

    let mut buffer = vec![0u8; return_length as usize];
    unsafe {
        GetTokenInformation(
            token_handle,
            TokenUser,
            Some(buffer.as_mut_ptr() as *mut c_void),
            return_length,
            &mut return_length,
        )
        .map_err(|e| anyhow!("Failed to get token information: {}", e))?;
    }

    let token_user = unsafe { &*(buffer.as_ptr() as *const TOKEN_USER) };
    let sid_ptr = token_user.User.Sid;

    let sid_length = unsafe { GetLengthSid(sid_ptr) };

    if sid_length == 0 {
        return Err(anyhow!("Invalid SID length"));
    }

    let mut sid_buffer = vec![0u8; sid_length as usize];
    unsafe {
        ptr::copy_nonoverlapping(
            sid_ptr.0 as *const u8,
            sid_buffer.as_mut_ptr(),
            sid_length as usize,
        );
    }

    Ok(sid_buffer)
}

pub fn create_restricted_dacl(user_sid: &[u8]) -> Result<*mut c_void> {
    let mut system_sid_buffer = vec![0u8; 64];
    let mut system_sid_size = system_sid_buffer.len() as u32;
    unsafe {
        CreateWellKnownSid(
            WinLocalSystemSid,
            None,
            Some(PSID(system_sid_buffer.as_mut_ptr() as *mut c_void)),
            &mut system_sid_size,
        )
        .map_err(|e| anyhow!("Failed to create SYSTEM SID: {}", e))?;
    }

    let mut explicit_access: [EXPLICIT_ACCESS_W; 2] = unsafe { std::mem::zeroed() };

    explicit_access[0].grfAccessPermissions = FILE_ALL_ACCESS.0;
    explicit_access[0].grfAccessMode = SET_ACCESS;
    explicit_access[0].grfInheritance = ACE_FLAGS(0);
    explicit_access[0].Trustee = TRUSTEE_W {
        pMultipleTrustee: ptr::null_mut(),
        MultipleTrusteeOperation: Default::default(),
        TrusteeForm: TRUSTEE_IS_SID,
        TrusteeType: TRUSTEE_IS_USER,
        ptstrName: PWSTR::from_raw(system_sid_buffer.as_ptr() as *mut u16),
    };

    explicit_access[1].grfAccessPermissions = FILE_ALL_ACCESS.0;
    explicit_access[1].grfAccessMode = SET_ACCESS;
    explicit_access[1].grfInheritance = ACE_FLAGS(0);
    explicit_access[1].Trustee = TRUSTEE_W {
        pMultipleTrustee: ptr::null_mut(),
        MultipleTrusteeOperation: Default::default(),
        TrusteeForm: TRUSTEE_IS_SID,
        TrusteeType: TRUSTEE_IS_USER,
        ptstrName: PWSTR::from_raw(user_sid.as_ptr() as *mut u16),
    };

    let mut new_acl: *mut ACL = ptr::null_mut();
    let result = unsafe {
        SetEntriesInAclW(
            Some(&explicit_access),
            None,
            &mut new_acl,
        )
    };

    if result.0 != 0 {
        return Err(anyhow!(
            "SetEntriesInAclW failed with error code: {}",
            result.0
        ));
    }

    if new_acl.is_null() {
        return Err(anyhow!("SetEntriesInAclW returned null ACL"));
    }

    Ok(new_acl as *mut c_void)
}

pub fn create_named_pipe_server(
    pipe_name: &str,
    for_input: bool,
    user_token: UserToken,
) -> Result<HANDLE> {
    const SD_BUFFER_SIZE: usize = 64;
    const _: () = assert!(
        SD_BUFFER_SIZE >= 40,
        "SD_BUFFER_SIZE must be at least 40 bytes for SECURITY_DESCRIPTOR"
    );

    let mut sd_buffer = [0u8; SD_BUFFER_SIZE];
    let sd_ptr = PSECURITY_DESCRIPTOR(sd_buffer.as_mut_ptr() as *mut c_void);

    unsafe {
        InitializeSecurityDescriptor(sd_ptr, 1)
            .map_err(|e| anyhow!("Failed to initialize security descriptor: {}", e))?;
    }

    let user_sid = get_user_sid_from_token(user_token)
        .context("Failed to get user SID from token for pipe DACL")?;
    let acl_ptr =
        create_restricted_dacl(&user_sid).context("Failed to create restricted DACL for pipe")?;

    log::debug!("Created restricted DACL for pipe: {}", pipe_name);

    unsafe {
        SetSecurityDescriptorDacl(sd_ptr, true, Some(acl_ptr as *const _ as *const _), false)
            .map_err(|e| {
                let _ = LocalFree(Some(HLOCAL(acl_ptr)));
                anyhow!("Failed to set restricted DACL: {}", e)
            })?;
    }

    let sa = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: sd_buffer.as_mut_ptr() as *mut c_void,
        bInheritHandle: false.into(),
    };

    let wide_name: Vec<u16> = OsStr::new(pipe_name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let access_mode = if for_input {
        FILE_FLAGS_AND_ATTRIBUTES(PIPE_ACCESS_INBOUND | FILE_FLAG_OVERLAPPED.0)
    } else {
        FILE_FLAGS_AND_ATTRIBUTES(PIPE_ACCESS_OUTBOUND | FILE_FLAG_OVERLAPPED.0)
    };

    log::debug!(
        "Creating named pipe: {} (for_input={}, restricted_dacl=true)",
        pipe_name,
        for_input
    );

    let handle = unsafe {
        CreateNamedPipeW(
            PCWSTR::from_raw(wide_name.as_ptr()),
            access_mode,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
            1,
            PIPE_BUFFER_SIZE,
            PIPE_BUFFER_SIZE,
            PIPE_DEFAULT_TIMEOUT_MS,
            Some(&sa),
        )
    };

    unsafe {
        let _ = LocalFree(Some(HLOCAL(acl_ptr)));
    }

    if handle == INVALID_HANDLE_VALUE {
        return Err(anyhow!(
            "Failed to create named pipe {}: {}",
            pipe_name,
            std::io::Error::last_os_error()
        ));
    }

    log::debug!("Named pipe created: {}", pipe_name);
    Ok(handle)
}

pub fn wait_for_pipe_connection(
    pipe_handle: OwnedHandle,
    pipe_name: &str,
    timeout_ms: u32,
) -> Result<File> {
    log::debug!("Waiting for pipe connection: {}", pipe_name);

    let event = unsafe { CreateEventW(None, true, false, PCWSTR::null()) }
        .map_err(|e| anyhow!("Failed to create event for pipe connection: {}", e))?;
    let event_handle = OwnedHandle::new(event);

    let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
    overlapped.hEvent = event_handle.as_raw();

    let result = unsafe { ConnectNamedPipe(pipe_handle.as_raw(), Some(&mut overlapped)) };
    if result.is_err() {
        let err = std::io::Error::last_os_error();
        let err_code = err.raw_os_error().unwrap_or(0);

        if err_code == ERROR_PIPE_CONNECTED.0 as i32 {
            log::debug!("Pipe already connected: {}", pipe_name);
            return Ok(unsafe { File::from_raw_handle(pipe_handle.into_raw().0 as RawHandle) });
        }

        if err_code == ERROR_IO_PENDING.0 as i32 {
            log::debug!("Pipe connection pending, waiting with timeout...");
            let wait_result = unsafe { WaitForSingleObject(event_handle.as_raw(), timeout_ms) };

            if wait_result != WAIT_OBJECT_0 {
                log::error!("Timeout waiting for pipe connection: {}", pipe_name);
                return Err(anyhow!(
                    "Timeout waiting for pipe connection: {}",
                    pipe_name
                ));
            }

            let mut bytes_transferred = 0u32;
            let overlapped_result = unsafe {
                GetOverlappedResult(
                    pipe_handle.as_raw(),
                    &overlapped,
                    &mut bytes_transferred,
                    false,
                )
            };
            if overlapped_result.is_err() {
                let err = std::io::Error::last_os_error();
                log::error!("Failed to complete pipe connection {}: {}", pipe_name, err);
                return Err(anyhow!(
                    "Failed to complete pipe connection {}: {}",
                    pipe_name,
                    err
                ));
            }

            log::debug!("Pipe connected: {}", pipe_name);
        } else {
            log::error!("Failed to connect named pipe {}: {}", pipe_name, err);
            return Err(anyhow!(
                "Failed to connect named pipe {}: {}",
                pipe_name,
                err
            ));
        }
    } else {
        log::debug!("Pipe connected immediately: {}", pipe_name);
    }

    Ok(unsafe { File::from_raw_handle(pipe_handle.into_raw().0 as RawHandle) })
}


struct EnvironmentBlockGuard {
    ptr: *mut c_void,
}

impl Drop for EnvironmentBlockGuard {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                let _ = DestroyEnvironmentBlock(self.ptr);
            }
        }
    }
}

pub fn launch_terminal_helper_with_token(
    user_token: UserToken,
    input_pipe_name: &str,
    output_pipe_name: &str,
    terminal_id: i32,
    rows: u16,
    cols: u16,
) -> Result<HelperProcessInfo> {
    let exe_path =
        std::env::current_exe().map_err(|e| anyhow!("Failed to get current exe path: {}", e))?;

    let cmd_args = format!(
        "--terminal-helper {} {} {} {} {}",
        input_pipe_name, output_pipe_name, rows, cols, terminal_id
    );

    log::debug!("Launching terminal helper for terminal {}", terminal_id);

    let exe_path_wide: Vec<u16> = OsStr::new(exe_path.as_os_str())
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let cmd_line = format!("\"{}\" {}", exe_path.display(), cmd_args);
    let mut cmd_wide: Vec<u16> = OsStr::new(&cmd_line)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut si: STARTUPINFOW = unsafe { std::mem::zeroed() };
    si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;

    let mut pi: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };

    let mut environment: *mut c_void = ptr::null_mut();
    let env_ok = unsafe {
        CreateEnvironmentBlock(
            &mut environment,
            Some(HANDLE(user_token.as_raw() as _)),
            true,
        )
    }
    .is_ok();

    let _env_guard = if env_ok && !environment.is_null() {
        Some(EnvironmentBlockGuard { ptr: environment })
    } else {
        if !env_ok {
            log::warn!("Failed to create environment block, using default");
        }
        None
    };

    let creation_flags = CREATE_NO_WINDOW
        | if env_ok {
            CREATE_UNICODE_ENVIRONMENT
        } else {
            PROCESS_CREATION_FLAGS(0)
        };

    let result = unsafe {
        CreateProcessAsUserW(
            Some(HANDLE(user_token.as_raw() as _)),
            PCWSTR::from_raw(exe_path_wide.as_ptr()),
            Some(PWSTR::from_raw(cmd_wide.as_mut_ptr())),
            None,
            None,
            false,
            creation_flags,
            if env_ok { Some(environment) } else { None },
            PCWSTR::null(),
            &si,
            &mut pi,
        )
    };


    if let Err(e) = result {
        log::error!("CreateProcessAsUserW failed: {}", e);
        return Err(anyhow!("Failed to launch terminal helper: {}", e));
    }

    unsafe {
        let _ = CloseHandle(pi.hThread);
    }

    log::info!("Terminal helper launched with PID {}", pi.dwProcessId);
    Ok(HelperProcessInfo {
        handle: pi.hProcess,
        pid: pi.dwProcessId,
    })
}

pub fn is_helper_process_running(handle: HANDLE) -> bool {
    let wait_result = unsafe { WaitForSingleObject(handle, 0) };
    wait_result != WAIT_OBJECT_0
}

pub fn run_terminal_helper(args: &[String]) -> Result<()> {
    if args.len() < 5 {
        return Err(anyhow!(
            "Usage: --terminal-helper <input_pipe> <output_pipe> <rows> <cols> <terminal_id>"
        ));
    }

    let input_pipe_name = &args[0];
    let output_pipe_name = &args[1];
    let rows: u16 = args[2]
        .parse()
        .map_err(|e| anyhow!("Failed to parse rows '{}': {}", args[2], e))?;
    let cols: u16 = args[3]
        .parse()
        .map_err(|e| anyhow!("Failed to parse cols '{}': {}", args[3], e))?;
    let terminal_id: i32 = args[4]
        .parse()
        .map_err(|e| anyhow!("Failed to parse terminal_id '{}': {}", args[4], e))?;

    log::debug!(
        "Terminal helper starting: terminal_id={}, size={}x{}",
        terminal_id,
        cols,
        rows
    );

    let mut input_pipe = open_pipe(input_pipe_name, true)?;
    let mut output_pipe = open_pipe(output_pipe_name, false)?;

    let pty_size = PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    };

    let pty_system = portable_pty::native_pty_system();
    let pty_pair = pty_system.openpty(pty_size).context("Failed to open PTY")?;

    let shell = get_default_shell();
    log::debug!("Using shell: {}", shell);

    let mut cmd = CommandBuilder::new(&shell);
    configure_utf8_shell_command(&shell, &mut cmd);
    let mut child = pty_pair
        .slave
        .spawn_command(cmd)
        .context("Failed to spawn shell")?;

    drop(pty_pair.slave);

    let pid = child.process_id().unwrap_or(0);
    log::debug!("Shell started with PID: {}", pid);

    let mut pty_writer = pty_pair
        .master
        .take_writer()
        .context("Failed to get PTY writer")?;

    let mut pty_reader = pty_pair
        .master
        .try_clone_reader()
        .context("Failed to get PTY reader")?;

    let pty_master: Arc<Mutex<Box<dyn MasterPty + Send>>> = Arc::new(Mutex::new(pty_pair.master));

    let exiting = Arc::new(AtomicBool::new(false));

    let exiting_clone = exiting.clone();
    let pty_master_clone = pty_master.clone();
    let input_thread = thread::spawn(move || {
        let mut input_pipe = input_pipe;
        let mut header_buf = [0u8; MSG_HEADER_SIZE];
        let mut payload_buf = vec![0u8; 4096];

        loop {
            if exiting_clone.load(Ordering::SeqCst) {
                break;
            }

            match read_exact_or_eof(&mut input_pipe, &mut header_buf) {
                Ok(false) => {
                    log::debug!("Input pipe EOF");
                    break;
                }
                Ok(true) => {}
                Err(e) => {
                    log::error!("Input pipe header read error: {}", e);
                    break;
                }
            }

            let msg_type = header_buf[0];
            let payload_len =
                u32::from_le_bytes([header_buf[1], header_buf[2], header_buf[3], header_buf[4]])
                    as usize;

            if payload_len > MAX_PAYLOAD_SIZE {
                log::error!(
                    "Payload too large: {} bytes (max {})",
                    payload_len,
                    MAX_PAYLOAD_SIZE
                );
                break;
            }

            if payload_buf.len() < payload_len {
                payload_buf.resize(payload_len, 0);
            }

            if payload_len > 0 {
                match read_exact_or_eof(&mut input_pipe, &mut payload_buf[..payload_len]) {
                    Ok(false) => {
                        log::debug!("Input pipe EOF during payload read");
                        break;
                    }
                    Ok(true) => {}
                    Err(e) => {
                        log::error!("Input pipe payload read error: {}", e);
                        break;
                    }
                }
            }

            match msg_type {
                MSG_TYPE_DATA => {
                    if let Err(e) = pty_writer.write_all(&payload_buf[..payload_len]) {
                        log::error!("PTY write error: {}", e);
                        break;
                    }
                    if let Err(e) = pty_writer.flush() {
                        log::error!("PTY flush error: {}", e);
                        break;
                    }
                }
                MSG_TYPE_RESIZE => {
                    if payload_len >= 4 {
                        let rows = u16::from_le_bytes([payload_buf[0], payload_buf[1]]);
                        let cols = u16::from_le_bytes([payload_buf[2], payload_buf[3]]);
                        log::debug!("Resize: {}x{}", cols, rows);
                        if let Ok(master) = pty_master_clone.lock() {
                            let _ = master.resize(PtySize {
                                rows,
                                cols,
                                pixel_width: 0,
                                pixel_height: 0,
                            });
                        }
                    }
                }
                _ => {
                    log::error!("Unknown message type: {}, terminating", msg_type);
                    break;
                }
            }
        }
        log::debug!("Input thread exiting");
    });

    let exiting_clone = exiting.clone();
    let output_thread = thread::spawn(move || {
        let mut output_pipe = output_pipe;
        let mut buf = vec![0u8; 4096];
        loop {
            if exiting_clone.load(Ordering::SeqCst) {
                break;
            }
            match pty_reader.read(&mut buf) {
                Ok(0) => {
                    log::debug!("PTY EOF");
                    break;
                }
                Ok(n) => {
                    if let Err(e) = output_pipe.write_all(&buf[..n]) {
                        log::error!("Output pipe write error: {}", e);
                        break;
                    }
                    if let Err(e) = output_pipe.flush() {
                        log::error!("Output pipe flush error: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    if e.kind() != std::io::ErrorKind::WouldBlock {
                        log::error!("PTY read error: {}", e);
                        break;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
            }
        }
        log::debug!("Output thread exiting");
    });

    let exit_status = child.wait();
    log::info!("Shell exited: {:?}", exit_status);

    exiting.store(true, Ordering::SeqCst);

    let _ = input_thread.join();
    let _ = output_thread.join();

    drop(pty_master);

    log::info!("Terminal helper exiting");
    Ok(())
}

fn read_exact_or_eof<R: Read>(reader: &mut R, buf: &mut [u8]) -> std::io::Result<bool> {
    let mut pos = 0;
    while pos < buf.len() {
        match reader.read(&mut buf[pos..]) {
            Ok(0) => return Ok(false),
            Ok(n) => pos += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(true)
}

fn open_pipe(pipe_name: &str, for_read: bool) -> Result<File> {
    let wide_name: Vec<u16> = OsStr::new(pipe_name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let access = if for_read {
        FILE_GENERIC_READ.0
    } else {
        FILE_GENERIC_WRITE.0
    };

    let handle = unsafe {
        CreateFileW(
            PCWSTR::from_raw(wide_name.as_ptr()),
            access,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
    };

    match handle {
        Ok(h) => Ok(unsafe { File::from_raw_handle(h.0 as _) }),
        Err(e) => Err(anyhow!(
            "Failed to open {} pipe '{}': {}",
            if for_read { "input" } else { "output" },
            pipe_name,
            e
        )),
    }
}
