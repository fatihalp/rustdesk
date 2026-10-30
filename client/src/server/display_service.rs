use super::*;
use crate::common::SimpleCallOnReturn;
#[cfg(target_os = "linux")]
use crate::platform::linux::is_x11;
#[cfg(windows)]
use crate::virtual_display_manager;
#[cfg(windows)]
use hbb_common::get_version_number;
use hbb_common::protobuf::MessageField;
use scrap::Display;
use std::sync::atomic::{AtomicBool, Ordering};


pub const NAME: &'static str = "display";

#[cfg(windows)]
const DUMMY_DISPLAY_SIDE_MAX_SIZE: usize = 1024;

struct ChangedResolution {
    original: (i32, i32),
    changed: (i32, i32),
}

lazy_static::lazy_static! {
    static ref IS_CAPTURER_MAGNIFIER_SUPPORTED: bool = is_capturer_mag_supported();
    static ref CHANGED_RESOLUTIONS: Arc<RwLock<HashMap<String, ChangedResolution>>> = Default::default();
    static ref SYNC_DISPLAYS: Arc<Mutex<SyncDisplaysInfo>> = Default::default();
}

#[cfg(target_os = "linux")]
lazy_static::lazy_static! {
    static ref WAYLAND_UINPUT_RECT: Mutex<WaylandUinputRect> = Default::default();
    static ref WAYLAND_LAYOUT: Mutex<WaylandLayout> = Default::default();
}

#[cfg(target_os = "linux")]
const WAYLAND_LAYOUT_CHECK_INTERVAL: Duration = Duration::from_millis(1500);

#[cfg(target_os = "linux")]
#[derive(Default)]
struct WaylandUinputRect {
    rect: Option<(i32, i32, i32, i32)>,
    last_check: Option<std::time::Instant>,
}

#[cfg(target_os = "linux")]
#[derive(Default)]
struct WaylandLayout {
    baseline: Vec<scrap::wayland::display::DisplayRect>,
    live: Vec<scrap::wayland::display::DisplayRect>,
    seen: Vec<scrap::wayland::display::DisplayRect>,
    unseen_build: Option<u64>,
}

#[cfg(target_os = "linux")]
impl WaylandLayout {
    fn reset_baseline(&mut self, baseline: Vec<scrap::wayland::display::DisplayRect>) {
        if self.seen.is_empty() {
            let previous = std::mem::take(&mut self.baseline);
            self.seen = previous;
        }
        self.baseline = baseline;
        self.live.clear();
    }

    fn edge(
        &self,
        live: &[scrap::wayland::display::DisplayRect],
        snapshot_missing: bool,
        generation: u64,
    ) -> bool {
        if self.unseen_build == Some(generation) {
            return true;
        }
        if !self.seen.is_empty() {
            return self.seen != live;
        }
        if self.baseline.is_empty() {
            return snapshot_missing;
        }
        self.baseline != live
    }

    fn observe(&mut self, live: &[scrap::wayland::display::DisplayRect]) {
        self.live = live.to_vec();
        self.seen = live.to_vec();
        self.unseen_build = None;
    }

    fn note_capturer(&mut self, built_on: &[scrap::wayland::display::DisplayRect], built_gen: u64) {
        if built_on.is_empty() {
            return;
        }
        if self.seen.is_empty() {
            self.seen = built_on.to_vec();
        } else if self.seen != built_on {
            self.unseen_build = Some(self.unseen_build.map_or(built_gen, |g| g.max(built_gen)));
        }
    }
}

#[cfg(target_os = "linux")]
static WAYLAND_LAYOUT_DRIFTED: AtomicBool = AtomicBool::new(false);

#[cfg(all(target_os = "linux", feature = "drm"))]
pub(crate) fn wayland_layout_drifted() -> bool {
    WAYLAND_LAYOUT_DRIFTED.load(Ordering::Relaxed)
}

