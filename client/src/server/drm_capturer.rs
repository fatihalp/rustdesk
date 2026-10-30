
use crate::ipc::{connect_drm, Data, DrmDisplayInfo};
use hbb_common::{anyhow::anyhow, bail, log, tokio, ResultType};
use base::message_proto::DisplayInfo;
use scrap::drm_render::RenderConverter;
use scrap::drmtap_dl::drmtap_dmabuf_desc;
use scrap::{Frame, Pixfmt, PixelBuffer, TraitCapturer};
use std::collections::BTreeMap;
use std::io;
use std::os::fd::{AsRawFd, RawFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

const HANDSHAKE_TIMEOUT_MS: u64 = 3000;
const DRM_CONNECT_TIMEOUT_MS: u64 = 1000;
const DISPLAY_LIST_TIMEOUT_MS: u64 = HANDSHAKE_TIMEOUT_MS + 4000;
const HANDSHAKE_WAIT_MS: u64 = DRM_CONNECT_TIMEOUT_MS + DISPLAY_LIST_TIMEOUT_MS * 2 + 500;
const BODY_READ_TIMEOUT: Duration = Duration::from_secs(5);
const REPEAT_EVERY: Duration = Duration::from_secs(1);
const STILL_AFTER: Duration = Duration::from_millis(100);
const STILL_FPS: u8 = 30;

struct FrameSlot {
    latest: Option<(usize, usize, Pixfmt, Option<u32>, Vec<u8>)>,
    free: [Option<Vec<u8>>; 2],
    ended: Option<String>,
}

impl FrameSlot {
    fn publish(
        &mut self,
        w: usize,
        h: usize,
        fmt: Pixfmt,
        plane_rotation: Option<u32>,
        buf: Vec<u8>,
    ) {
        if let Some((.., old)) = self.latest.take() {
            self.recycle(old);
        }
        self.latest = Some((w, h, fmt, plane_rotation, buf));
    }

    fn recycle(&mut self, buf: Vec<u8>) {
        if let Some(slot) = self.free.iter_mut().find(|s| s.is_none()) {
            *slot = Some(buf);
        }
    }

    fn take_free(&mut self) -> Option<Vec<u8>> {
        self.free.iter_mut().find_map(|s| s.take())
    }
}

const TRANSFORM_PENDING: i32 = i32::MIN;

struct Shared {
    slot: Mutex<FrameSlot>,
    cv: Condvar,
    cursor_transform: std::sync::atomic::AtomicI32,
    cal_context: Mutex<Option<CalContext>>,
    demand_fps: std::sync::atomic::AtomicU8,
    pace: Mutex<EncoderPace>,
}

#[derive(Default)]
struct EncoderPace {
    last_call: Option<Instant>,
    last_wait: Duration,
    busy: Option<Duration>,
    spf: Duration,
}

impl EncoderPace {
    fn demand(&mut self, now: Instant, returned_at: Instant, spf: Duration) -> u8 {
        if spf < self.spf {
            self.busy = None;
            self.last_call = None;
        }
        self.spf = spf;
        if let Some(prev) = self.last_call.filter(|&prev| returned_at > prev) {
            let sample = now
                .saturating_duration_since(prev)
                .saturating_sub(self.last_wait)
                .min(Duration::from_secs(1));
            self.busy = Some(self.busy.map_or(sample, |avg| (avg * 7 + sample) / 8));
        }
        self.last_call = Some(now);
        self.last_wait = Duration::ZERO;
        let demand = demand_fps_of(spf, self.busy);
        if now.saturating_duration_since(returned_at) >= STILL_AFTER {
            demand.min(STILL_FPS)
        } else {
            demand
        }
    }
}

struct WaitStamp<'a>(&'a Mutex<EncoderPace>, Instant);

impl Drop for WaitStamp<'_> {
    fn drop(&mut self) {
        if let Ok(mut pace) = self.0.lock() {
            pace.last_wait = self.1.elapsed();
        }
    }
}

fn demand_fps_of(spf: Duration, busy: Option<Duration>) -> u8 {
    let max = f64::from(super::video_qos::MAX_FPS);
    let asked = if spf.is_zero() {
        max
    } else {
        (1.0 / spf.as_secs_f64()).round().clamp(1.0, max)
    };
    let achieved = match busy {
        Some(o) if o > spf => (1.0 / o.as_secs_f64()).ceil().clamp(1.0, max),
        _ => max,
    };
    asked.min(achieved) as u8
}

pub struct IpcDrmCapturer {
    shared: Arc<Shared>,
    stop: Arc<AtomicBool>,
    display: i32,
    connector: Option<String>,
    session_size: Option<(usize, usize)>,
    transform: i32,
    snapshot_gen: u64,
    cur: Vec<u8>,
    cur_w: usize,
    cur_h: usize,
    cur_fmt: Pixfmt,
    got_frame: bool,
    spare: Vec<u8>,
    returned_at: Instant,
}

fn connector_key(d: &DrmDisplayInfo) -> String {
    format!("{}:{}", d.device, d.name)
}

fn delivered_transform(d: &DrmDisplayInfo, wl_transform: i32) -> i32 {
    if d.plane_transposed {
        90
    } else {
        wl_transform
    }
}

fn rotated_dims(transform: i32, w: usize, h: usize) -> (usize, usize) {
    if transform == 90 || transform == 270 {
        (h, w)
    } else {
        (w, h)
    }
}

fn unrotate_hotspot(transform: i32, w: i32, h: i32, hotx: i32, hoty: i32) -> (i32, i32) {
    match transform {
        90 => (h - 1 - hoty, hotx),
        180 => (w - 1 - hotx, h - 1 - hoty),
        270 => (hoty, w - 1 - hotx),
        _ => (hotx, hoty),
    }
}

fn unrotate_bgra(src: &[u8], w: usize, h: usize, transform: i32, dst: &mut Vec<u8>) {
    const PX: usize = 4;
    let stride = if h > 0 { src.len() / h } else { 0 };
    let (dw, dh) = rotated_dims(transform, w, h);
    dst.resize(
        dw.checked_mul(dh).and_then(|p| p.checked_mul(PX)).unwrap_or(0),
        0,
    );
    if dst.is_empty() || stride < w * PX {
        log::error!("unrotate: rejected geometry {w}x{h} stride {stride}; frame left blank");
        return;
    }
    let mode = match transform {
        90 => scrap::RotationMode::kRotate90,
        180 => scrap::RotationMode::kRotate180,
        270 => scrap::RotationMode::kRotate270,
        _ => scrap::RotationMode::kRotate0,
    };
    unsafe {
        scrap::ARGBRotate(
            src.as_ptr(),
            stride as i32,
            dst.as_mut_ptr(),
            (dw * PX) as i32,
            w as i32,
            h as i32,
            mode,
        );
    }
}

fn transform_and_origin(
    drm: &[DrmDisplayInfo],
    wire_idx: usize,
    wl: &scrap::wayland::display::Displays,
) -> (i32, Option<(i32, i32)>) {
    if wl.displays.is_empty() || (wl.displays.len() == 1 && drm.len() > 1) {
        if wl.displays.is_empty() && !drm.is_empty() {
            UNROTATED_SNAPSHOT_PENDING.store(true, Ordering::Release);
            log::warn!(
                "drm: no wayland snapshot at capturer build for display {:?}; assuming unrotated",
                drm.get(wire_idx).map(|d| d.name.as_str()).unwrap_or("?")
            );
        }
        return (0, None);
    }
    let assignment = assign_wayland_outputs(drm, &wl.displays);
    let transform = identity_matches(drm, &wl.displays)
        .get(wire_idx)
        .copied()
        .flatten()
        .map(|j| wl.displays[j].transform)
        .unwrap_or(0);
    let origin = augment_with_wayland_geometry_from(drm, wl, &assignment)
        .get(wire_idx)
        .map(|di| (di.x, di.y));
    (transform, origin)
}

const PLANE_ROTATE_0: u32 = 0x1;

fn frame_transform(wl_transform: i32, plane_rotation: Option<u32>) -> i32 {
    match plane_rotation {
        Some(mask) if mask != PLANE_ROTATE_0 => 0,
        Some(_) => wl_transform,
        None => {
            if wl_transform == 90 || wl_transform == 270 {
                wl_transform
            } else {
                0
            }
        }
    }
}

fn display_info_of(display: i32) -> Option<DrmDisplayInfo> {
    match &*DRM_STATE.lock().unwrap() {
        ProbeState::Available(_, list) => list.get(display.max(0) as usize).cloned(),
        _ => None,
    }
}

#[derive(Clone, Copy)]
struct DisplayHealth {
    zero_frame_streak: u32,
    since: Instant,
    demotes: u32,
    last_build: Option<Instant>,
    rapid_builds: u32,
    prefer_cpu: bool,
    fallback_rejected: bool,
}

impl DisplayHealth {
    fn new() -> Self {
        Self {
            zero_frame_streak: 0,
            since: Instant::now(),
            demotes: 0,
            last_build: None,
            rapid_builds: 0,
            prefer_cpu: false,
            fallback_rejected: false,
        }
    }

    fn demoted(&self) -> bool {
        self.zero_frame_streak >= DRM_GRAB_MAX_FAILURES
            && self.since.elapsed() < demote_cooldown(self.demotes)
    }
}

static DRM_DISPLAY_HEALTH: Mutex<BTreeMap<String, DisplayHealth>> = Mutex::new(BTreeMap::new());
const DRM_GRAB_MAX_FAILURES: u32 = 4;
const DEMOTE_COOLDOWN: Duration = Duration::from_secs(30);
const DEMOTE_BACKOFF_MAX_SHIFT: u32 = 4;
const RAPID_REBUILD_WINDOW: Duration = Duration::from_secs(3);
const RAPID_REBUILD_MAX: u32 = 6;

fn demote_cooldown(demotes: u32) -> Duration {
    DEMOTE_COOLDOWN * (1u32 << demotes.saturating_sub(1).min(DEMOTE_BACKOFF_MAX_SHIFT))
}

#[derive(Debug, PartialEq, Eq)]
enum RefreshOutcome {
    Publish,
    Unavailable,
    Restamp,
    GiveUp,
}

fn refresh_outcome(probe: Option<usize>, failures: u32) -> RefreshOutcome {
    match probe {
        Some(0) => RefreshOutcome::Unavailable,
        Some(_) => RefreshOutcome::Publish,
        None if failures >= DRM_REFRESH_MAX_FAILURES => RefreshOutcome::GiveUp,
        None => RefreshOutcome::Restamp,
    }
}

fn drm_prefer_cpu(key: &Option<String>) -> bool {
    key.as_ref().is_some_and(|k| {
        DRM_DISPLAY_HEALTH
            .lock()
            .unwrap()
            .get(k)
            .is_some_and(|h| h.prefer_cpu)
    })
}

fn drm_set_prefer_cpu(key: &Option<String>) {
    if let Some(k) = key {
        DRM_DISPLAY_HEALTH
            .lock()
            .unwrap()
            .entry(k.clone())
            .or_insert_with(DisplayHealth::new)
            .prefer_cpu = true;
    }
}

fn render_node_count() -> usize {
    std::fs::read_dir("/dev/dri").map_or(0, |entries| {
        entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .and_then(|n| n.strip_prefix("renderD"))
                    .and_then(|minor| minor.parse::<u32>().ok())
                    .is_some()
            })
            .count()
    })
}

static UINPUT_REFRESH_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static UNROTATED_SNAPSHOT_PENDING: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub(super) fn take_unrotated_snapshot_pending() -> bool {
    UNROTATED_SNAPSHOT_PENDING.swap(false, std::sync::atomic::Ordering::AcqRel)
}
static UINPUT_REFRESH_BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

impl IpcDrmCapturer {
    pub fn new(
        display: i32,
        expected: Option<DrmDisplayInfo>,
    ) -> ResultType<(IpcDrmCapturer, Vec<DrmDisplayInfo>, usize, Option<(i32, i32)>)> {
        let shared = Arc::new(Shared {
            slot: Mutex::new(FrameSlot {
                latest: None,
                free: [None, None],
                ended: None,
            }),
            cv: Condvar::new(),
            cursor_transform: std::sync::atomic::AtomicI32::new(TRANSFORM_PENDING),
            cal_context: Mutex::new(None),
            demand_fps: std::sync::atomic::AtomicU8::new(0),
            pace: Mutex::new(EncoderPace::default()),
        });
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = std::sync::mpsc::channel::<ResultType<(Vec<DrmDisplayInfo>, usize)>>();
        {
            let shared = shared.clone();
            let stop = stop.clone();
            std::thread::Builder::new()
                .name("drm-recv".into())
                .spawn(move || recv_thread(display, expected, shared, stop, tx))
                .map_err(|err| anyhow!("could not spawn the drm receive thread: {err}"))?;
        }
        let (displays, wire_idx) = match rx.recv_timeout(Duration::from_millis(HANDSHAKE_WAIT_MS)) {
            Ok(res) => res?,
            Err(_) => {
                stop.store(true, Ordering::SeqCst);
                bail!("drm capture handshake timed out");
            }
        };
        let snapshot_gen = scrap::wayland::display::wayland_snapshot_generation();
        let wl = scrap::wayland::display::get_displays();
        let (wl_transform, origin) = transform_and_origin(&displays, wire_idx, &wl);
        let cal = calibration_context(&displays, wire_idx, &wl, snapshot_gen);
        log::info!(
            "drm: cursor calibration for display {display} ({}): {cal:?}",
            displays.get(wire_idx).map(|d| d.name.as_str()).unwrap_or("?")
        );
        *shared.cal_context.lock().unwrap() = cal.ok();
        super::display_service::note_capturer_layout(&wl.displays, snapshot_gen);
        shared
            .cursor_transform
            .store(wl_transform, std::sync::atomic::Ordering::Release);
        Ok((
            IpcDrmCapturer {
                shared,
                stop,
                display,
                connector: displays.get(wire_idx).map(connector_key),
                session_size: displays.get(wire_idx).map(|d| {
                    rotated_dims(
                        delivered_transform(d, wl_transform),
                        d.width as usize,
                        d.height as usize,
                    )
                }),
                transform: wl_transform,
                snapshot_gen,
                cur: Vec::new(),
                cur_w: 0,
                cur_h: 0,
                cur_fmt: Pixfmt::BGRA,
                got_frame: false,
                spare: Vec::new(),
                returned_at: Instant::now(),
            },
            displays,
            wire_idx,
            origin,
        ))
    }