#[cfg(target_os = "linux")]
pub(super) fn set_wayland_uinput_rect(rect: (i32, i32, i32, i32)) {
    WAYLAND_UINPUT_RECT.lock().unwrap().rect = Some(rect);
}

#[cfg(all(target_os = "linux", feature = "drm"))]
static INPUT_MAP_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(u64::MAX);
#[cfg(all(target_os = "linux", feature = "drm"))]
static INPUT_MAP_EPOCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[cfg(all(target_os = "linux", feature = "drm"))]
pub(crate) fn input_map_gen() -> u64 {
    INPUT_MAP_GEN.load(Ordering::Acquire)
}
#[cfg(all(target_os = "linux", feature = "drm"))]
pub(crate) fn input_map_epoch() -> u64 {
    INPUT_MAP_EPOCH.load(Ordering::Relaxed)
}
#[cfg(all(target_os = "linux", feature = "drm"))]
pub(super) fn note_input_map_adopted(gen: u64) {
    INPUT_MAP_EPOCH.fetch_add(1, Ordering::Relaxed);
    INPUT_MAP_GEN.store(gen, Ordering::Release);
}
#[cfg(all(target_os = "linux", feature = "drm"))]
pub(super) fn note_input_map_ready(gen: u64) {
    INPUT_MAP_GEN.store(gen, Ordering::Release);
}
#[cfg(all(target_os = "linux", feature = "drm"))]
pub(super) fn note_input_map_unknown() {
    WAYLAND_UINPUT_RECT.lock().unwrap().rect = None;
    note_input_map_adopted(u64::MAX);
}
#[cfg(all(target_os = "linux", feature = "drm"))]
pub(super) const UINPUT_APPLY_LOG_INTERVAL: Duration = Duration::from_secs(600);
#[cfg(all(target_os = "linux", feature = "drm"))]
pub(super) fn run_uinput_apply<R: Send + 'static>(
    job: impl FnOnce(&tokio::runtime::Runtime) -> R + Send + 'static,
) -> tokio::sync::oneshot::Receiver<R> {
    type Job = Box<dyn FnOnce(&tokio::runtime::Runtime) + Send>;
    static JOBS: Mutex<Option<std::sync::mpsc::Sender<Job>>> = Mutex::new(None);
    fn start() -> Option<std::sync::mpsc::Sender<Job>> {
        let (tx, rx) = std::sync::mpsc::channel::<Job>();
        let spawned = std::thread::Builder::new()
            .name("uinput-apply".to_owned())
            .spawn(move || {
                let rt = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(err) => {
                        hbb_common::throttled_log!(
                            UINPUT_APPLY_LOG_INTERVAL,
                            error,
                            "uinput apply thread: failed to build a runtime: {err}"
                        );
                        return;
                    }
                };
                for job in rx {
                    job(&rt);
                }
            });
        match spawned {
            Ok(_) => Some(tx),
            Err(err) => {
                hbb_common::throttled_log!(
                    UINPUT_APPLY_LOG_INTERVAL,
                    error,
                    "failed to start the uinput apply thread: {err}"
                );
                None
            }
        }
    }
    let (tx, rx) = tokio::sync::oneshot::channel();
    let job: Job = Box::new(move |rt| {
        let _ = tx.send(job(rt));
    });
    let mut jobs = JOBS.lock().unwrap();
    let job = match jobs.as_ref() {
        Some(sender) => match sender.send(job) {
            Ok(()) => return rx,
            Err(std::sync::mpsc::SendError(job)) => job,
        },
        None => job,
    };
    *jobs = start();
    if let Some(tx) = jobs.as_ref() {
        let _ = tx.send(job);
    }
    rx
}
#[cfg(all(target_os = "linux", feature = "drm"))]
pub(super) fn input_map_label(gen: u64, from_compositor: bool) -> u64 {
    if from_compositor {
        gen
    } else {
        u64::MAX
    }
}

#[cfg(all(target_os = "linux", feature = "drm"))]
pub(super) fn wayland_uinput_rect() -> Option<(i32, i32, i32, i32)> {
    WAYLAND_UINPUT_RECT.lock().unwrap().rect
}

#[cfg(target_os = "linux")]
pub(super) fn set_wayland_layout_baseline(baseline: Vec<scrap::wayland::display::DisplayRect>) {
    WAYLAND_LAYOUT_DRIFTED.store(false, Ordering::Relaxed);
    WAYLAND_LAYOUT.lock().unwrap().reset_baseline(baseline);
}

#[cfg(all(target_os = "linux", feature = "drm"))]
pub(super) fn note_capturer_layout(
    displays: &[base::platform::linux::WaylandDisplayInfo],
    built_gen: u64,
) {
    if displays.is_empty() {
        return;
    }
    let rects = scrap::wayland::display::logical_rects_of_displays(displays);
    WAYLAND_LAYOUT
        .lock()
        .unwrap()
        .note_capturer(&rects, built_gen);
}

#[cfg(target_os = "linux")]
pub(super) fn remap_wayland_uinput_coord(x: i32, y: i32) -> (i32, i32) {
    if !WAYLAND_LAYOUT_DRIFTED.load(Ordering::Relaxed) {
        return (x, y);
    }
    let lock = WAYLAND_LAYOUT.lock().unwrap();
    scrap::wayland::display::remap_to_live_layout(x, y, &lock.baseline, &lock.live)
}