    fn note_session_without_frame(&self) {
        let Some(key) = self.connector.clone() else {
            log::debug!(
                "drm: display {} produced no frame but has no connector identity; \
                 not counting it against any display",
                self.display
            );
            return;
        };
        let mut map = DRM_DISPLAY_HEALTH.lock().unwrap();
        let h = map.entry(key).or_insert_with(DisplayHealth::new);
        h.zero_frame_streak += 1;
        h.since = Instant::now();
        if h.zero_frame_streak == DRM_GRAB_MAX_FAILURES {
            h.demotes += 1;
            log::warn!(
                "drm: display {} produced no frame in {} sessions; using PipeWire for it, \
                 retrying DRM in {:?} (demotion {})",
                self.display,
                h.zero_frame_streak,
                demote_cooldown(h.demotes),
                h.demotes
            );
        }
    }
}

impl Drop for IpcDrmCapturer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

impl TraitCapturer for IpcDrmCapturer {
    fn frame<'a>(&'a mut self, timeout: Duration) -> io::Result<Frame<'a>> {
        let demand =
            self.shared
                .pace
                .lock()
                .unwrap()
                .demand(Instant::now(), self.returned_at, timeout);
        self.shared.demand_fps.store(demand, Ordering::Relaxed);
        let deadline = Instant::now() + timeout;
        let repeat = {
            let waited = WaitStamp(&self.shared.pace, Instant::now());
            let mut slot = self.shared.slot.lock().unwrap();
            loop {
                if slot.latest.is_some() || slot.ended.is_some() {
                    break;
                }
                let now = Instant::now();
                if now >= deadline {
                    return Err(io::ErrorKind::WouldBlock.into());
                }
                let (guard, _timed_out) =
                    self.shared.cv.wait_timeout(slot, deadline - now).unwrap();
                slot = guard;
            }
            if let Some((w, h, fmt, plane_rotation, buf)) = slot.latest.take() {
                drop(slot);
                drop(waited);
                if scrap::wayland::display::wayland_snapshot_generation() != self.snapshot_gen {
                    self.shared.slot.lock().unwrap().recycle(buf);
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!("drm: display {} layout changed; rebuilding", self.display),
                    ));
                }
                let t = frame_transform(self.transform, plane_rotation);
                let (fw, fh) = rotated_dims(t, w, h);
                if self.session_size.is_some_and(|(sw, sh)| (fw, fh) != (sw, sh)) {
                    self.shared.slot.lock().unwrap().recycle(buf);
                    if !self.got_frame {
                        self.note_session_without_frame();
                    }
                    let (sw, sh) = self.session_size.unwrap_or_default();
                    let what = if self.got_frame {
                        "changed geometry mid-session"
                    } else {
                        "never matched its advertised geometry"
                    };
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!(
                            "drm: display {} {what} ({sw}x{sh} -> {fw}x{fh}); rebuilding",
                            self.display
                        ),
                    ));
                }
                let repeat = if t == 0 {
                    let repeat = fmt == self.cur_fmt && buf == self.cur;
                    let previous = std::mem::replace(&mut self.cur, buf);
                    self.shared.slot.lock().unwrap().recycle(previous);
                    repeat
                } else if !matches!(fmt, Pixfmt::BGRA | Pixfmt::RGBA) {
                    self.shared.slot.lock().unwrap().recycle(buf);
                    if !self.got_frame {
                        self.note_session_without_frame();
                    }
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!(
                            "drm: display {} delivered {fmt:?} on a rotated output; rebuilding",
                            self.display
                        ),
                    ));
                } else {
                    unrotate_bgra(&buf, w, h, t, &mut self.spare);
                    let repeat = fmt == self.cur_fmt && self.spare == self.cur;
                    std::mem::swap(&mut self.cur, &mut self.spare);
                    self.shared.slot.lock().unwrap().recycle(buf);
                    repeat
                };
                self.cur_w = fw;
                self.cur_h = fh;
                self.cur_fmt = fmt;
                if !self.got_frame {
                    self.got_frame = true;
                    if let Some(key) = &self.connector {
                        if let Some(h) = DRM_DISPLAY_HEALTH.lock().unwrap().get_mut(key) {
                            h.zero_frame_streak = 0;
                            h.demotes = 0;
                            h.since = Instant::now();
                            h.fallback_rejected = false;
                        }
                    }
                }
                repeat
            } else {
                let err = slot
                    .ended
                    .clone()
                    .unwrap_or_else(|| "drm stream ended".to_owned());
                if !self.got_frame {
                    self.note_session_without_frame();
                }
                return Err(io::Error::new(io::ErrorKind::Other, err));
            }
        };
        if repeat && self.returned_at.elapsed() < REPEAT_EVERY {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        self.returned_at = Instant::now();
        Ok(Frame::PixelBuffer(PixelBuffer::new(
            &self.cur,
            self.cur_fmt,
            self.cur_w,
            self.cur_h,
        )))
    }
}

#[tokio::main(flavor = "current_thread")]
async fn recv_thread(
    display: i32,
    expected: Option<DrmDisplayInfo>,
    shared: Arc<Shared>,
    stop: Arc<AtomicBool>,
    tx: std::sync::mpsc::Sender<ResultType<(Vec<DrmDisplayInfo>, usize)>>,
) {
    let cursor_epoch = next_cursor_epoch();
    let mut conn = match connect_drm(DRM_CONNECT_TIMEOUT_MS).await {
        Ok(c) => c,
        Err(err) => {
            let _ = tx.send(Err(err));
            return;
        }
    };
    if let Err(err) = conn.send_msg(&Data::DrmHello { wake: true }, None).await {
        let _ = tx.send(Err(err));
        return;
    }
    let displays = match conn.recv_msg_timeout2(DISPLAY_LIST_TIMEOUT_MS).await {
        Some(Ok((Data::DrmDisplayList(v), _fd))) => v,
        Some(Ok((other, _fd))) => {
            let _ = tx.send(Err(anyhow!("expected DrmDisplayList, got {:?}", other)));
            return;
        }
        Some(Err(err)) => {
            let _ = tx.send(Err(err));
            return;
        }
        None => {
            let _ = tx.send(Err(anyhow!("timed out waiting for DrmDisplayList")));
            return;
        }
    };
    let wire_idx = match &expected {
        Some(e) => {
            match displays
                .iter()
                .position(|d| d.device == e.device && d.name == e.name)
            {
                Some(i) => i,
                None => {
                    let _ = tx.send(Err(anyhow!(
                        "display {display} ({}) is no longer in the service's list; \
                         the video service will rebuild against the fresh topology",
                        e.name
                    )));
                    return;
                }
            }
        }
        None => {
            let _ = tx.send(Err(anyhow!(
                "display {display} is not in the advertised list; not guessing a monitor for it"
            )));
            return;
        }
    };
    let bound_to = displays
        .get(wire_idx)
        .map(|d| (d.device.clone(), d.crtc_id));
    let our_key = displays.get(wire_idx).map(connector_key);
    let render_node = displays
        .get(wire_idx)
        .or_else(|| displays.first())
        .map(|d| d.render_node.clone())
        .unwrap_or_default();
    let ambiguous_gpu = render_node.is_empty() && render_node_count() > 1;
    let force_cpu = drm_prefer_cpu(&our_key) || ambiguous_gpu;
    let mut converter = if force_cpu {
        None
    } else {
        RenderConverter::open_render(Some(render_node.as_str()))
    };
    let need_cpu = converter.is_none();
    if need_cpu {
        log::info!(
            "drm: requesting the CPU-converted frame path for display {display} ({})",
            if ambiguous_gpu {
                "the service did not name the exporting GPU and this host has several render nodes; \
                 auto-selecting one can import the scanout on the wrong device and silently corrupt it"
            } else if force_cpu {
                "a prior consumer convert failed, e.g. multi-GPU render-node mismatch"
            } else {
                "no render-node convert context: libdrmtap did not load here, or \
                 drmtap_open_render found no usable /dev/dri/renderD*"
            }
        );
    }
    if let Err(err) = conn
        .send_msg(
            &Data::DrmStart {
                display: wire_idx as i32,
                need_cpu,
            },
            None,
        )
        .await
    {
        let _ = tx.send(Err(err));
        return;
    }
    let _ = tx.send(Ok((displays, wire_idx)));

    let mut pending_cursor: Option<(u64, u32, u32, i32, i32, bool, Vec<u8>)> = None;
    let mut cal: Option<CursorCal> = None;
    let end_reason = loop {
        if stop.load(Ordering::SeqCst) {
            break "stopped".to_owned();
        }
        if pending_cursor.is_some() {
            let t = shared.cursor_transform.load(std::sync::atomic::Ordering::Acquire);
            if t != TRANSFORM_PENDING {
                if let Some((id, width, height, hotx, hoty, hot_measured, raw)) = pending_cursor.take() {
                    let ctx = *shared.cal_context.lock().unwrap();
                    cal = on_cursor_shape(
                        display, cursor_epoch, ctx, id, width, height, hotx, hoty, hot_measured, raw, t,
                    );
                }
            }
        }
        let (msg, recv_fd) = match conn.recv_msg_timeout2(200).await {
            None => continue,
            Some(Ok(pair)) => pair,
            Some(Err(err)) => break format!("recv: {err}"),
        };
        match msg {
            Data::DrmFrameDmabuf(desc) => {
                if cal.is_some() {
                    on_cursor_plane(
                        &mut cal,
                        desc.cursor_pos,
                        super::input_service::last_peer_abs_sample,
                        super::display_service::wayland_layout_drifted(),
                        scrap::wayland::display::wayland_snapshot_generation(),
                        super::display_service::input_map_gen(),
                        super::display_service::input_map_epoch(),
                        display,
                        cursor_epoch,
                    );
                }
                let conv = match converter.as_mut() {
                    Some(c) => c,
                    None => break "no DRM render node; cannot convert dma-buf frame".to_owned(),
                };
                let received_fd: RawFd = if desc.has_fd {
                    match recv_fd.as_ref() {
                        Some(f) => f.as_raw_fd(),
                        None => {
                            break "dma-buf frame set has_fd but carried no SCM_RIGHTS fd".to_owned()
                        }
                    }
                } else {
                    -1
                };
                let mut ddesc = drmtap_dmabuf_desc {
                    dma_buf_fd: -1,
                    width: desc.width,
                    height: desc.height,
                    format: desc.format,
                    modifier: desc.modifier,
                    fb_id: desc.fb_id,
                    num_planes: desc.num_planes,
                    offsets: desc.offsets,
                    pitches: desc.pitches,
                    hdr_eotf: desc.hdr_eotf,
                    hdr_max_nits: desc.hdr_max_nits,
                };
                match conv.convert(&mut ddesc, received_fd) {
                    Ok((data, w, h, fmt)) => {
                        let mut buf = shared.slot.lock().unwrap().take_free().unwrap_or_default();
                        buf.clear();
                        buf.extend_from_slice(data);
                        let mut slot = shared.slot.lock().unwrap();
                        slot.publish(w as usize, h as usize, fmt, desc.plane_rotation, buf);
                        shared.cv.notify_one();
                    }
                    Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                    Err(err) => {
                        drm_set_prefer_cpu(&our_key);
                        break format!("convert: {err}");
                    }
                }
                if let Err(err) = conn.send_frame_ack(shared.demand_fps.load(Ordering::Relaxed)).await {
                    break format!("frame ack: {err}");
                }
            }
            Data::DrmFrame {
                width,
                height,
                plane_rotation,
                cursor_pos,
            } => {
                if cal.is_some() {
                    on_cursor_plane(
                        &mut cal,
                        cursor_pos,
                        super::input_service::last_peer_abs_sample,
                        super::display_service::wayland_layout_drifted(),
                        scrap::wayland::display::wayland_snapshot_generation(),
                        super::display_service::input_map_gen(),
                        super::display_service::input_map_epoch(),
                        display,
                        cursor_epoch,
                    );
                }
                if width == 0 || height == 0 {
                    break format!("cpu frame: degenerate geometry {width}x{height}");
                }
                let need = (width as usize)
                    .saturating_mul(height as usize)
                    .saturating_mul(4);
                let mut buf = shared.slot.lock().unwrap().take_free().unwrap_or_default();
                match tokio::time::timeout(BODY_READ_TIMEOUT, conn.next_raw_into(&mut buf)).await {
                    Err(_) => break "cpu frame body read timed out".to_owned(),
                    Ok(Ok(())) => {
                        if buf.len() < need {
                            break format!(
                                "cpu frame: body {} bytes < {need} for {width}x{height}",
                                buf.len()
                            );
                        }
                        let mut slot = shared.slot.lock().unwrap();
                        slot.publish(
                            width as usize,
                            height as usize,
                            Pixfmt::BGRA,
                            plane_rotation,
                            buf,
                        );
                        shared.cv.notify_one();
                    }
                    Ok(Err(err)) => break format!("frame body: {err}"),
                }
                if let Err(err) = conn.send_frame_ack(shared.demand_fps.load(Ordering::Relaxed)).await {
                    break format!("frame ack: {err}");
                }
            }
            Data::DrmCursor {
                id,
                width,
                height,
                hotx,
                hoty,
                hot_measured,
            } => {
                let need = (width as usize)
                    .saturating_mul(height as usize)
                    .saturating_mul(4);
                let mut raw = Vec::new();
                match tokio::time::timeout(BODY_READ_TIMEOUT, conn.next_raw_into(&mut raw)).await {
                    Err(_) => break "cursor body read timed out".to_owned(),
                    Ok(Ok(())) => {
                        if raw.len() < need {
                            break format!(
                                "cursor body {} bytes < {need} for {width}x{height}",
                                raw.len()
                            );
                        }
                        let t = shared.cursor_transform.load(std::sync::atomic::Ordering::Acquire);
                        if t == TRANSFORM_PENDING {
                            cal = None;
                            pending_cursor = Some((id, width, height, hotx, hoty, hot_measured, raw));
                        } else {
                            pending_cursor = None;
                            let ctx = *shared.cal_context.lock().unwrap();
                            cal = on_cursor_shape(
                                display,
                                cursor_epoch,
                                ctx,
                                id,
                                width,
                                height,
                                hotx,
                                hoty,
                                hot_measured,
                                raw,
                                t,
                            );
                        }
                    }
                    Ok(Err(err)) => break format!("cursor body: {err}"),
                }
            }
            Data::DrmDisplaysChanged(list) => {
                let now_at_our_index = list
                    .get(display.max(0) as usize)
                    .map(|d| (d.device.clone(), d.crtc_id));
                if bound_to.is_some() && now_at_our_index != bound_to {
                    swap_available_displays(list);
                    scrap::wayland::display::clear_wayland_displays_cache();
                    break match (&bound_to, &now_at_our_index) {
                        (Some((_, was)), Some((_, now))) => format!(
                            "hotplug renumbered display {display}: it was crtc {was}, now crtc {now}"
                        ),
                        _ => format!("hotplug removed display {display} from the list"),
                    };
                }
                swap_available_displays(list);
                scrap::wayland::display::clear_wayland_displays_cache();
                UINPUT_REFRESH_GEN.fetch_add(1, Ordering::AcqRel);
                if !UINPUT_REFRESH_BUSY.swap(true, Ordering::AcqRel) {
                    let mut busy = UinputRefreshGuard(true);
                    let spawned = std::thread::Builder::new()
                        .name("drm-uinput-refresh".into())
                        .spawn(move || {
                        let rt = match tokio::runtime::Builder::new_current_thread()
                            .enable_all()
                            .build()
                        {
                            Ok(rt) => rt,
                            Err(err) => {
                                log::warn!(
                                    "drm: uinput refresh worker could not build a runtime: {err}"
                                );
                                return;
                            }
                        };
                        let mut served = 0u64;
                        loop {
                            let g = UINPUT_REFRESH_GEN.load(Ordering::Acquire);
                            if g != served {
                                served = g;
                                rt.block_on(super::wayland::update_uinput_resolution());
                                continue;
                            }
                            busy.release();
                            if UINPUT_REFRESH_GEN.load(Ordering::Acquire) == served {
                                break;
                            }
                            if !busy.retake() {
                                break;
                            }
                        }
                    });
                    if let Err(err) = spawned {
                        log::error!("drm: could not spawn the uinput refresh worker: {err}");
                    }
                }
            }
            _ => {}
        }
    };
    log::info!("drm capture stream ended: {end_reason}");
    drop(converter);
    remove_drm_cursor(display, cursor_epoch);
    let mut slot = shared.slot.lock().unwrap();
    slot.ended = Some(format!("drm stream ended ({end_reason})"));
    shared.cv.notify_one();
}