#[cfg(target_os = "linux")]
fn refresh_wayland_uinput_rect_if_changed() {
    if is_x11() || !crate::input_service::wayland_use_uinput() {
        return;
    }
    {
        let mut lock = WAYLAND_UINPUT_RECT.lock().unwrap();
        if let Some(last_check) = lock.last_check {
            if last_check.elapsed() < WAYLAND_LAYOUT_CHECK_INTERVAL {
                return;
            }
        }
        lock.last_check = Some(std::time::Instant::now());
    }
    let Some((rect, live_rects)) = scrap::wayland::display::get_layout_for_uinput_live() else {
        return;
    };
    let (live_changed, mut drifted) = {
        let mut layout = WAYLAND_LAYOUT.lock().unwrap();
        #[cfg(feature = "drm")]
        let snapshot_missing = scrap::wayland::display::wayland_snapshot_missing();
        #[cfg(not(feature = "drm"))]
        let snapshot_missing = false;
        #[cfg(feature = "drm")]
        let generation = scrap::wayland::display::wayland_snapshot_generation();
        #[cfg(not(feature = "drm"))]
        let generation = 0;
        let live_changed = layout.edge(&live_rects, snapshot_missing, generation);
        let drifted = !layout.baseline.is_empty()
            && !live_rects.is_empty()
            && layout.baseline != live_rects;
        layout.observe(&live_rects);
        (live_changed, drifted)
    };
    #[cfg(feature = "drm")]
    {
        static PROMOTION_OWED: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);
        let blind_build = super::drm_capturer::take_unrotated_snapshot_pending();
        if live_changed || blind_build {
            PROMOTION_OWED.store(true, Ordering::Release);
        }
        if PROMOTION_OWED.load(Ordering::Acquire) && super::drm_capturer::is_available_cached() {
            PROMOTION_OWED.store(false, Ordering::Release);
            scrap::wayland::display::clear_wayland_displays_cache();
            scrap::wayland::display::bump_layout_generation();
            set_wayland_layout_baseline(live_rects.clone());
            WAYLAND_LAYOUT.lock().unwrap().live = live_rects.clone();
            drifted = false;
        }
    }
    #[cfg(not(feature = "drm"))]
    let _ = live_changed;
    #[cfg(feature = "drm")]
    if run_uinput_apply(move |rt| {
        let mut range_ok = WAYLAND_UINPUT_RECT.lock().unwrap().rect == Some(rect);
        if range_ok {
            note_input_map_ready(scrap::wayland::display::wayland_snapshot_generation());
        }
        if crate::platform::linux::is_login_screen_wayland_cached() {
            return;
        }
        if !range_ok {
            let (minx, maxx, miny, maxy) = rect;
            log::info!(
                "desktop layout changed, update mouse resolution: ({}, {}), ({}, {})",
                minx,
                maxx,
                miny,
                maxy
            );
            note_input_map_unknown();
            match rt.block_on(async {
                timeout(
                    3_000,
                    crate::input_service::update_mouse_resolution(minx, maxx, miny, maxy),
                )
                .await
            }) {
                Ok(Ok(())) => {
                    WAYLAND_UINPUT_RECT.lock().unwrap().rect = Some(rect);
                    note_input_map_adopted(scrap::wayland::display::wayland_snapshot_generation());
                    range_ok = true;
                }
                Ok(Err(err)) => log::error!("Failed to update mouse resolution: {}", err),
                Err(err) => log::error!("Failed to update mouse resolution: {}", err),
            }
        }
        WAYLAND_LAYOUT_DRIFTED.store(drifted && range_ok, Ordering::Relaxed);
    })
    .blocking_recv()
    .is_err()
    {
        hbb_common::throttled_log!(
            UINPUT_APPLY_LOG_INTERVAL,
            error,
            "Failed to check the uinput range: the uinput apply thread is gone"
        );
    }
    #[cfg(not(feature = "drm"))]
    {
        let mut range_ok = WAYLAND_UINPUT_RECT.lock().unwrap().rect == Some(rect);
        if !range_ok {
            let (minx, maxx, miny, maxy) = rect;
            log::info!(
                "desktop layout changed, update mouse resolution: ({}, {}), ({}, {})",
                minx,
                maxx,
                miny,
                maxy
            );
            match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => {
                    match rt.block_on(async {
                        timeout(
                            3_000,
                            crate::input_service::update_mouse_resolution(minx, maxx, miny, maxy),
                        )
                        .await
                    }) {
                        Ok(Ok(())) => {
                            WAYLAND_UINPUT_RECT.lock().unwrap().rect = Some(rect);
                            range_ok = true;
                        }
                        Ok(Err(err)) => log::error!("Failed to update mouse resolution: {}", err),
                        Err(err) => log::error!("Failed to update mouse resolution: {}", err),
                    }
                }
                Err(err) => {
                    log::error!("Failed to build tokio runtime: {}", err);
                }
            }
        }
        WAYLAND_LAYOUT_DRIFTED.store(drifted && range_ok, Ordering::Relaxed);
    }
}

static TEMP_IGNORE_DISPLAYS_CHANGED: AtomicBool = AtomicBool::new(false);

#[derive(Default)]
struct SyncDisplaysInfo {
    displays: Vec<DisplayInfo>,
    is_synced: bool,
}

impl SyncDisplaysInfo {
    fn check_changed(&mut self, displays: &[DisplayInfo]) {
        if self.displays.as_slice() == displays {
            return;
        }

        self.displays = displays.to_vec();
        if !TEMP_IGNORE_DISPLAYS_CHANGED.load(Ordering::Relaxed) {
            self.is_synced = false;
        }
    }

    fn get_update_sync_displays(&mut self) -> Option<Vec<DisplayInfo>> {
        if self.is_synced {
            return None;
        }
        self.is_synced = true;
        Some(self.displays.clone())
    }
}

pub fn temp_ignore_displays_changed() -> SimpleCallOnReturn {
    TEMP_IGNORE_DISPLAYS_CHANGED.store(true, std::sync::atomic::Ordering::Relaxed);
    SimpleCallOnReturn {
        b: true,
        f: Box::new(move || {
            std::thread::sleep(Duration::from_millis(1000));
            TEMP_IGNORE_DISPLAYS_CHANGED.store(false, Ordering::Relaxed);
            SYNC_DISPLAYS.lock().unwrap().is_synced = false;
        }),
    }
}

pub(super) fn check_display_changed(
    ndisplay: usize,
    idx: usize,
    (x, y, w, h): (i32, i32, usize, usize),
) -> Option<DisplayInfo> {
    #[cfg(target_os = "linux")]
    {
        if !is_x11() {
            return None;
        }
    }

    let lock = SYNC_DISPLAYS.lock().unwrap();
    let d = lock.displays.get(idx)?;
    if ndisplay != lock.displays.len() {
        return Some(d.clone());
    }
    if !(d.x == x && d.y == y && d.width == w as i32 && d.height == h as i32) {
        Some(d.clone())
    } else {
        None
    }
}

#[inline]
pub fn set_last_changed_resolution(display_name: &str, original: (i32, i32), changed: (i32, i32)) {
    let mut lock = CHANGED_RESOLUTIONS.write().unwrap();
    match lock.get_mut(display_name) {
        Some(res) => res.changed = changed,
        None => {
            lock.insert(
                display_name.to_owned(),
                ChangedResolution { original, changed },
            );
        }
    }
}

#[inline]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn restore_resolutions() {
    for (name, res) in CHANGED_RESOLUTIONS.read().unwrap().iter() {
        let (w, h) = res.original;
        log::info!("Restore resolution of display '{}' to ({}, {})", name, w, h);
        if let Err(e) = crate::platform::change_resolution(name, w as _, h as _) {
            log::error!(
                "Failed to restore resolution of display '{}' to ({},{}): {}",
                name,
                w,
                h,
                e
            );
        }
    }
    CHANGED_RESOLUTIONS.write().unwrap().clear();
}

#[inline]
fn is_capturer_mag_supported() -> bool {
    #[cfg(windows)]
    return scrap::CapturerMag::is_supported();
    #[cfg(not(windows))]
    false
}

#[inline]
pub fn capture_cursor_embedded() -> bool {
    scrap::is_cursor_embedded()
}

#[inline]
#[cfg(windows)]
pub fn is_privacy_mode_mag_supported() -> bool {
    return *IS_CAPTURER_MAGNIFIER_SUPPORTED
        && get_version_number(&crate::VERSION) > get_version_number("1.1.9");
}

pub fn new() -> GenericService {
    let svc = EmptyExtraFieldService::new(NAME.to_owned(), true);
    GenericService::run(&svc.clone(), run);
    svc.sp
}

fn displays_to_msg(displays: Vec<DisplayInfo>) -> Message {
    let mut pi = PeerInfo {
        ..Default::default()
    };
    pi.displays = displays.clone();

    #[cfg(windows)]
    if crate::platform::is_installed() {
        let m = crate::virtual_display_manager::get_platform_additions();
        pi.platform_additions = serde_json::to_string(&m).unwrap_or_default();
    }

    pi.current_display = 0;
    let mut msg_out = Message::new();
    msg_out.set_peer_info(pi);
    msg_out
}

fn check_get_displays_changed_msg() -> Option<Message> {
    #[cfg(target_os = "linux")]
    {
        if !is_x11() {
            #[cfg(feature = "drm")]
            if super::drm_capturer::is_available_cached() {
                let synced = !SYNC_DISPLAYS.lock().unwrap().displays.is_empty();
                let stamped_before = scrap::wayland::display::wayland_failure_stamped();
                if !synced || !scrap::wayland::display::wayland_lookup_suppressed() {
                    if let Some(displays) = super::drm_capturer::get_display_infos() {
                        if !synced
                            || stamped_before
                            || !scrap::wayland::display::wayland_lookup_suppressed()
                        {
                            SYNC_DISPLAYS.lock().unwrap().check_changed(&displays);
                        }
                    }
                }
            }
            return get_displays_msg();
        }
    }
    check_update_displays(&try_get_displays().ok()?);
    get_displays_msg()
}