#[derive(Clone)]
pub struct DrmCursorData {
    pub id: u64,
    pub width: i32,
    pub height: i32,
    pub hotx: i32,
    pub hoty: i32,
    pub colors: Vec<u8>,
}



type Pos = (i32, i32);
type Hotspot = (i32, i32);

const CURSOR_CAL_STABLE_TICKS: u32 = 3;
const CURSOR_CAL_WINDOW_TICKS: u32 = 33;
const CURSOR_CAL_TOLERANCE: i32 = 2;
const CURSOR_CAL_MIN_INPUT_AGE_MS: u64 = 150;
const CURSOR_CAL_MAX_INPUT_AGE_MS: u64 = 10_000;

#[derive(Clone, Copy, Debug, PartialEq)]
struct CalContext {
    rect: (i32, i32, i32, i32),
    physical_size: (i32, i32),
    built_gen: u64,
}

fn calibration_context(
    drm: &[DrmDisplayInfo],
    wire_idx: usize,
    wl: &scrap::wayland::display::Displays,
    gen: u64,
) -> Result<CalContext, &'static str> {
    if wl.displays.is_empty() || (wl.displays.len() == 1 && drm.len() > 1) {
        return Err("partial wayland snapshot");
    }
    let info = drm.get(wire_idx).ok_or("wire index out of range")?;
    let j = identity_matches(drm, &wl.displays)
        .get(wire_idx)
        .copied()
        .flatten()
        .ok_or("no identity match")?;
    if wl.displays[j].transform != 0 {
        return Err("rotated output");
    }
    let rects = scrap::wayland::display::logical_rects_of_displays(&wl.displays);
    let r = rects.get(j).ok_or("no logical rect")?;
    if r.w <= 0 || r.h <= 0 || info.width == 0 || info.height == 0 {
        return Err("degenerate geometry");
    }
    Ok(CalContext {
        rect: (r.x, r.y, r.w, r.h),
        physical_size: (info.width as i32, info.height as i32),
        built_gen: gen,
    })
}

fn measure_hotspot(
    ctx: &CalContext,
    injected: Pos,
    plane: Pos,
    cursor_size: (i32, i32),
) -> Option<Hotspot> {
    let (ax, ay, aw, ah) = ctx.rect;
    let (px, py) = ctx.physical_size;
    let (ix, iy) = injected;
    if ix < ax || ix >= ax + aw || iy < ay || iy >= ay + ah {
        return None;
    }
    let tipx = ((ix - ax) as f64 * px as f64 / aw as f64).round() as i32;
    let tipy = ((iy - ay) as f64 * py as f64 / ah as f64).round() as i32;
    let h = (tipx - plane.0, tipy - plane.1);
    if h.0 < 0 || h.1 < 0 || h.0 >= cursor_size.0 || h.1 >= cursor_size.1 {
        return None;
    }
    Some(h)
}

struct CursorCal {
    id: u64,
    width: u32,
    height: u32,
    raw: Vec<u8>,
    ctx: CalContext,
    wire_hot: Hotspot,
    current_hot: Hotspot,
    candidate: Option<(Hotspot, u64, u64)>,
    plane: Option<Pos>,
    stable_ticks: u32,
    measured_this_position: bool,
}

fn calibratable(id: u64, hot_measured: bool) -> bool {
    id != scrap::drm_reader::HIDDEN_CURSOR_ID && !hot_measured
}

fn update_plane_stability(c: &mut CursorCal, p: Pos) {
    if c.plane != Some(p) {
        c.plane = Some(p);
        c.stable_ticks = 0;
        c.measured_this_position = false;
        return;
    }
    c.stable_ticks = c.stable_ticks.saturating_add(1);
}

fn settled_unmeasured(c: &CursorCal) -> bool {
    !c.measured_this_position
        && (CURSOR_CAL_STABLE_TICKS..=CURSOR_CAL_WINDOW_TICKS).contains(&c.stable_ticks)
}

fn usable_sample(
    ctx: &CalContext,
    s: Option<super::input_service::PeerAbsSample>,
    drifted: bool,
    current_gen: u64,
    map_gen: u64,
    map_epoch: u64,
) -> Option<(Pos, u64, u64)> {
    if drifted || current_gen != ctx.built_gen || map_gen != ctx.built_gen {
        return None;
    }
    let s = s?;
    if s.gen != ctx.built_gen || s.map_epoch != map_epoch {
        return None;
    }
    if !(CURSOR_CAL_MIN_INPUT_AGE_MS..=CURSOR_CAL_MAX_INPUT_AGE_MS).contains(&s.age_ms) {
        return None;
    }
    Some((s.pos, s.seq, s.map_epoch))
}

fn near(a: Hotspot, b: Hotspot) -> bool {
    (a.0 - b.0).abs() <= CURSOR_CAL_TOLERANCE && (a.1 - b.1).abs() <= CURSOR_CAL_TOLERANCE
}

fn observe_measurement(c: &mut CursorCal, m: Hotspot, seq: u64, map_epoch: u64) -> Option<Hotspot> {
    match c.candidate {
        Some((_, kseq, _)) if kseq == seq => None,
        Some((k, _, kepoch)) if kepoch == map_epoch && near(k, m) => {
            c.candidate = None;
            Some(k)
        }
        _ => {
            c.candidate = Some((m, seq, map_epoch));
            None
        }
    }
}

fn apply_confirmed_hotspot(c: &mut CursorCal, accepted: Hotspot, display: i32, cursor_epoch: u64) {
    if near(accepted, c.current_hot) {
        if c.current_hot == c.wire_hot {
            forget_cursor_cal(c.id, c.ctx.built_gen);
        } else if cached_cursor_cal(c.id, c.ctx.built_gen) != Some(c.current_hot) {
            store_cursor_cal(c.id, c.current_hot, c.ctx.built_gen);
        }
        return;
    }
    let target = if near(accepted, c.wire_hot) {
        forget_cursor_cal(c.id, c.ctx.built_gen);
        c.wire_hot
    } else {
        store_cursor_cal(c.id, accepted, c.ctx.built_gen);
        accepted
    };
    debug_assert_ne!(target, c.current_hot, "not near the served value, so a change");
    c.current_hot = target;
    let id = if target == c.wire_hot {
        c.id
    } else {
        remix_cursor_id(c.id, target.0, target.1)
    };
    log::info!(
        "drm: cursor hotspot for display {display}: confirmed {target:?} (wire {:?}); serving it",
        c.wire_hot
    );
    deliver_drm_cursor(
        display,
        cursor_epoch,
        id,
        c.width,
        c.height,
        target.0,
        target.1,
        false,
        c.raw.clone(),
        0,
    );
}

static CURSOR_CAL_CACHE: Mutex<BTreeMap<u64, (Hotspot, u64)>> = Mutex::new(BTreeMap::new());
const CURSOR_CAL_CACHE_CAP: usize = 64;

fn store_cursor_cal(wire_id: u64, h: Hotspot, gen: u64) {
    let mut cache = CURSOR_CAL_CACHE.lock().unwrap();
    if cache.get(&wire_id).is_some_and(|(_, g)| *g > gen) {
        return;
    }
    if cache.len() >= CURSOR_CAL_CACHE_CAP && !cache.contains_key(&wire_id) {
        cache.retain(|_, (_, g)| *g > gen);
        if cache.len() >= CURSOR_CAL_CACHE_CAP {
            return;
        }
    }
    cache.insert(wire_id, (h, gen));
}
fn forget_cursor_cal(wire_id: u64, gen: u64) {
    let mut cache = CURSOR_CAL_CACHE.lock().unwrap();
    if cache.get(&wire_id).is_some_and(|(_, g)| *g <= gen) {
        cache.remove(&wire_id);
    }
}
fn cached_cursor_cal(wire_id: u64, gen: u64) -> Option<Hotspot> {
    CURSOR_CAL_CACHE
        .lock()
        .unwrap()
        .get(&wire_id)
        .filter(|(_, g)| *g == gen)
        .map(|(h, _)| *h)
}

fn remix_cursor_id(wire_id: u64, hotx: i32, hoty: i32) -> u64 {
    let mut id = wire_id ^ 0x9e37_79b9_7f4a_7c15;
    for v in [hotx as u32 as u64, hoty as u32 as u64] {
        id ^= v;
        id = id.wrapping_mul(1099511628211);
    }
    id
}

#[allow(clippy::too_many_arguments)]
fn on_cursor_shape(
    display: i32,
    cursor_epoch: u64,
    ctx: Option<CalContext>,
    id: u64,
    width: u32,
    height: u32,
    hotx: i32,
    hoty: i32,
    hot_measured: bool,
    raw: Vec<u8>,
    t: i32,
) -> Option<CursorCal> {
    let cal = ctx.filter(|_| calibratable(id, hot_measured)).map(|ctx| {
        debug_assert_eq!(t, 0, "a calibration context implies an upright output");
        let wire_hot = (hotx, hoty);
        CursorCal {
            id,
            width,
            height,
            raw: raw.clone(),
            ctx,
            wire_hot,
            current_hot: cached_cursor_cal(id, ctx.built_gen).unwrap_or(wire_hot),
            candidate: None,
            plane: None,
            stable_ticks: 0,
            measured_this_position: false,
        }
    });
    let (pid, hx, hy) = match &cal {
        Some(c) if c.current_hot != c.wire_hot => (
            remix_cursor_id(id, c.current_hot.0, c.current_hot.1),
            c.current_hot.0,
            c.current_hot.1,
        ),
        _ => (id, hotx, hoty),
    };
    deliver_drm_cursor(display, cursor_epoch, pid, width, height, hx, hy, hot_measured, raw, t);
    cal
}