pub fn check_displays_changed() -> ResultType<()> {
    #[cfg(target_os = "linux")]
    {
        if !is_x11() {
            return Ok(());
        }
    }
    check_update_displays(&try_get_displays()?);
    Ok(())
}

fn get_displays_msg() -> Option<Message> {
    let displays = SYNC_DISPLAYS.lock().unwrap().get_update_sync_displays()?;
    Some(displays_to_msg(displays))
}

fn run(sp: EmptyExtraFieldService) -> ResultType<()> {
    while sp.ok() {
        sp.snapshot(|sps| {
            if !TEMP_IGNORE_DISPLAYS_CHANGED.load(Ordering::Relaxed) {
                if sps.has_subscribes() {
                    SYNC_DISPLAYS.lock().unwrap().is_synced = false;
                    bail!("new subscriber");
                }
            }
            Ok(())
        })?;

        if let Some(msg_out) = check_get_displays_changed_msg() {
            sp.send(msg_out);
            log::info!("Displays changed");
        }

        #[cfg(target_os = "linux")]
        if sp.has_subscribes() {
            refresh_wayland_uinput_rect_if_changed();
        }

        std::thread::sleep(Duration::from_millis(300));
    }

    Ok(())
}

#[inline]
pub(super) fn get_original_resolution(
    display_name: &str,
    w: usize,
    h: usize,
) -> MessageField<Resolution> {
    #[cfg(windows)]
    let is_supportdesk_virtual_display =
        crate::virtual_display_manager::supportdesk_idd::is_virtual_display(&display_name);
    #[cfg(not(windows))]
    let is_supportdesk_virtual_display = false;
    Some(if is_supportdesk_virtual_display {
        Resolution {
            width: 0,
            height: 0,
            ..Default::default()
        }
    } else {
        let changed_resolutions = CHANGED_RESOLUTIONS.write().unwrap();
        let (width, height) = match changed_resolutions.get(display_name) {
            Some(res) => {
                res.original
            }
            None => (w as _, h as _),
        };
        Resolution {
            width,
            height,
            ..Default::default()
        }
    })
    .into()
}

pub(super) fn get_sync_displays() -> Vec<DisplayInfo> {
    SYNC_DISPLAYS.lock().unwrap().displays.clone()
}

pub(super) fn get_display_info(idx: usize) -> Option<DisplayInfo> {
    SYNC_DISPLAYS.lock().unwrap().displays.get(idx).cloned()
}

#[cfg(all(target_os = "linux", feature = "drm"))]
pub fn has_non_drm_backed_display() -> bool {
    match super::drm_capturer::display_count_and_any_demoted() {
        Some((count, any_demoted)) => {
            count < SYNC_DISPLAYS.lock().unwrap().displays.len() || any_demoted
        }
        None => false,
    }
}

pub(super) fn check_update_displays(all: &Vec<Display>) {
    let _ = update_sync_displays(all);
}

#[inline]
#[cfg(target_os = "linux")]
fn wayland_has_compositor() -> bool {
    #[cfg(feature = "drm")]
    {
        !crate::platform::linux::is_login_screen_wayland_cached()
    }
    #[cfg(not(feature = "drm"))]
    {
        true
    }
}