#[allow(clippy::too_many_arguments)]
fn on_cursor_plane(
    cal: &mut Option<CursorCal>,
    pos: Option<Pos>,
    sample: impl FnOnce() -> Option<super::input_service::PeerAbsSample>,
    drifted: bool,
    current_gen: u64,
    map_gen: u64,
    map_epoch: u64,
    display: i32,
    cursor_epoch: u64,
) {
    let (Some(c), Some(p)) = (cal.as_mut(), pos) else {
        return;
    };
    update_plane_stability(c, p);
    if !settled_unmeasured(c) {
        return;
    }
    let Some((injected, seq, epoch)) =
        usable_sample(&c.ctx, sample(), drifted, current_gen, map_gen, map_epoch)
    else {
        return;
    };
    c.measured_this_position = true;
    let Some(m) = measure_hotspot(&c.ctx, injected, p, (c.width as i32, c.height as i32)) else {
        return;
    };
    let Some(accepted) = observe_measurement(c, m, seq, epoch) else {
        log::debug!("drm: cursor hotspot for display {display}: measured {m:?} (sample {seq}), waiting for a second one");
        return;
    };
    apply_confirmed_hotspot(c, accepted, display, cursor_epoch);
}

static DRM_CURSOR: Mutex<BTreeMap<i32, (u64, DrmCursorData)>> = Mutex::new(BTreeMap::new());
static DRM_CURSOR_EPOCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn next_cursor_epoch() -> u64 {
    DRM_CURSOR_EPOCH.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

fn set_drm_cursor(display: i32, epoch: u64, c: DrmCursorData) {
    let mut map = DRM_CURSOR.lock().unwrap();
    match map.get(&display) {
        Some((stored, _)) if *stored > epoch => {}
        _ => {
            map.insert(display, (epoch, c));
        }
    }
}

fn remove_drm_cursor(display: i32, epoch: u64) {
    let mut map = DRM_CURSOR.lock().unwrap();
    if map.get(&display).map(|(e, _)| *e) == Some(epoch) {
        map.remove(&display);
    }
}

#[allow(clippy::too_many_arguments)]
fn deliver_drm_cursor(
    display: i32,
    cursor_epoch: u64,
    id: u64,
    width: u32,
    height: u32,
    hotx: i32,
    hoty: i32,
    hot_measured: bool,
    raw: Vec<u8>,
    t: i32,
) {
    let (width, height, hotx, hoty, colors) = if t != 0 {
        let mut turned = Vec::new();
        unrotate_bgra(&raw, width as usize, height as usize, t, &mut turned);
        let (dw, dh) = rotated_dims(t, width as usize, height as usize);
        let (hx, hy) = if hot_measured {
            unrotate_hotspot(t, width as i32, height as i32, hotx, hoty)
        } else {
            scrap::drm_reader::infer_hotspot(&turned, dw, dh)
        };
        (dw as i32, dh as i32, hx, hy, turned)
    } else {
        (width as i32, height as i32, hotx, hoty, raw)
    };
    note_received_provenance(display, id, hot_measured);
    let id = fold_cursor_id(id, t);
    set_drm_cursor(
        display,
        cursor_epoch,
        DrmCursorData {
            id,
            width,
            height,
            hotx,
            hoty,
            colors,
        },
    );
}

static LAST_RECEIVED_PROVENANCE: Mutex<BTreeMap<i32, bool>> = Mutex::new(BTreeMap::new());

fn provenance_is_news_for(display: i32, hot_measured: bool) -> bool {
    LAST_RECEIVED_PROVENANCE
        .lock()
        .unwrap()
        .insert(display, hot_measured)
        != Some(hot_measured)
}

fn note_received_provenance(display: i32, id: u64, hot_measured: bool) {
    if id == scrap::drm_reader::HIDDEN_CURSOR_ID {
        return;
    }
    if !provenance_is_news_for(display, hot_measured) {
        return;
    }
    if hot_measured {
        log::info!(
            "drm: cursor hotspots arrive MEASURED; mapping the point the producer sent. \
             (a producer that predates the field reads as measured too, on purpose: that \
              preserves what the old protocol meant)"
        );
    } else {
        log::info!(
            "drm: cursor hotspots arrive as a GUESS; re-inferring from the upright bitmap"
        );
    }
}

fn fold_cursor_id(id: u64, t: i32) -> u64 {
    if id == scrap::drm_reader::HIDDEN_CURSOR_ID {
        id
    } else {
        (id ^ t as u32 as u64).wrapping_mul(1099511628211)
    }
}

fn with_drm_cursor<T>(f: impl Fn(&DrmCursorData) -> T) -> Option<T> {
    let map = DRM_CURSOR.lock().unwrap();
    map.values()
        .map(|(_, c)| c)
        .find(|c| c.id != scrap::drm_reader::HIDDEN_CURSOR_ID)
        .or_else(|| map.values().map(|(_, c)| c).next())
        .map(f)
}

pub fn drm_cursor_id() -> Option<u64> {
    with_drm_cursor(|c| c.id)
}

pub fn drm_cursor() -> Option<DrmCursorData> {
    with_drm_cursor(|c| c.clone())
}

enum ProbeState {
    Unknown,
    Unavailable(Instant),
    Available(Instant, Vec<DrmDisplayInfo>),
}

static DRM_STATE: Mutex<ProbeState> = Mutex::new(ProbeState::Unknown);
const NEGATIVE_TTL: Duration = Duration::from_secs(30);
const POSITIVE_TTL: Duration = Duration::from_secs(15);

fn query_displays(wake: bool) -> ResultType<Vec<DrmDisplayInfo>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("drm-query".into())
        .spawn(move || {
            let _ = tx.send(query_displays_async(wake));
        })
        .map_err(|err| anyhow!("could not spawn the drm display query thread: {err}"))?;
    rx.recv_timeout(Duration::from_millis(HANDSHAKE_WAIT_MS))
        .map_err(|_| anyhow!("drm display query timed out"))?
}

#[tokio::main(flavor = "current_thread")]
async fn query_displays_async(wake: bool) -> ResultType<Vec<DrmDisplayInfo>> {
    query_displays_inner(wake).await
}

async fn query_displays_inner(wake: bool) -> ResultType<Vec<DrmDisplayInfo>> {
    let mut conn = connect_drm(DRM_CONNECT_TIMEOUT_MS).await?;
    conn.send_msg(&Data::DrmHello { wake }, None).await?;
    match conn.recv_msg_timeout2(DISPLAY_LIST_TIMEOUT_MS).await {
        Some(Ok((Data::DrmDisplayList(v), _fd))) => Ok(v),
        Some(Ok((other, _fd))) => Err(anyhow!("expected DrmDisplayList, got {:?}", other)),
        Some(Err(err)) => Err(err),
        None => Err(anyhow!("timed out waiting for DrmDisplayList")),
    }
}

static DRM_PROBE_FAILURES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
const DRM_PROBE_MAX_FAILURES: u32 = 5;
static DRM_REFRESH_FAILURES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
const DRM_REFRESH_MAX_FAILURES: u32 = 3;
static DRM_PROBE_IN_FLIGHT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

static DRM_STATE_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[inline]
fn publish_probe_state(st: &mut ProbeState, next: ProbeState) {
    *st = next;
    DRM_STATE_GEN.fetch_add(1, Ordering::Release);
}

struct ProbeInFlightGuard;
impl Drop for ProbeInFlightGuard {
    fn drop(&mut self) {
        DRM_PROBE_IN_FLIGHT.store(false, Ordering::Release);
    }
}

struct UinputRefreshGuard(bool);
impl UinputRefreshGuard {
    fn release(&mut self) {
        if self.0 {
            self.0 = false;
            UINPUT_REFRESH_BUSY.store(false, Ordering::Release);
        }
    }
    fn retake(&mut self) -> bool {
        self.0 = !UINPUT_REFRESH_BUSY.swap(true, Ordering::AcqRel);
        self.0
    }
}
impl Drop for UinputRefreshGuard {
    fn drop(&mut self) {
        self.release();
    }
}