pub(super) fn update_sync_displays(all: &Vec<Display>) -> Vec<DisplayInfo> {
    #[cfg(target_os = "linux")]
    let use_logical_scale = !is_x11()
        && crate::is_server()
        && wayland_has_compositor()
        && scrap::wayland::display::get_displays().displays.len() > 1;
    let displays = all
        .iter()
        .map(|d| {
            let display_name = d.name();
            #[allow(unused_assignments)]
            #[allow(unused_mut)]
            let mut scale = 1.0;
            #[cfg(target_os = "macos")]
            {
                scale = d.scale();
            }
            #[cfg(target_os = "linux")]
            {
                if use_logical_scale {
                    scale = d.scale();
                }
            }
            let original_resolution = get_original_resolution(
                &display_name,
                ((d.width() as f64) / scale).round() as usize,
                (d.height() as f64 / scale).round() as usize,
            );
            DisplayInfo {
                x: d.origin().0 as _,
                y: d.origin().1 as _,
                width: d.width() as _,
                height: d.height() as _,
                name: display_name,
                online: d.is_online(),
                cursor_embedded: false,
                original_resolution,
                scale,
                ..Default::default()
            }
        })
        .collect::<Vec<DisplayInfo>>();
    SYNC_DISPLAYS.lock().unwrap().check_changed(&displays);
    displays
}

pub fn is_inited_msg() -> Option<Message> {
    #[cfg(target_os = "linux")]
    if !is_x11() {
        return super::wayland::is_inited();
    }
    None
}

pub async fn update_get_sync_displays_on_login() -> ResultType<(Vec<DisplayInfo>, usize)> {
    #[cfg(target_os = "linux")]
    {
        if !is_x11() {
            let (displays, primary_display_idx) =
                super::wayland::get_displays_and_primary().await?;
            let primary_display_idx =
                normalize_primary_display_idx(primary_display_idx, displays.len());
            return Ok((displays, primary_display_idx));
        }
    }
    #[cfg(not(windows))]
    let displays = display_service::try_get_displays();
    #[cfg(windows)]
    let displays = display_service::try_get_displays_add_amyuni_headless();
    let displays = displays?;
    let primary_display_idx = get_primary_2(&displays);
    let sync_displays = update_sync_displays(&displays);
    let primary_display_idx =
        normalize_primary_display_idx(primary_display_idx, sync_displays.len());
    Ok((sync_displays, primary_display_idx))
}

#[inline]
fn normalize_primary_display_idx(primary_display_idx: usize, display_len: usize) -> usize {
    if primary_display_idx < display_len {
        primary_display_idx
    } else {
        0
    }
}

#[inline]
pub fn get_primary_2(all: &Vec<Display>) -> usize {
    all.iter().position(|d| d.is_primary()).unwrap_or(0)
}

#[inline]
#[cfg(windows)]
fn no_displays(displays: &Vec<Display>) -> bool {
    let display_len = displays.len();
    if display_len == 0 {
        true
    } else if display_len == 1 {
        let display = &displays[0];
        if display.width() > DUMMY_DISPLAY_SIDE_MAX_SIZE
            || display.height() > DUMMY_DISPLAY_SIDE_MAX_SIZE
        {
            return false;
        }
        let any_real = crate::platform::resolutions(&display.name())
            .iter()
            .any(|r| {
                (r.height as usize) > DUMMY_DISPLAY_SIDE_MAX_SIZE
                    || (r.width as usize) > DUMMY_DISPLAY_SIDE_MAX_SIZE
            });
        !any_real
    } else {
        false
    }
}

#[inline]
#[cfg(not(windows))]
pub fn try_get_displays() -> ResultType<Vec<Display>> {
    Ok(Display::all()?)
}

#[inline]
#[cfg(windows)]
pub fn try_get_displays() -> ResultType<Vec<Display>> {
    try_get_displays_(false)
}

#[inline]
#[cfg(windows)]
pub fn try_get_displays_add_amyuni_headless() -> ResultType<Vec<Display>> {
    try_get_displays_(true)
}

#[inline]
#[cfg(windows)]
pub fn try_get_displays_(add_amyuni_headless: bool) -> ResultType<Vec<Display>> {
    let mut displays = Display::all()?;

    if !crate::platform::is_installed() || !virtual_display_manager::is_virtual_display_supported()
    {
        return Ok(displays);
    }

    if virtual_display_manager::is_amyuni_idd() && !add_amyuni_headless {
        return Ok(displays);
    }


    let no_displays_v = no_displays(&displays);
    if no_displays_v {
        log::debug!("no displays, create virtual display");
        if let Err(e) = virtual_display_manager::plug_in_headless() {
            log::error!("plug in headless failed {}", e);
        } else {
            displays = Display::all()?;
        }
    }
    Ok(displays)
}

#[cfg(all(test, target_os = "linux", feature = "drm"))]
mod input_map_tests {
    use super::run_uinput_apply;

    #[test]
    fn uinput_applies_run_one_at_a_time_in_the_order_asked() {
        let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let answers: Vec<_> = (0..4)
            .map(|i| {
                let log = log.clone();
                run_uinput_apply(move |_| {
                    log.lock().unwrap().push(("start", i));
                    std::thread::sleep(std::time::Duration::from_millis(20));
                    log.lock().unwrap().push(("end", i));
                    i * 10
                })
            })
            .collect();
        let got: Vec<i32> = answers
            .into_iter()
            .map(|a| a.blocking_recv().unwrap())
            .collect();
        assert_eq!(got, vec![0, 10, 20, 30]);
        let want: Vec<_> = (0..4).flat_map(|i| [("start", i), ("end", i)]).collect();
        assert_eq!(*log.lock().unwrap(), want, "one job at a time, in order");
    }
}

#[cfg(all(test, target_os = "linux"))]
mod wayland_layout_tests {
    use super::WaylandLayout;
    use scrap::wayland::display::DisplayRect;

    fn layout(w: i32, h: i32, transform: i32) -> Vec<DisplayRect> {
        vec![DisplayRect {
            name: "DP-1".into(),
            x: 0,
            y: 0,
            w,
            h,
            transform,
        }]
    }

    #[test]
    fn a_rotation_between_two_session_inits_is_still_an_edge() {
        let upright = layout(1920, 1080, 0);
        let rotated = layout(1080, 1920, 1);
        let mut l = WaylandLayout::default();
        l.reset_baseline(upright.clone());
        l.observe(&upright);
        l.reset_baseline(upright.clone());
        l.reset_baseline(rotated.clone());
        assert!(l.edge(&rotated, false, 0));
    }

    #[test]
    fn repeated_baseline_resets_without_a_rotation_are_not_an_edge() {
        let upright = layout(1920, 1080, 0);
        let mut l = WaylandLayout::default();
        l.reset_baseline(upright.clone());
        l.observe(&upright);
        l.reset_baseline(upright.clone());
        l.reset_baseline(upright.clone());
        assert!(!l.edge(&upright, false, 0));
    }

    #[test]
    fn a_capturer_built_after_a_failed_init_still_owes_a_rebuild() {
        let upright = layout(1920, 1080, 0);
        let rotated = layout(1080, 1920, 1);

        let mut l = WaylandLayout::default();
        l.reset_baseline(Vec::new());
        l.note_capturer(&upright, 0);
        assert!(l.edge(&rotated, false, 0));

        let mut l2 = WaylandLayout::default();
        l2.reset_baseline(Vec::new());
        l2.note_capturer(&upright, 0);
        l2.reset_baseline(rotated.clone());
        assert!(l2.edge(&rotated, false, 0));

        let mut l3 = WaylandLayout::default();
        l3.reset_baseline(Vec::new());
        l3.note_capturer(&upright, 0);
        assert!(!l3.edge(&upright, false, 0));
    }

    #[test]
    fn a_promoted_layout_is_not_an_edge_again() {
        let rotated = layout(1080, 1920, 1);
        let mut l = WaylandLayout::default();
        l.reset_baseline(layout(1920, 1080, 0));
        l.observe(&rotated);
        l.reset_baseline(rotated.clone());
        assert!(!l.edge(&rotated, false, 0));
    }
}