pub(crate) fn is_available_cached() -> bool {
    matches!(&*DRM_STATE.lock().unwrap(), ProbeState::Available(..))
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Availability {
    Available,
    Unavailable,
    Unsettled,
}

fn availability() -> Availability {
    let (verdict, stale_no) = {
        let st = DRM_STATE.lock().unwrap();
        let stale_no =
            matches!(&*st, ProbeState::Unavailable(since) if since.elapsed() >= NEGATIVE_TTL);
        let verdict = match &*st {
            ProbeState::Available(since, _) => {
                Some((Availability::Available, since.elapsed() >= POSITIVE_TTL))
            }
            ProbeState::Unavailable(_) => Some((Availability::Unavailable, false)),
            ProbeState::Unknown => None,
        };
        (verdict, stale_no)
    };
    if let Some((answer, stale)) = verdict {
        if stale {
            refresh_available_async();
        }
        if stale_no {
            refresh_unavailable_async();
        }
        return answer;
    }
    if DRM_PROBE_IN_FLIGHT.swap(true, Ordering::AcqRel) {
        return match &*DRM_STATE.lock().unwrap() {
            ProbeState::Available(..) => Availability::Available,
            ProbeState::Unavailable(_) => Availability::Unavailable,
            ProbeState::Unknown => Availability::Unsettled,
        };
    }
    let _in_flight = ProbeInFlightGuard;
    probe_and_publish()
}

pub(crate) fn availability_cached() -> Availability {
    let (verdict, stale_no) = {
        let st = DRM_STATE.lock().unwrap();
        let stale_no =
            matches!(&*st, ProbeState::Unavailable(since) if since.elapsed() >= NEGATIVE_TTL);
        let verdict = match &*st {
            ProbeState::Available(since, _) => {
                Some((Availability::Available, since.elapsed() >= POSITIVE_TTL))
            }
            ProbeState::Unavailable(_) => Some((Availability::Unavailable, false)),
            ProbeState::Unknown => None,
        };
        (verdict, stale_no)
    };
    if let Some((answer, stale)) = verdict {
        if stale {
            refresh_available_async();
        }
        if stale_no {
            refresh_unavailable_async();
        }
        return answer;
    }
    if !DRM_PROBE_IN_FLIGHT.swap(true, Ordering::AcqRel) {
        let in_flight = ProbeInFlightGuard;
        let spawned = std::thread::Builder::new()
            .name("drm-avail-probe".into())
            .spawn(move || {
                let _in_flight = in_flight;
                probe_and_publish();
            });
        if let Err(err) = spawned {
            log::warn!("drm: could not spawn the availability probe thread: {err}");
        }
    }
    Availability::Unsettled
}

fn probe_and_publish() -> Availability {
    let t = Instant::now();
    let result = query_displays(true);
    let mut st = DRM_STATE.lock().unwrap();
    let answer = match result {
        Ok(list) if !list.is_empty() => {
            log::debug!(
                "drm: availability probe -> available ({} displays) in {:?}",
                list.len(),
                t.elapsed()
            );
            DRM_PROBE_FAILURES.store(0, Ordering::Relaxed);
            publish_probe_state(&mut st, ProbeState::Available(Instant::now(), list));
            Availability::Available
        }
        Ok(_) => {
            log::info!("drm: availability probe -> no displays in {:?}", t.elapsed());
            publish_probe_state(&mut st, ProbeState::Unavailable(Instant::now()));
            Availability::Unavailable
        }
        Err(err) => {
            let n = DRM_PROBE_FAILURES.fetch_add(1, Ordering::Relaxed) + 1;
            if n >= DRM_PROBE_MAX_FAILURES {
                log::info!("drm: availability probe failed {n}x ({err}); disabling DRM");
                publish_probe_state(&mut st, ProbeState::Unavailable(Instant::now()));
                Availability::Unavailable
            } else {
                log::info!(
                    "drm: availability probe failed ({err}), attempt {n}/{DRM_PROBE_MAX_FAILURES}; will retry"
                );
                Availability::Unsettled
            }
        }
    };
    drop(st);
    answer
}

pub(crate) fn is_available() -> bool {
    availability() == Availability::Available
}

fn refresh_unavailable_async() {
    if DRM_PROBE_IN_FLIGHT.swap(true, Ordering::AcqRel) {
        return;
    }
    let in_flight = ProbeInFlightGuard;
    let sampled_gen = {
        let st = DRM_STATE.lock().unwrap();
        match &*st {
            ProbeState::Unavailable(since) if since.elapsed() >= NEGATIVE_TTL => {}
            _ => return,
        }
        DRM_STATE_GEN.load(Ordering::Acquire)
    };
    let spawned = std::thread::Builder::new()
        .name("drm-unavail-refresh".into())
        .spawn(move || {
            let _in_flight = in_flight;
            let result = query_displays(true);
            let mut st = DRM_STATE.lock().unwrap();
            if DRM_STATE_GEN.load(Ordering::Acquire) != sampled_gen {
                return;
            }
            match result {
                Ok(list) if !list.is_empty() => {
                    log::info!(
                        "drm: availability re-probe -> available ({} displays)",
                        list.len()
                    );
                    DRM_PROBE_FAILURES.store(0, Ordering::Relaxed);
                    publish_probe_state(&mut st, ProbeState::Available(Instant::now(), list));
                    drop(st);
                    scrap::wayland::display::clear_wayland_displays_cache();
                }
                _ => {
                    publish_probe_state(&mut st, ProbeState::Unavailable(Instant::now()));
                }
            }
        });
    if let Err(err) = spawned {
        log::warn!("drm: could not spawn the unavailability re-probe thread: {err}");
    }
}

fn refresh_available_async() {
    if DRM_PROBE_IN_FLIGHT.swap(true, Ordering::AcqRel) {
        return;
    }
    let in_flight = ProbeInFlightGuard;
    let sampled_gen = {
        let st = DRM_STATE.lock().unwrap();
        if !matches!(&*st, ProbeState::Available(..)) {
            return;
        }
        DRM_STATE_GEN.load(Ordering::Acquire)
    };
    let spawned = std::thread::Builder::new()
        .name("drm-avail-refresh".into())
        .spawn(move || {
            let _in_flight = in_flight;
            let result = query_displays(true);
            let mut st = DRM_STATE.lock().unwrap();
            if DRM_STATE_GEN.load(Ordering::Acquire) != sampled_gen {
                return;
            }
            let failures = match &result {
                Ok(_) => {
                    DRM_REFRESH_FAILURES.store(0, Ordering::Relaxed);
                    0
                }
                Err(_) => DRM_REFRESH_FAILURES.fetch_add(1, Ordering::Relaxed) + 1,
            };
            match refresh_outcome(result.as_ref().ok().map(|l| l.len()), failures) {
                RefreshOutcome::Publish => {
                    let fresh = result.unwrap_or_default();
                    let changed = match &*st {
                        ProbeState::Available(_, old) => *old != fresh,
                        _ => true,
                    };
                    publish_probe_state(&mut st, ProbeState::Available(Instant::now(), fresh));
                    if changed {
                        drop(st);
                        scrap::wayland::display::clear_wayland_displays_cache();
                    }
                }
                RefreshOutcome::Unavailable => {
                    log::info!("drm: refresh -> 0 displays, marking DRM unavailable");
                    publish_probe_state(&mut st, ProbeState::Unavailable(Instant::now()));
                }
                RefreshOutcome::Restamp => {
                    if let ProbeState::Available(since, _) = &mut *st {
                        *since = Instant::now();
                    }
                }
                RefreshOutcome::GiveUp => {
                    log::info!(
                        "drm: availability refresh failed {failures}x ({:?}); the producer looks \
                         gone, dropping the cached verdict so the next enumeration re-probes",
                        result.as_ref().err()
                    );
                    DRM_REFRESH_FAILURES.store(0, Ordering::Relaxed);
                    publish_probe_state(&mut st, ProbeState::Unknown);
                }
            }
        });
    if let Err(err) = spawned {
        log::warn!(
            "drm: could not spawn the availability refresh thread: {err}; the cached verdict \
             stays stale until the next probe"
        );
    }
}

pub(super) fn warm_availability() {
    for _ in 0..10 {
        if crate::platform::linux::is_x11_for_drm() {
            std::thread::sleep(Duration::from_millis(300));
            continue;
        }
        let sampled_gen = {
            let st = DRM_STATE.lock().unwrap();
            if matches!(&*st, ProbeState::Available(..)) {
                return;
            }
            DRM_STATE_GEN.load(Ordering::Acquire)
        };
        match query_displays(false) {
            Ok(list) if !list.is_empty() => {
                let mut st = DRM_STATE.lock().unwrap();
                if DRM_STATE_GEN.load(Ordering::Acquire) == sampled_gen {
                    log::info!("drm: consumer cache warmed ({} displays) at startup", list.len());
                    publish_probe_state(&mut st, ProbeState::Available(Instant::now(), list));
                }
                return;
            }
            Ok(_) => {
                log::info!("drm: no display is lit at startup; the first login probes again");
                return;
            }
            Err(_) => std::thread::sleep(Duration::from_millis(300)),
        }
    }
    log::info!("drm: consumer cache warm found no producer at startup (will probe lazily)");
}

pub(super) async fn settle_unknown_availability() {
    let unknown = matches!(&*DRM_STATE.lock().unwrap(), ProbeState::Unknown);
    if unknown {
        if let Err(err) = tokio::task::spawn_blocking(availability).await {
            log::warn!("drm: login availability settlement task failed: {err}");
        }
    }
}

pub(super) async fn refresh_displays_for_login() {
    let sampled_gen = {
        let st = DRM_STATE.lock().unwrap();
        if !matches!(&*st, ProbeState::Available(..)) {
            return;
        }
        DRM_STATE_GEN.load(Ordering::Acquire)
    };
    let t = Instant::now();
    match query_displays_inner(true).await {
        Ok(list) if !list.is_empty() => {
            let changed = {
                let mut st = DRM_STATE.lock().unwrap();
                if DRM_STATE_GEN.load(Ordering::Acquire) != sampled_gen {
                    log::debug!(
                        "drm: login display refresh superseded while probing; keeping the newer list"
                    );
                    return;
                }
                match &*st {
                    ProbeState::Available(_, old) => {
                        let changed = *old != list;
                        log::debug!(
                            "drm: login display refresh -> {} display(s) in {:?}{}",
                            list.len(),
                            t.elapsed(),
                            if changed { " (list changed)" } else { "" }
                        );
                        publish_probe_state(&mut st, ProbeState::Available(Instant::now(), list));
                        changed
                    }
                    _ => return,
                }
            };
            if changed {
                scrap::wayland::display::clear_wayland_displays_cache();
            }
        }
        Ok(_) => log::debug!(
            "drm: login display refresh found no displays in {:?}; keeping the cached list",
            t.elapsed()
        ),
        Err(err) => log::debug!(
            "drm: login display refresh failed in {:?} ({err}); keeping the cached list",
            t.elapsed()
        ),
    }
}

pub(super) fn display_count_and_any_demoted() -> Option<(usize, bool)> {
    let (len, keys): (usize, Vec<String>) = match &*DRM_STATE.lock().unwrap() {
        ProbeState::Available(_, list) => (
            list.len(),
            if list.len() > 1 {
                list.iter().map(connector_key).collect()
            } else {
                Vec::new()
            },
        ),
        _ => return None,
    };
    let any_demoted = if len > 1 {
        let health = DRM_DISPLAY_HEALTH.lock().unwrap();
        keys.iter()
            .any(|k| health.get(k).is_some_and(|h| h.demoted()))
    } else {
        false
    };
    Some((len, any_demoted))
}

fn mark_demoted_displays(list: &[DrmDisplayInfo], infos: &mut [DisplayInfo]) {
    let health = DRM_DISPLAY_HEALTH.lock().unwrap();
    if list.len() <= 1 {
        if let (Some(display), Some(info)) = (list.first(), infos.first_mut()) {
            if health
                .get(&connector_key(display))
                .is_some_and(|health| health.demoted() && health.fallback_rejected)
            {
                info.online = false;
            }
        }
        return;
    }
    for (display, info) in list.iter().zip(infos.iter_mut()) {
        if health
            .get(&connector_key(display))
            .is_some_and(|health| health.demoted())
        {
            info.online = false;
        }
    }
}

pub(super) fn mark_fallback_rejected(display_idx: usize) {
    let Some(expected) = display_info_of(display_idx as i32) else {
        return;
    };
    DRM_DISPLAY_HEALTH
        .lock()
        .unwrap()
        .entry(connector_key(&expected))
        .or_insert_with(DisplayHealth::new)
        .fallback_rejected = true;
}

fn primary_index_from_assignment(assignment: &[Option<usize>], primary: usize) -> usize {
    assignment
        .iter()
        .position(|assigned| *assigned == Some(primary))
        .unwrap_or(0)
}

pub(super) fn get_display_infos_and_primary() -> Option<(Vec<DisplayInfo>, usize)> {
    let list = match &*DRM_STATE.lock().unwrap() {
        ProbeState::Available(_, list) => list.clone(),
        _ => return None,
    };
    let wl = scrap::wayland::display::get_displays();
    let assignment = assign_wayland_outputs(&list, &wl.displays);
    let mut infos = augment_with_wayland_geometry_from(&list, &wl, &assignment);
    mark_demoted_displays(&list, &mut infos);
    let primary = primary_index_from_assignment(&assignment, wl.primary);
    Some((infos, primary))
}

pub(super) fn get_display_infos() -> Option<Vec<DisplayInfo>> {
    let list = match &*DRM_STATE.lock().unwrap() {
        ProbeState::Available(_, list) => list.clone(),
        _ => return None,
    };
    let mut infos = augment_with_wayland_geometry(&list);
    mark_demoted_displays(&list, &mut infos);
    Some(infos)
}

fn augment_with_wayland_geometry(drm: &[DrmDisplayInfo]) -> Vec<DisplayInfo> {
    let wl = scrap::wayland::display::get_displays();
    let assignment = assign_wayland_outputs(drm, &wl.displays);
    augment_with_wayland_geometry_from(drm, &wl, &assignment)
}

fn augment_with_wayland_geometry_from(
    drm: &[DrmDisplayInfo],
    wl: &scrap::wayland::display::Displays,
    matched: &[Option<usize>],
) -> Vec<DisplayInfo> {
    let mut infos: Vec<DisplayInfo> = drm.iter().map(display_info_from_drm).collect();
    if drm.is_empty() {
        return infos;
    }
    if wl.displays.is_empty() {
        return infos;
    }
    let origin_only = wl.displays.len() == 1;
    if origin_only && drm.len() > 1 {
        return infos;
    }
    let identity = identity_matches(drm, &wl.displays);
    for (i, info) in infos.iter_mut().enumerate() {
        let Some(w) = matched[i].map(|j| &wl.displays[j]) else {
            continue;
        };
        info.x = w.x;
        info.y = w.y;
        let is_identity = identity[i].is_some() && identity[i] == matched[i];
        if is_identity && !drm[i].plane_transposed && (w.transform == 90 || w.transform == 270) {
            std::mem::swap(&mut info.width, &mut info.height);
            info.original_resolution = super::display_service::get_original_resolution(
                &drm[i].name,
                info.width as usize,
                info.height as usize,
            );
        }
        if origin_only {
            continue;
        }
        if let Some((lw, lh)) = w.logical_size {
            if lw > 0 && lh > 0 {
                info.scale = info.width as f64 / lw as f64;
                info.original_resolution = super::display_service::get_original_resolution(
                    &drm[i].name,
                    lw as usize,
                    lh as usize,
                );
            }
        }
    }
    infos
}

fn identity_matches(
    drm: &[DrmDisplayInfo],
    wl: &[base::platform::linux::WaylandDisplayInfo],
) -> Vec<Option<usize>> {
    let mut taken = vec![false; wl.len()];
    let mut matched: Vec<Option<usize>> = vec![None; drm.len()];
    for (i, d) in drm.iter().enumerate() {
        let dn = normalize_connector(&d.name);
        if let Some((j, _)) = wl
            .iter()
            .enumerate()
            .find(|(j, w)| !taken[*j] && normalize_connector(&w.name) == dn)
        {
            matched[i] = Some(j);
            taken[j] = true;
        }
    }
    for (i, d) in drm.iter().enumerate() {
        if matched[i].is_some() {
            continue;
        }
        let free_same: Vec<usize> = wl
            .iter()
            .enumerate()
            .filter(|(j, w)| !taken[*j] && w.width == d.width as i32 && w.height == d.height as i32)
            .map(|(j, _)| j)
            .collect();
        let unmatched_same = drm
            .iter()
            .enumerate()
            .filter(|(k, o)| matched[*k].is_none() && o.width == d.width && o.height == d.height)
            .count();
        if free_same.len() == 1 && unmatched_same == 1 {
            matched[i] = Some(free_same[0]);
            taken[free_same[0]] = true;
        }
    }
    matched
}

fn assign_wayland_outputs(
    drm: &[DrmDisplayInfo],
    wl: &[base::platform::linux::WaylandDisplayInfo],
) -> Vec<Option<usize>> {
    let mut matched = identity_matches(drm, wl);
    let mut taken = vec![false; wl.len()];
    for m in matched.iter().flatten() {
        taken[*m] = true;
    }
    for (i, d) in drm.iter().enumerate() {
        if matched[i].is_some() {
            continue;
        }
        let free_same_size = wl
            .iter()
            .enumerate()
            .position(|(j, w)| !taken[j] && w.width == d.width as i32 && w.height == d.height as i32);
        let Some(j) = free_same_size.or_else(|| taken.iter().position(|t| !t)) else {
            continue;
        };
        log::warn!(
            "drm: connector {} matched no compositor output by name or by a unique resolution; \
             falling back to layout order and taking {} at ({}, {})",
            d.name,
            wl[j].name,
            wl[j].x,
            wl[j].y
        );
        matched[i] = Some(j);
        taken[j] = true;
    }
    matched
}


fn normalize_connector(name: &str) -> String {
    let parts: Vec<&str> = name.split('-').collect();
    if parts.len() == 3 && parts[1].len() == 1 && parts[1].chars().all(|c| c.is_ascii_alphabetic()) {
        format!("{}-{}", parts[0], parts[2])
    } else {
        name.to_string()
    }
}

fn swap_available_displays(list: Vec<DrmDisplayInfo>) {
    let mut st = DRM_STATE.lock().unwrap();
    if matches!(&*st, ProbeState::Available(..)) {
        if list.is_empty() {
            log::info!("drm: hotplug refresh -> 0 displays, marking DRM unavailable");
            publish_probe_state(&mut st, ProbeState::Unavailable(Instant::now()));
        } else {
            log::info!("drm: hotplug refresh -> {} display(s)", list.len());
            publish_probe_state(&mut st, ProbeState::Available(Instant::now(), list));
        }
    }
}

fn display_info_from_drm(d: &DrmDisplayInfo) -> DisplayInfo {
    let (width, height) = rotated_dims(
        delivered_transform(d, 0),
        d.width as usize,
        d.height as usize,
    );
    let original_resolution =
        super::display_service::get_original_resolution(&d.name, width, height);
    DisplayInfo {
        x: d.x,
        y: d.y,
        width: width as i32,
        height: height as i32,
        name: d.name.clone(),
        online: d.active,
        cursor_embedded: false,
        original_resolution,
        scale: 1.0,
        ..Default::default()
    }
}

pub(super) fn get_capturer_info(
    display_idx: usize,
) -> ResultType<super::video_service::CapturerInfo> {
    let expected = display_info_of(display_idx as i32);
    let key = expected.as_ref().map(connector_key);
    {
        let mut map = DRM_DISPLAY_HEALTH.lock().unwrap();
        if let Some(h) = key.as_ref().and_then(|k| map.get_mut(k)) {
            if h.zero_frame_streak >= DRM_GRAB_MAX_FAILURES {
                if h.demoted() {
                    bail!(
                        "drm capture for display {display_idx} repeatedly produced no frame; using PipeWire"
                    );
                }
                h.zero_frame_streak = 0;
                h.since = Instant::now();
                h.fallback_rejected = false;
            }
        }
    }
    let (capturer, displays, wire_idx, origin) = IpcDrmCapturer::new(display_idx as i32, expected)?;
    if let Some(key) = key.clone() {
        let now = Instant::now();
        let mut map = DRM_DISPLAY_HEALTH.lock().unwrap();
        let h = map.entry(key).or_insert_with(DisplayHealth::new);
        h.rapid_builds = match h.last_build {
            Some(last) if now.duration_since(last) < RAPID_REBUILD_WINDOW => h.rapid_builds + 1,
            _ => 0,
        };
        h.last_build = Some(now);
        if h.rapid_builds >= RAPID_REBUILD_MAX {
            log::warn!(
                "drm: display {display_idx} rebuilt {} times within {RAPID_REBUILD_WINDOW:?}; flapping, falling back to PipeWire",
                h.rapid_builds
            );
            h.zero_frame_streak = DRM_GRAB_MAX_FAILURES;
            h.since = now;
            h.demotes += 1;
            bail!("drm capture for display {display_idx} is flapping; using PipeWire");
        }
    }
    let ndisplay = displays.len();
    let d = displays
        .get(wire_idx)
        .ok_or_else(|| anyhow!("drm display index {wire_idx} out of range ({ndisplay})"))?
        .clone();
    let origin = origin.unwrap_or((d.x, d.y));
    let (cap_w, cap_h) = rotated_dims(
        delivered_transform(&d, capturer.transform),
        d.width as usize,
        d.height as usize,
    );
    Ok(super::video_service::CapturerInfo {
        origin,
        width: cap_w,
        height: cap_h,
        ndisplay,
        current: display_idx,
        privacy_mode_id: 0,
        _capturer_privacy_mode_id: 0,
        capturer: Box::new(capturer),
    })
}

#[cfg(test)]
mod drm_capturer_tests {
    use super::*;

    fn capturer_with(session: Option<(usize, usize)>) -> IpcDrmCapturer {
        capturer_named(session, None)
    }

    fn capturer_named(session: Option<(usize, usize)>, key: Option<&str>) -> IpcDrmCapturer {
        let connector = key.map(|k| k.to_owned());
        IpcDrmCapturer {
            shared: Arc::new(Shared {
                slot: Mutex::new(FrameSlot {
                    latest: None,
                    free: [None, None],
                    ended: None,
                }),
                cv: Condvar::new(),
                cursor_transform: std::sync::atomic::AtomicI32::new(0),
                cal_context: Mutex::new(None),
                demand_fps: std::sync::atomic::AtomicU8::new(0),
                pace: Mutex::new(EncoderPace::default()),
            }),
            stop: Arc::new(AtomicBool::new(false)),
            display: 0,
            connector,
            session_size: session,
            transform: 0,
            snapshot_gen: scrap::wayland::display::wayland_snapshot_generation(),
            cur: Vec::new(),
            cur_w: 0,
            cur_h: 0,
            cur_fmt: Pixfmt::BGRA,
            got_frame: false,
            spare: Vec::new(),
            returned_at: Instant::now(),
        }
    }

    fn px_frame(labels: &[&[u8]], pad_bytes: usize) -> (Vec<u8>, usize, usize) {
        let h = labels.len();
        let w = labels[0].len();
        let mut buf = Vec::new();
        for row in labels {
            for &l in *row {
                buf.extend_from_slice(&[l, l, l, 255]);
            }
            buf.extend(std::iter::repeat(0u8).take(pad_bytes));
        }
        (buf, w, h)
    }

    fn labels_of(buf: &[u8], w: usize, h: usize) -> Vec<Vec<u8>> {
        (0..h)
            .map(|y| (0..w).map(|x| buf[(y * w + x) * 4]).collect())
            .collect()
    }

    #[test]
    fn the_cursor_id_names_the_orientation_too() {
        let wire = 0xDEAD_BEEF_u64;
        assert_ne!(fold_cursor_id(wire, 0), fold_cursor_id(wire, 90));
        assert_ne!(fold_cursor_id(wire, 90), fold_cursor_id(wire, 270));
        assert_eq!(fold_cursor_id(wire, 90), fold_cursor_id(wire, 90));
        let hidden = scrap::drm_reader::HIDDEN_CURSOR_ID;
        assert_eq!(fold_cursor_id(hidden, 90), hidden);
    }

    #[test]
    fn unrotate_hotspot_follows_the_pixel_mapping() {
        assert_eq!(unrotate_hotspot(90, 3, 2, 2, 0), (1, 2));
        assert_eq!(unrotate_hotspot(270, 3, 2, 2, 0), (0, 0));
        assert_eq!(unrotate_hotspot(180, 3, 2, 2, 0), (0, 1));
        assert_eq!(unrotate_hotspot(0, 3, 2, 2, 0), (2, 0));
    }

    #[test]
    fn unrotate_90_maps_the_left_column_to_the_top_row() {
        let (src, w, h) = px_frame(&[&[1, 2, 3], &[4, 5, 6]], 0);
        let mut dst = Vec::new();
        unrotate_bgra(&src, w, h, 90, &mut dst);
        assert_eq!(labels_of(&dst, h, w), vec![vec![4, 1], vec![5, 2], vec![6, 3]]);
    }

    fn arrow() -> (Vec<u8>, usize, usize) {
        let (w, h) = (4usize, 6usize);
        let mut px = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                if x <= y / 2 {
                    px[(y * w + x) * 4 + 3] = 255;
                }
            }
        }
        (px, w, h)
    }

    fn ibeam_theme_aspect() -> (Vec<u8>, usize, usize, (i32, i32)) {
        let (w, h) = (24usize, 24usize);
        let mut px = vec![0u8; w * h * 4];
        for y in 6..15 {
            for x in 2..22 {
                px[(y * w + x) * 4 + 3] = 255;
            }
        }
        (px, w, h, (11, 10))
    }

    fn as_scanned_out(upright: &[u8], w: usize, h: usize, t: i32) -> (Vec<u8>, usize, usize) {
        let mut out = Vec::new();
        unrotate_bgra(upright, w, h, (360 - t) % 360, &mut out);
        let (dw, dh) = rotated_dims(t, w, h);
        (out, dw, dh)
    }

    #[test]
    fn a_legacy_cursor_message_keeps_the_hotspot_the_old_protocol_meant() {
        use crate::ipc::Data;

        let (up, w, h, _box_centre) = ibeam_theme_aspect();
        let t = 90;
        let (scan, sw, sh) = as_scanned_out(&up, w, h, t);
        let (hotx, hoty) = (3, 5);

        let legacy = format!(
            r#"{{"t":"DrmCursor","c":{{"id":11,"width":{sw},"height":{sh},"hotx":{hotx},"hoty":{hoty}}}}}"#
        );
        let (id, mw, mh, mx, my, measured) =
            match serde_json::from_str::<Data>(&legacy).expect("a legacy DrmCursor must parse") {
                Data::DrmCursor { id, width, height, hotx, hoty, hot_measured } => {
                    (id, width, height, hotx, hoty, hot_measured)
                }
                other => panic!("expected DrmCursor, got {other:?}"),
            };
        assert!(measured, "no provenance on the wire must mean what the old protocol meant");

        let mapped = unrotate_hotspot(t, mw as i32, mh as i32, mx, my);
        let mut turned = Vec::new();
        unrotate_bgra(&scan, sw, sh, t, &mut turned);
        let (dw, dh) = rotated_dims(t, sw, sh);
        let reinferred = scrap::drm_reader::infer_hotspot(&turned, dw, dh);
        assert_ne!(
            mapped, reinferred,
            "fixture cannot discriminate: mapping and re-inferring give the same point"
        );

        let display = 9_400;
        deliver_drm_cursor(display, 1, id, mw, mh, mx, my, measured, scan, t);
        let published = {
            let map = DRM_CURSOR.lock().unwrap();
            let (_, c) = map.get(&display).expect("the delivery path published nothing");
            (c.hotx, c.hoty)
        };
        assert_eq!(
            published, mapped,
            "a legacy message must keep the hotspot the producer sent, transformed as a point; \
             re-inferring it would move the cursor for the length of an upgrade"
        );

        let modern = format!(
            r#"{{"t":"DrmCursor","c":{{"id":12,"width":{sw},"height":{sh},"hotx":{hotx},"hoty":{hoty},"hot_measured":false}}}}"#
        );
        let Data::DrmCursor { hot_measured, .. } =
            serde_json::from_str::<Data>(&modern).expect("a modern DrmCursor must parse")
        else {
            panic!("expected DrmCursor")
        };
        assert!(!hot_measured);
        let (scan2, _, _) = as_scanned_out(&up, w, h, t);
        deliver_drm_cursor(display + 1, 1, 12, sw as u32, sh as u32, hotx, hoty, false, scan2, t);
        let published2 = {
            let map = DRM_CURSOR.lock().unwrap();
            let (_, c) = map.get(&(display + 1)).expect("nothing published for the control");
            (c.hotx, c.hoty)
        };
        assert_eq!(
            published2, reinferred,
            "an explicit false must still re-infer, or the legacy default has eaten the new \
             producer's request"
        );
    }

    #[test]
    fn the_cursor_path_publishes_an_upright_sprite_at_every_angle() {
        let (up, w, h) = arrow();
        for (i, t) in [0, 90, 180, 270].into_iter().enumerate() {
            let display = 9_100 + i as i32;
            let (scan, sw, sh) = as_scanned_out(&up, w, h, t);
            deliver_drm_cursor(display, 1, 7, sw as u32, sh as u32, 0, 0, false, scan, t);
            let map = DRM_CURSOR.lock().unwrap();
            let (_, c) = map.get(&display).expect("the cursor path published nothing");
            assert_eq!(
                (c.width as usize, c.height as usize),
                (w, h),
                "the published sprite is upright-sized at {t}"
            );
            assert_eq!(c.colors, up, "the published sprite is upright at {t}");
            assert_eq!((c.hotx, c.hoty), (0, 0), "the tip is the hotspot at {t}");
        }
    }

    #[test]
    fn a_plane_that_rotated_holds_the_logical_desktop_whatever_wl_output_says() {
        assert_eq!(frame_transform(0, Some(0x4)), 0);
        assert_eq!(frame_transform(90, Some(0x4)), 0);
        assert_eq!(frame_transform(180, Some(0x2)), 0);
        assert_eq!(frame_transform(90, Some(0x8)), 0);
    }

    #[test]
    fn a_180_frame_is_turned_only_when_the_plane_did_not_turn_it() {
        use scrap::TraitPixelBuffer;
        let (src, w, h) = px_frame(&[&[1, 2, 3], &[4, 5, 6]], 0);
        let mut c = capturer_with(Some((w, h)));
        c.transform = 180;
        put_frame_with(&c, w, h, Some(0x1), &src);
        match c.frame(Duration::from_millis(50)) {
            Ok(Frame::PixelBuffer(pb)) => {
                assert_eq!((pb.width(), pb.height()), (w, h));
                assert_eq!(labels_of(pb.data(), w, h), vec![vec![6, 5, 4], vec![3, 2, 1]]);
            }
            Ok(_) => panic!("expected a pixel-buffer frame"),
            Err(err) => panic!("expected a delivered frame, got {err}"),
        }
        put_frame_with(&c, w, h, Some(0x1), &src);
        assert!(matches!(
            c.frame(Duration::from_millis(50)),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock
        ));
        put_frame_with(&c, w, h, Some(0x4), &src);
        match c.frame(Duration::from_millis(50)) {
            Ok(Frame::PixelBuffer(pb)) => {
                assert_eq!(labels_of(pb.data(), w, h), vec![vec![1, 2, 3], vec![4, 5, 6]]);
            }
            Ok(_) => panic!("expected a pixel-buffer frame"),
            Err(err) => panic!("expected a delivered frame, got {err}"),
        }
        let (other, _, _) = px_frame(&[&[7, 8, 9], &[10, 11, 12]], 0);
        put_frame_with(&c, w, h, None, &other);
        match c.frame(Duration::from_millis(50)) {
            Ok(Frame::PixelBuffer(pb)) => {
                assert_eq!(labels_of(pb.data(), w, h), vec![vec![7, 8, 9], vec![10, 11, 12]]);
            }
            Ok(_) => panic!("expected a pixel-buffer frame"),
            Err(err) => panic!("expected a delivered frame, got {err}"),
        }
    }

    #[test]
    fn a_frame_equal_to_the_last_one_goes_out_once_a_second() {
        let (still, w, h) = px_frame(&[&[1, 2, 3], &[4, 5, 6]], 0);
        let (moved, _, _) = px_frame(&[&[1, 2, 3], &[4, 5, 7]], 0);
        let mut c = capturer_with(Some((w, h)));
        put_frame_with(&c, w, h, None, &still);
        assert!(c.frame(Duration::from_millis(50)).is_ok());
        put_frame_with(&c, w, h, None, &still);
        assert!(matches!(
            c.frame(Duration::from_millis(50)),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock
        ));
        std::thread::sleep(REPEAT_EVERY);
        put_frame_with(&c, w, h, None, &still);
        assert!(c.frame(Duration::from_millis(50)).is_ok());
        put_frame_with(&c, w, h, None, &still);
        assert!(matches!(
            c.frame(Duration::from_millis(50)),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock
        ));
        put_frame_with(&c, w, h, None, &moved);
        assert!(c.frame(Duration::from_millis(50)).is_ok());
    }

    #[test]
    fn a_portrait_scanout_from_a_plane_that_rotated_90_is_left_alone() {
        use scrap::TraitPixelBuffer;
        let (portrait, pw, ph) = px_frame(&[&[1, 2], &[3, 4], &[5, 6]], 0);
        let mut c = capturer_with(Some((pw, ph)));
        c.transform = 90;
        put_frame_with(&c, pw, ph, Some(0x2), &portrait);
        match c.frame(Duration::from_millis(50)) {
            Ok(Frame::PixelBuffer(pb)) => {
                assert_eq!((pb.width(), pb.height()), (pw, ph));
                assert_eq!(labels_of(pb.data(), pw, ph), vec![vec![1, 2], vec![3, 4], vec![5, 6]]);
            }
            Ok(_) => panic!("expected a pixel-buffer frame"),
            Err(err) => panic!("expected a delivered frame, got {err}"),
        }
        let (landscape, lw, lh) = px_frame(&[&[1, 2, 3], &[4, 5, 6]], 0);
        let mut expected = Vec::new();
        unrotate_bgra(&landscape, lw, lh, 90, &mut expected);
        put_frame_with(&c, lw, lh, Some(0x1), &landscape);
        match c.frame(Duration::from_millis(50)) {
            Ok(Frame::PixelBuffer(pb)) => {
                assert_eq!((pb.width(), pb.height()), (pw, ph));
                assert_eq!(pb.data(), &expected[..]);
            }
            Ok(_) => panic!("expected a pixel-buffer frame"),
            Err(err) => panic!("expected a delivered frame, got {err}"),
        }
    }

    #[test]
    fn unrotate_reads_padded_strides_and_writes_tight() {
        let (src, w, h) = px_frame(&[&[1, 2, 3], &[4, 5, 6]], 8);
        let mut dst = Vec::new();
        unrotate_bgra(&src, w, h, 90, &mut dst);
        assert_eq!(dst.len(), w * h * 4);
        assert_eq!(labels_of(&dst, h, w), vec![vec![4, 1], vec![5, 2], vec![6, 3]]);
        let mut plain = Vec::new();
        unrotate_bgra(&src, w, h, 0, &mut plain);
        assert_eq!(labels_of(&plain, w, h), vec![vec![1, 2, 3], vec![4, 5, 6]]);
    }

    #[test]
    fn a_rotated_session_delivers_rotated_frames_and_guards_in_rotated_dims() {
        use scrap::TraitPixelBuffer;
        let mut c = capturer_with(Some((32, 64)));
        c.transform = 90;
        put_frame(&c, 64, 32);
        match c.frame(Duration::from_millis(50)) {
            Ok(Frame::PixelBuffer(pb)) => {
                assert_eq!((pb.width(), pb.height()), (32, 64));
            }
            Ok(_) => panic!("expected a pixel-buffer frame"),
            Err(err) => panic!("expected a delivered frame, got {err}"),
        }
        put_frame(&c, 32, 64);
        let err = match c.frame(Duration::from_millis(50)) {
            Err(e) => e,
            Ok(_) => panic!("a scanout change must end a rotated session too"),
        };
        assert!(err.to_string().contains("(32x64 -> 64x32)"), "{err}");
    }

    fn zero_frame_streak_of(c: &IpcDrmCapturer) -> u32 {
        let key = c.connector.clone().expect("this check needs an identity");
        DRM_DISPLAY_HEALTH
            .lock()
            .unwrap()
            .get(&key)
            .map(|h| h.zero_frame_streak)
            .unwrap_or(0)
    }

    fn put_frame(c: &IpcDrmCapturer, w: usize, h: usize) {
        put_frame_rot(c, w, h, None);
    }

    fn put_frame_rot(c: &IpcDrmCapturer, w: usize, h: usize, plane_rotation: Option<u32>) {
        static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let mut buf = c.shared.slot.lock().unwrap().take_free().unwrap_or_default();
        buf.clear();
        buf.resize(w * h * 4, 0);
        buf[..4].copy_from_slice(&SEQ.fetch_add(1, Ordering::Relaxed).to_le_bytes());
        let mut slot = c.shared.slot.lock().unwrap();
        slot.publish(w, h, Pixfmt::BGRA, plane_rotation, buf);
        c.shared.cv.notify_one();
    }

    fn put_frame_with(
        c: &IpcDrmCapturer,
        w: usize,
        h: usize,
        plane_rotation: Option<u32>,
        pixels: &[u8],
    ) {
        let mut buf = c.shared.slot.lock().unwrap().take_free().unwrap_or_default();
        buf.clear();
        buf.extend_from_slice(pixels);
        assert_eq!(buf.len(), w * h * 4);
        let mut slot = c.shared.slot.lock().unwrap();
        slot.publish(w, h, Pixfmt::BGRA, plane_rotation, buf);
        c.shared.cv.notify_one();
    }

    #[test]
    fn a_smaller_frame_ends_the_session_instead_of_being_encoded() {
        let mut c = capturer_named(Some((1920, 1080)), Some("test:mid-session-shrink"));
        put_frame(&c, 1920, 1080);
        assert!(matches!(c.frame(Duration::from_millis(50)), Ok(_)));
        put_frame(&c, 1280, 720);
        let err = match c.frame(Duration::from_millis(50)) {
            Err(e) => e,
            Ok(_) => panic!("a mid-session shrink must be a hard error, not a delivered frame"),
        };
        assert!(err.to_string().contains("changed geometry mid-session"));
        assert!(
            c.got_frame,
            "the rebuild must not look like a display that never produced a frame"
        );
        assert_eq!(
            zero_frame_streak_of(&c),
            0,
            "a session that streamed must not be counted as one that produced nothing"
        );
    }

    static CACHE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    const CAL_GEN: u64 = 7;
    fn ctx1() -> CalContext {
        CalContext {
            rect: (0, 0, 1920, 1080),
            physical_size: (1920, 1080),
            built_gen: CAL_GEN,
        }
    }
    fn peer(pos: Pos, seq: u64) -> crate::server::input_service::PeerAbsSample {
        peer_under(pos, seq, CAL_GEN, 0)
    }
    fn peer_under(
        pos: Pos,
        seq: u64,
        gen: u64,
        map_epoch: u64,
    ) -> crate::server::input_service::PeerAbsSample {
        crate::server::input_service::PeerAbsSample {
            pos,
            age_ms: 400,
            seq,
            gen,
            map_epoch,
        }
    }
    fn arrive(display: i32, epoch: u64, shape: u64, wire: Hotspot) -> Option<CursorCal> {
        on_cursor_shape(
            display,
            epoch,
            Some(ctx1()),
            shape,
            64,
            64,
            wire.0,
            wire.1,
            false,
            vec![0; 64 * 64 * 4],
            0,
        )
    }
    fn settle(
        cal: &mut Option<CursorCal>,
        plane: Pos,
        sample: Option<crate::server::input_service::PeerAbsSample>,
        display: i32,
        epoch: u64,
    ) {
        settle_under(cal, plane, sample, display, epoch, (CAL_GEN, CAL_GEN, 0));
    }
    fn settle_under(
        cal: &mut Option<CursorCal>,
        plane: Pos,
        sample: Option<crate::server::input_service::PeerAbsSample>,
        display: i32,
        epoch: u64,
        (gen, map_gen, map_epoch): (u64, u64, u64),
    ) {
        for _ in 0..=CURSOR_CAL_STABLE_TICKS {
            on_cursor_plane(cal, Some(plane), || sample, false, gen, map_gen, map_epoch, display, epoch);
        }
    }
    fn served(display: i32) -> (u64, i32, i32) {
        let map = DRM_CURSOR.lock().unwrap();
        let (_, c) = map.get(&display).expect("a shape was published for this display");
        (c.id, c.hotx, c.hoty)
    }
    fn plain(shape: u64) -> u64 {
        fold_cursor_id(shape, 0)
    }
    fn corrected(shape: u64, h: Hotspot) -> u64 {
        fold_cursor_id(remix_cursor_id(shape, h.0, h.1), 0)
    }

    #[test]
    fn a_confirming_measurement_publishes_the_retained_candidate_not_the_new_sample() {
        let _serial = CACHE_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let (d, e, shape) = (9_700, next_cursor_epoch(), 0x7a00);
        forget_cursor_cal(shape, CAL_GEN);
        let mut cal = arrive(d, e, shape, (4, 4));
        assert_eq!(served(d), (plain(shape), 4, 4));
        settle(&mut cal, (488, 288), Some(peer((500, 300), 1)), d, e);
        assert_eq!(cal.as_ref().unwrap().candidate, Some(((12, 12), 1, 0)));
        assert_eq!(cal.as_ref().unwrap().current_hot, (4, 4));
        assert_eq!(served(d), (plain(shape), 4, 4), "unconfirmed: nothing visible changes");
        assert_eq!(cached_cursor_cal(shape, CAL_GEN), None);
        settle(&mut cal, (587, 388), Some(peer((600, 400), 2)), d, e);
        let c = cal.as_ref().unwrap();
        assert_eq!(c.candidate, None);
        assert_eq!(c.current_hot, (12, 12));
        assert_eq!(cached_cursor_cal(shape, CAL_GEN), Some((12, 12)));
        assert_eq!(served(d), (corrected(shape, (12, 12)), 12, 12));
        forget_cursor_cal(shape, CAL_GEN);
    }

    #[test]
    fn a_calibration_context_exists_only_for_an_identity_matched_upright_output() {
        let one = |t: i32| {
            let mut o = wl_display("HDMI-1", 0, 0, 1920, 1080);
            o.transform = t;
            scrap::wayland::display::Displays {
                primary: 0,
                displays: vec![o],
            }
        };
        let drm = [drm_display("HDMI-A-1", 1920, 1080)];
        assert_eq!(
            calibration_context(&drm, 0, &one(0), 7),
            Ok(CalContext {
                rect: (0, 0, 1920, 1080),
                physical_size: (1920, 1080),
                built_gen: 7
            })
        );
        for t in [90, 180, 270] {
            assert!(calibration_context(&drm, 0, &one(t), 7).is_err(), "transform {t}");
        }
        assert_eq!(frame_transform(transform_and_origin(&drm, 0, &one(180)).0, Some(0x4)), 0);
        let two = [
            drm_display("HDMI-A-1", 1920, 1080),
            drm_display("DP-1", 2560, 1440),
        ];
        assert!(calibration_context(&two, 0, &one(0), 7).is_err());
        let wl = scrap::wayland::display::Displays {
            primary: 0,
            displays: vec![
                wl_display("DP-1", 0, 0, 2560, 1440),
                wl_display("DP-2", 2560, 0, 3840, 2160),
            ],
        };
        assert!(calibration_context(&drm, 0, &wl, 7).is_err());
        assert_eq!(transform_and_origin(&drm, 0, &wl).0, 0);
        let mut a = wl_display("HDMI-1", 0, 0, 1920, 1080);
        a.logical_size = Some((0, 0));
        let mut b = wl_display("DP-1", 1920, 0, 2560, 1440);
        b.logical_size = Some((0, 0));
        let wl = scrap::wayland::display::Displays {
            primary: 0,
            displays: vec![a, b],
        };
        assert!(calibration_context(&two, 0, &wl, 7).is_err());
        let mut r = wl_display("HDMI-1", 0, 0, 1920, 1080);
        r.transform = 90;
        let wl = scrap::wayland::display::Displays {
            primary: 0,
            displays: vec![r, wl_display("DP-1", 1920, 0, 2560, 1440)],
        };
        assert!(calibration_context(&two, 0, &wl, 7).is_err());
        assert_eq!(
            calibration_context(&two, 1, &wl, 7).map(|c| (c.rect, c.physical_size)),
            Ok(((1920, 0, 2560, 1440), (2560, 1440)))
        );
        let panel = [
            drm_display("eDP-1", 2880, 1800),
            drm_display("DP-6", 1920, 1080),
        ];
        let mut p = wl_display("eDP-1", 0, 0, 2880, 1800);
        p.logical_size = Some((1986, 1241));
        let wl = scrap::wayland::display::Displays {
            primary: 0,
            displays: vec![p, wl_display("DP-6", 1986, 0, 1920, 1080)],
        };
        let ctx = calibration_context(&panel, 0, &wl, 7).unwrap();
        assert_eq!(ctx.rect, (0, 0, 1986, 1241));
        assert_eq!(measure_hotspot(&ctx, (1643, 577), (2357, 814), (128, 128)), Some((26, 23)));
    }

    #[test]
    fn the_measurement_is_the_injected_tip_in_scanout_space_minus_the_plane() {
        let ctx = ctx1();
        assert_eq!(measure_hotspot(&ctx, (500, 300), (488, 288), (32, 32)), Some((12, 12)));
        let scaled = CalContext {
            rect: (0, 0, 1986, 1241),
            physical_size: (2880, 1800),
            built_gen: CAL_GEN,
        };
        assert_eq!(measure_hotspot(&scaled, (1643, 577), (2357, 814), (128, 128)), Some((26, 23)));
        let right = CalContext {
            rect: (1920, 0, 1920, 1080),
            ..ctx
        };
        assert_eq!(measure_hotspot(&right, (500, 300), (488, 288), (32, 32)), None);
        assert_eq!(measure_hotspot(&ctx, (900, 300), (488, 288), (32, 32)), None);
        assert_eq!(measure_hotspot(&ctx, (480, 300), (488, 288), (32, 32)), None);
    }

    fn drm_display(name: &str, w: u32, h: u32) -> DrmDisplayInfo {
        DrmDisplayInfo {
            name: name.to_owned(),
            crtc_id: 1,
            x: 0,
            y: 0,
            width: w,
            height: h,
            active: true,
            render_node: String::new(),
            device: String::new(),
            plane_transposed: false,
        }
    }

    fn wl_display(
        name: &str,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    ) -> base::platform::linux::WaylandDisplayInfo {
        base::platform::linux::WaylandDisplayInfo {
            name: name.to_owned(),
            x,
            y,
            width: w,
            height: h,
            logical_size: Some((w, h)),
            refresh_rate: 60,
            transform: 0,
        }
    }

    fn plane_transposed_display(name: &str, w: u32, h: u32) -> DrmDisplayInfo {
        DrmDisplayInfo {
            plane_transposed: true,
            ..drm_display(name, w, h)
        }
    }

    #[test]
    fn a_plane_transposed_display_advertises_its_scanout() {
        let drm = [plane_transposed_display("eDP-1", 800, 1280)];
        let wl = scrap::wayland::display::Displays {
            primary: 0,
            displays: vec![],
        };
        let assignment = assign_wayland_outputs(&drm, &wl.displays);
        let infos = augment_with_wayland_geometry_from(&drm, &wl, &assignment);
        assert_eq!((infos[0].width, infos[0].height), (1280, 800));
    }

    #[test]
    fn a_rotated_output_over_a_rotated_plane_is_swapped_once() {
        let drm = [plane_transposed_display("HDMI-A-1", 1920, 1080)];
        let mut out = wl_display("HDMI-1", 0, 0, 1920, 1080);
        out.transform = 90;
        let wl = scrap::wayland::display::Displays {
            primary: 0,
            displays: vec![out],
        };
        let assignment = assign_wayland_outputs(&drm, &wl.displays);
        let infos = augment_with_wayland_geometry_from(&drm, &wl, &assignment);
        assert_eq!((infos[0].width, infos[0].height), (1080, 1920));
    }

    #[test]
    fn a_lone_rotated_output_advertises_delivered_dimensions() {
        let drm = [drm_display("HDMI-A-1", 1920, 1080)];
        let mut out = wl_display("HDMI-1", 0, 0, 1920, 1080);
        out.transform = 90;
        let wl = scrap::wayland::display::Displays {
            primary: 0,
            displays: vec![out],
        };
        let assignment = assign_wayland_outputs(&drm, &wl.displays);
        let infos = augment_with_wayland_geometry_from(&drm, &wl, &assignment);
        assert_eq!((infos[0].width, infos[0].height), (1080, 1920));
        assert_eq!(infos[0].scale, 1.0);
    }

    #[test]
    fn transform_and_origin_come_from_the_same_snapshot() {
        let drm = [
            drm_display("HDMI-A-1", 1920, 1080),
            drm_display("DP-1", 2560, 1440),
        ];
        let mut rotated = wl_display("DP-1", 1920, 0, 2560, 1440);
        rotated.transform = 270;
        let wl = scrap::wayland::display::Displays {
            primary: 0,
            displays: vec![rotated, wl_display("HDMI-1", 0, 0, 1920, 1080)],
        };
        let (t, origin) = transform_and_origin(&drm, 1, &wl);
        assert_eq!(t, 270);
        assert_eq!(origin, Some((1920, 0)));
        let lone = scrap::wayland::display::Displays {
            primary: 0,
            displays: vec![wl_display("HDMI-1", 0, 0, 1920, 1080)],
        };
        assert_eq!(transform_and_origin(&drm, 1, &lone), (0, None));
    }

    #[test]
    fn one_connector_assignment_drives_geometry_and_primary() {
        let drm = [
            drm_display("HDMI-A-1", 1920, 1080),
            drm_display("DP-1", 2560, 1440),
        ];
        let wl = scrap::wayland::display::Displays {
            primary: 0,
            displays: vec![
                wl_display("DP-1", 1920, 0, 2560, 1440),
                wl_display("HDMI-1", 0, 0, 1920, 1080),
            ],
        };

        let assignment = assign_wayland_outputs(&drm, &wl.displays);
        let infos = augment_with_wayland_geometry_from(&drm, &wl, &assignment);
        assert_eq!((infos[0].x, infos[1].x), (0, 1920));
        assert_eq!(primary_index_from_assignment(&assignment, wl.primary), 1);
    }

    #[test]
    fn a_resolution_guess_never_steals_an_exact_name_match() {
        let drm = vec![
            drm_display("DSI-1", 1920, 1080),
            drm_display("HDMI-A-1", 1920, 1080),
        ];
        let wl = vec![
            wl_display("HDMI-1", 0, 0, 1920, 1080),
            wl_display("Unknown-9", 1920, 0, 2560, 1440),
        ];
        let m = identity_matches(&drm, &wl);
        assert_eq!(m[1], Some(0), "the exact name match must win globally");
        assert_eq!(m[0], None, "the leftover pairing is not forced, so no identity");
        let drm2 = vec![
            drm_display("DSI-1", 1920, 1080),
            drm_display("DSI-2", 1920, 1080),
        ];
        let wl2 = vec![wl_display("HDMI-1", 0, 0, 1920, 1080)];
        let m2 = identity_matches(&drm2, &wl2);
        assert!(m2[0].is_none() && m2[1].is_none());
    }

    #[test]
    fn identical_monitors_that_match_no_name_take_layout_order() {
        let drm = [drm_display("DP-1", 1920, 1080), drm_display("DP-2", 1920, 1080)];
        let wl = [
            wl_display("Unknown-1", 0, 0, 1920, 1080),
            wl_display("Unknown-2", 1920, 0, 1920, 1080),
        ];
        assert_eq!(assign_wayland_outputs(&drm, &wl), vec![Some(0), Some(1)]);
    }

    #[test]
    fn health_reports_demoted_only_while_the_cooldown_runs() {
        let mut h = DisplayHealth::new();
        assert!(!h.demoted(), "a fresh display is not demoted");
        h.zero_frame_streak = DRM_GRAB_MAX_FAILURES - 1;
        assert!(!h.demoted(), "one session short of the threshold is not demoted");
        h.zero_frame_streak = DRM_GRAB_MAX_FAILURES;
        h.demotes = 1;
        assert!(h.demoted(), "at the threshold, inside the cooldown");
        h.since = Instant::now() - demote_cooldown(h.demotes) - Duration::from_secs(1);
        assert!(!h.demoted(), "past the cooldown the display must be retried");
        h.demotes = 4;
        assert!(h.demoted(), "the backoff must still be holding it at demotion 4");
    }


    fn xcursor_images(path: &std::path::Path) -> Vec<(u32, usize, usize, i32, i32, Vec<u8>)> {
        let b = match std::fs::read(path) {
            Ok(b) => b,
            Err(_) => return Vec::new(),
        };
        let u32at = |o: usize| -> u32 {
            if o + 4 > b.len() {
                0
            } else {
                u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
            }
        };
        if b.len() < 16 || &b[0..4] != b"Xcur" {
            return Vec::new();
        }
        let (hdr, ntoc) = (u32at(4) as usize, u32at(12) as usize);
        let mut out: Vec<(u32, usize, usize, i32, i32, Vec<u8>)> = Vec::new();
        for i in 0..ntoc {
            let e = hdr + i * 12;
            if u32at(e) != 0xfffd_0002 {
                continue;
            }
            let nominal = u32at(e + 4);
            if out.iter().any(|(n, ..)| *n == nominal) {
                continue;
            }
            let p = u32at(e + 8) as usize;
            let (w, h) = (u32at(p + 16) as usize, u32at(p + 20) as usize);
            let (xhot, yhot) = (u32at(p + 24) as i32, u32at(p + 28) as i32);
            let px = p + 36;
            if w == 0 || h == 0 || px.saturating_add(w * h * 4) > b.len() {
                continue;
            }
            out.push((nominal, w, h, xhot, yhot, b[px..px + w * h * 4].to_vec()));
        }
        out
    }

    fn dist(a: (i32, i32), b: (i32, i32)) -> f64 {
        let (dx, dy) = ((a.0 - b.0) as f64, (a.1 - b.1) as f64);
        (dx * dx + dy * dy).sqrt()
    }

    #[test]
    #[ignore]
    fn adwaita_hotspot_survey() {
        let dir = std::env::var("SURVEY_DIR")
            .unwrap_or_else(|_| "/usr/share/icons/Adwaita/cursors".to_owned());
        let dir = std::path::Path::new(&dir);
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .expect("no installed Adwaita cursors to measure")
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        let want: u32 = std::env::var("SURVEY_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(24);
        let mut display = 20_000;
        println!("SURVEY size={want}");
        println!("shape,size,w,h,truth_x,truth_y,box,t,sent_x,sent_y,base_x,base_y,base_err,head_x,head_y,head_err,master_x,master_y,master_err");
        for name in &names {
            let imgs = xcursor_images(&dir.join(name));
            let Some((n, w, h, tx, ty, up)) = imgs.into_iter().find(|(n, ..)| *n == want) else {
                println!("# {name}: no {want} px image");
                continue;
            };
            let (mut minx, mut miny, mut maxx, mut maxy) = (w as i32, h as i32, -1i32, -1i32);
            for (i, px) in up.chunks_exact(4).take(w * h).enumerate() {
                if px[3] >= 128 {
                    let (x, y) = ((i % w) as i32, (i / w) as i32);
                    minx = minx.min(x);
                    maxx = maxx.max(x);
                    miny = miny.min(y);
                    maxy = maxy.max(y);
                }
            }
            let bx = format!("{}x{}+{}+{}", maxx - minx + 1, maxy - miny + 1, minx, miny);
            for t in [0, 90, 180, 270] {
                let (scan, sw, sh) = as_scanned_out(&up, w, h, t);
                let sent = scrap::drm_reader::infer_hotspot(&scan, sw, sh);
                let base = unrotate_hotspot(t, sw as i32, sh as i32, sent.0, sent.1);
                display += 1;
                deliver_drm_cursor(display, 1, 1, sw as u32, sh as u32, sent.0, sent.1, false, scan, t);
                let head = {
                    let map = DRM_CURSOR.lock().unwrap();
                    let (_, c) = map.get(&display).expect("nothing published");
                    (c.hotx, c.hoty)
                };
                let m = master_delivered(&up, w, h, t);
                println!(
                    "{name},{n},{w},{h},{tx},{ty},{bx},{t},{},{},{},{},{:.2},{},{},{:.2},{},{},{:.2}",
                    sent.0, sent.1, base.0, base.1, dist(base, (tx, ty)),
                    head.0, head.1, dist(head, (tx, ty)),
                    m.0, m.1, dist(m, (tx, ty))
                );
            }
        }
    }


    include!("drm_cursor_theme.rs");

    fn theme_sprite(mask_hex: &str, w: usize, h: usize) -> Vec<u8> {
        let bytes: Vec<u8> = (0..mask_hex.len() / 2)
            .map(|i| u8::from_str_radix(&mask_hex[i * 2..i * 2 + 2], 16).expect("fixture hex"))
            .collect();
        let mut px = vec![0u8; w * h * 4];
        for i in 0..w * h {
            if bytes[i / 8] & (0x80 >> (i % 8)) != 0 {
                px[i * 4 + 3] = 255;
            }
        }
        px
    }

    fn master_guess(rgba: &[u8], w: usize, h: usize) -> (i32, i32) {
        let (mut minx, mut miny, mut maxx, mut maxy) = (w as i32, h as i32, -1i32, -1i32);
        for (i, px) in rgba.chunks_exact(4).take(w * h).enumerate() {
            if px[3] >= 128 {
                let (x, y) = ((i % w) as i32, (i / w) as i32);
                minx = minx.min(x);
                maxx = maxx.max(x);
                miny = miny.min(y);
                maxy = maxy.max(y);
            }
        }
        if maxx < minx || maxy < miny {
            return (0, 0);
        }
        let (bw, bh) = (maxx - minx + 1, maxy - miny + 1);
        if bh > bw * 2 {
            ((minx + maxx) / 2, (miny + maxy) / 2)
        } else {
            (minx, miny)
        }
    }

    fn master_delivered(up: &[u8], w: usize, h: usize, t: i32) -> (i32, i32) {
        let (scan, sw, sh) = as_scanned_out(up, w, h, t);
        let (gx, gy) = master_guess(&scan, sw, sh);
        if t == 90 || t == 270 {
            unrotate_hotspot(t, sw as i32, sh as i32, gx, gy)
        } else {
            (gx, gy)
        }
    }

    fn away(a: (i32, i32), b: (i32, i32)) -> f64 {
        (((a.0 - b.0) as f64).powi(2) + ((a.1 - b.1) as f64).powi(2)).sqrt()
    }

    #[test]
    fn no_shape_lands_further_from_the_theme_than_master_except_the_two_no_bitmap_can_reach() {
        const NOT_INFERABLE: [&str; 2] = ["help", "alias"];
        let (mut sum_master, mut sum_now) = (0f64, 0f64);
        let mut regressed = Vec::new();
        for &(name, w, h, hx, hy, mask) in THEME_SHAPES {
            let (w, h) = (w as usize, h as usize);
            let up = theme_sprite(mask, w, h);
            let truth = (hx, hy);
            let angles = [0, 90, 270];
            let mut e_master = 0f64;
            let mut e_now = 0f64;
            for t in angles {
                e_master += away(master_delivered(&up, w, h, t), truth);
                e_now += away(scrap::drm_reader::infer_hotspot(&up, w, h), truth);
            }
            let (e_master, e_now) = (e_master / angles.len() as f64, e_now / angles.len() as f64);
            sum_master += e_master;
            sum_now += e_now;
            if e_now > e_master + 0.005 && !NOT_INFERABLE.contains(&name) {
                regressed.push((name, e_master, e_now));
            }
        }
        let n = THEME_SHAPES.len() as f64;
        assert!(
            regressed.is_empty(),
            "further from the theme hotspot than master, averaged over 0/90/270: {regressed:?}"
        );
        assert!(
            sum_now / n <= 4.5 && sum_master / n >= 11.0,
            "mean error over {n} shapes: master {:.2}, this pr {:.2} (measured 12.08 and 4.20)",
            sum_master / n,
            sum_now / n
        );
    }
}
