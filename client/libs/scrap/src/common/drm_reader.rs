
use super::drmtap_dl::{
    self, drmtap_config, drmtap_ctx, drmtap_cursor_info, drmtap_device, drmtap_display,
    drmtap_dmabuf_desc, drmtap_frame_info, DrmtapLib,
};
use hbb_common::log;
use std::ffi::CString;
use std::io;
use std::os::fd::{FromRawFd, OwnedFd};
use std::os::raw::c_int;

pub(crate) const MAX_DIM: u32 = 16384;
pub(crate) const MAX_FRAME_BYTES: usize = 256 * 1024 * 1024;
pub(crate) const DRM_FORMAT_XRGB8888: u32 = 0x3432_5258;
pub(crate) const DRM_FORMAT_ARGB8888: u32 = 0x3432_5241;
pub(crate) const DRM_FORMAT_XBGR8888: u32 = 0x3432_4258;
pub(crate) const DRM_FORMAT_ABGR8888: u32 = 0x3432_4241;

pub const HIDDEN_CURSOR_ID: u64 = u64::MAX;

pub struct CursorSnapshot {
    pub id: u64,
    pub width: u32,
    pub height: u32,
    pub hotx: i32,
    pub hoty: i32,
    pub hot_measured: bool,
    pub colors: Vec<u8>,
    pub x: i32,
    pub y: i32,
}

fn cursor_id(hash: u64, width: u32, height: u32, hotx: i32, hoty: i32, hot_measured: bool) -> u64 {
    let mut id = hash;
    for v in [
        width as u64,
        height as u64,
        hotx as u32 as u64,
        hoty as u32 as u64,
        hot_measured as u64,
    ] {
        id ^= v;
        id = id.wrapping_mul(1099511628211);
    }
    id
}

fn provenance_code(answered: Option<bool>) -> i8 {
    match answered {
        None => 0,
        Some(false) => 1,
        Some(true) => 2,
    }
}

fn provenance_is_news(last: i8, now: i8) -> bool {
    last != now
}

impl DrmReader {
    fn note_hotspot_provenance(&mut self, answered: Option<bool>, hot_measured: bool) {
        let now = provenance_code(answered);
        let last = std::mem::replace(&mut self.last_provenance, now);
        if !provenance_is_news(last, now) {
            return;
        }
        match answered {
            Some(true) => log::info!(
                "drm: cursor hotspot provenance: the driver published HOTSPOT_X/Y, using its value"
            ),
            Some(false) => log::info!(
                "drm: cursor hotspot provenance: the plane exposes no HOTSPOT_X/Y, inferring the \
                 hotspot from the bitmap"
            ),
            None => log::info!(
                "drm: cursor hotspot provenance: unavailable (libdrmtap older than 0.5.6, or a \
                 pre-0.5.6 privileged helper); falling back to the coordinate heuristic, which says \
                 hot_measured={hot_measured}"
            ),
        }
    }
}

fn hot_measured_from(answered: Option<bool>, hot_x: i32, hot_y: i32) -> bool {
    match answered {
        Some(measured) => measured,
        None => hot_x != 0 || hot_y != 0,
    }
}

const MIRROR_AGREEMENT_PERCENT: u64 = 88;

pub fn infer_hotspot(rgba: &[u8], w: usize, h: usize) -> (i32, i32) {
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
    let (cx, cy) = ((minx + maxx) / 2, (miny + maxy) / 2);
    if bh > bw * 2 || bw > bh * 2 {
        return (cx, cy);
    }
    let opaque = |x: i32, y: i32| {
        rgba.get((y as usize * w + x as usize) * 4 + 3)
            .is_some_and(|&a| a >= 128)
    };
    let agrees = |i: u64, u: u64| u == 0 || i * 100 >= u * MIRROR_AGREEMENT_PERCENT;
    let (mut ix, mut ux) = (0u64, 0u64);
    let (mut iy, mut uy) = (0u64, 0u64);
    let (mut ih, mut uh) = (0u64, 0u64);
    let (mut left, mut right, mut top, mut bottom) = (0u64, 0u64, 0u64, 0u64);
    let (half_w, half_h) = (bw / 2, bh / 2);
    for y in miny..=maxy {
        for x in minx..=maxx {
            let (fx, fy) = (maxx - (x - minx), maxy - (y - miny));
            let here = opaque(x, y);
            for (other, (i, u)) in [
                (opaque(fx, y), (&mut ix, &mut ux)),
                (opaque(x, fy), (&mut iy, &mut uy)),
                (opaque(fx, fy), (&mut ih, &mut uh)),
            ] {
                *i += u64::from(here && other);
                *u += u64::from(here || other);
            }
            if here {
                if x - minx < half_w {
                    left += 1;
                } else if maxx - x < half_w {
                    right += 1;
                }
                if y - miny < half_h {
                    top += 1;
                } else if maxy - y < half_h {
                    bottom += 1;
                }
            }
        }
    }
    if agrees(ih, uh) {
        return (cx, cy);
    }
    let diagonal = mirrors_about_a_diagonal(&opaque, minx, miny, bw, bh, &agrees);
    let side = |heavier_is_max: bool, lo: i32, hi: i32| if heavier_is_max { hi } else { lo };
    (
        if agrees(ix, ux) {
            cx
        } else if agrees(iy, uy) || diagonal {
            side(right > left, minx, maxx)
        } else {
            minx
        },
        if agrees(iy, uy) {
            cy
        } else if agrees(ix, ux) || diagonal {
            side(bottom > top, miny, maxy)
        } else {
            miny
        },
    )
}

fn mirrors_about_a_diagonal(
    opaque: &impl Fn(i32, i32) -> bool,
    minx: i32,
    miny: i32,
    bw: i32,
    bh: i32,
    agrees: &impl Fn(u64, u64) -> bool,
) -> bool {
    let k = bw.max(bh);
    for (dx, dy) in [(0, 0), (k - bw, k - bh)] {
        let cell = |r: i32, c: i32| {
            let (x, y) = (c - dx, r - dy);
            (0..bw).contains(&x) && (0..bh).contains(&y) && opaque(minx + x, miny + y)
        };
        let (mut id, mut ud, mut ia, mut ua) = (0u64, 0u64, 0u64, 0u64);
        for r in 0..k {
            for c in 0..k {
                let here = cell(r, c);
                for (other, (i, u)) in [
                    (cell(c, r), (&mut id, &mut ud)),
                    (cell(k - 1 - c, k - 1 - r), (&mut ia, &mut ua)),
                ] {
                    *i += u64::from(here && other);
                    *u += u64::from(here || other);
                }
            }
        }
        if agrees(id, ud) || agrees(ia, ua) {
            return true;
        }
    }
    false
}

pub struct DisplaySnapshot {
    pub name: String,
    pub crtc_id: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub refresh_hz: u32,
    pub active: bool,
}

pub struct DrmDevice {
    pub path: String,
    pub render_node: String,
    pub display_count: u32,
}

fn cstr_field(buf: &[std::os::raw::c_char]) -> String {
    let bytes: &[u8] =
        unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const u8, buf.len()) };
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

pub fn list_devices() -> Option<Vec<DrmDevice>> {
    let lib = drmtap_dl::get()?;
    let f = lib.list_devices?;
    const MAX: usize = 16;
    let mut raw: [drmtap_device; MAX] = unsafe { std::mem::zeroed() };
    let n = unsafe { f(raw.as_mut_ptr(), MAX as std::os::raw::c_int) };
    if n < 0 {
        log::warn!("drmtap_list_devices failed ({n}); using single-device auto-detect");
        return None;
    }
    let n = (n as usize).min(MAX);
    Some(
        raw[..n]
            .iter()
            .map(|d| DrmDevice {
                path: cstr_field(&d.path),
                render_node: cstr_field(&d.render_node),
                display_count: d.display_count,
            })
            .collect(),
    )
}

pub(super) fn device_under_dev_dri(path: &str) -> Option<std::path::PathBuf> {
    let p = std::fs::canonicalize(path).ok()?;
    if p.parent() == Some(std::path::Path::new("/dev/dri")) {
        Some(p)
    } else {
        None
    }
}

pub struct DrmReader {
    lib: &'static DrmtapLib,
    ctx: *mut drmtap_ctx,
    buf: Vec<u8>,
    last_provenance: i8,
    last_plane_rotation: Option<u32>,
}

impl DrmReader {
    pub fn open(device: Option<&str>, crtc_id: u32) -> Option<DrmReader> {
        let lib = drmtap_dl::get()?;
        let device_cstr = match device {
            None => None,
            Some(d) => {
                let Some(canonical) = device_under_dev_dri(d) else {
                    log::warn!("DRM device {d:?} is not under /dev/dri; refusing to open");
                    return None;
                };
                match canonical.to_str().and_then(|s| CString::new(s).ok()) {
                    Some(c) => Some(c),
                    None => return None,
                }
            }
        };
        let cfg = drmtap_config {
            device_path: device_cstr.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()),
            crtc_id,
            helper_path: std::ptr::null(),
            debug: 0,
        };
        let ctx = unsafe { (lib.open)(&cfg) };
        drop(device_cstr);
        if ctx.is_null() {
            log::info!("drmtap_open failed; DRM capture unavailable");
            return None;
        }
        Some(DrmReader {
            last_provenance: -1,
            lib,
            ctx,
            buf: Vec::new(),
            last_plane_rotation: None,
        })
    }

    pub fn grab(&mut self) -> io::Result<(&[u8], usize, usize)> {
        unsafe {
            let mut frame: drmtap_frame_info = std::mem::zeroed();
            let ret = (self.lib.grab_mapped)(self.ctx, &mut frame);
            if ret < 0 {
                let errno = -ret;
                if errno == hbb_common::libc::EAGAIN
                    || errno == hbb_common::libc::EBUSY
                    || errno == hbb_common::libc::EINTR
                {
                    return Err(io::ErrorKind::WouldBlock.into());
                }
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("drmtap_grab_mapped failed: errno {errno}"),
                ));
            }
            self.last_plane_rotation = self.read_plane_rotation();
            if frame.data.is_null() || frame.width == 0 || frame.height == 0 {
                (self.lib.frame_release)(self.ctx, &mut frame);
                return Err(io::ErrorKind::WouldBlock.into());
            }
            let w = frame.width;
            let h = frame.height;
            let stride = frame.stride as usize;
            if w > MAX_DIM || h > MAX_DIM || stride < (w as usize) * 4 {
                log::warn!(
                    "DRM scanout not 32-bit BGRA-compatible ({w}x{h} stride {stride} fourcc {:#010x}); falling back",
                    frame.format
                );
                (self.lib.frame_release)(self.ctx, &mut frame);
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "unsupported DRM scanout format",
                ));
            }
            if frame.format != 0
                && frame.format != DRM_FORMAT_XRGB8888
                && frame.format != DRM_FORMAT_ARGB8888
            {
                log::warn!(
                    "DRM scanout fourcc {:#010x} is not BGRA-compatible; falling back",
                    frame.format
                );
                (self.lib.frame_release)(self.ctx, &mut frame);
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "unsupported DRM scanout format",
                ));
            }
            let (w, h) = (w as usize, h as usize);
            let frame_size = match w.checked_mul(4).and_then(|x| x.checked_mul(h)) {
                Some(sz) if sz > 0 && sz <= MAX_FRAME_BYTES => sz,
                other => {
                    log::warn!(
                        "DRM scanout geometry {w}x{h} yields an out-of-range frame ({other:?} bytes); falling back"
                    );
                    (self.lib.frame_release)(self.ctx, &mut frame);
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        "DRM scanout frame too large",
                    ));
                }
            };
            match stride.checked_mul(h) {
                Some(sz) if sz > 0 && sz <= MAX_FRAME_BYTES => {}
                other => {
                    log::warn!(
                        "DRM scanout stride {stride} x {h} rows is out of range ({other:?} bytes); falling back"
                    );
                    (self.lib.frame_release)(self.ctx, &mut frame);
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        "DRM scanout stride out of range",
                    ));
                }
            }
            if self.buf.len() != frame_size {
                self.buf.resize(frame_size, 0);
            }
            let src = frame.data as *const u8;
            let dst = self.buf.as_mut_ptr();
            if stride == w * 4 {
                std::ptr::copy_nonoverlapping(src, dst, frame_size);
            } else {
                for y in 0..h {
                    std::ptr::copy_nonoverlapping(src.add(y * stride), dst.add(y * w * 4), w * 4);
                }
            }
            (self.lib.frame_release)(self.ctx, &mut frame);
            Ok((&self.buf, w, h))
        }
    }

    pub fn render_node(&mut self) -> Option<String> {
        let f = self.lib.render_node?;
        let ptr = unsafe { f(self.ctx) };
        if ptr.is_null() {
            return None;
        }
        unsafe { std::ffi::CStr::from_ptr(ptr) }
            .to_str()
            .ok()
            .map(|s| s.to_owned())
    }

    pub fn plane_rotation(&self) -> Option<u32> {
        self.last_plane_rotation
    }

    pub fn crtc_refresh(&mut self) -> Option<(u64, u64)> {
        let f = self.lib.crtc_refresh?;
        let (mut num, mut den) = (0u64, 0u64);
        let rc = unsafe { f(self.ctx, &mut num, &mut den) };
        (rc == 0 && num != 0 && den != 0).then_some((num, den))
    }

    fn read_plane_rotation(&mut self) -> Option<u32> {
        let f = self.lib.plane_rotation?;
        let mut rotation: u32 = 0;
        let rc = unsafe { f(self.ctx, &mut rotation) };
        plane_rotation_answer(rc, rotation)
    }

    pub fn query_plane_rotation(&mut self) -> Option<u32> {
        self.read_plane_rotation()
    }

    pub fn grab_desc(&mut self) -> io::Result<(OwnedFd, drmtap_dmabuf_desc)> {
        let grab_desc = self.lib.grab_desc;
        unsafe {
            let mut desc: drmtap_dmabuf_desc = std::mem::zeroed();
            let mut frame: drmtap_frame_info = std::mem::zeroed();
            let ret = grab_desc(self.ctx, &mut desc, &mut frame);
            if ret < 0 {
                let errno = -ret;
                if errno == hbb_common::libc::EAGAIN
                    || errno == hbb_common::libc::EBUSY
                    || errno == hbb_common::libc::EINTR
                {
                    return Err(io::ErrorKind::WouldBlock.into());
                }
                if errno == hbb_common::libc::ENOTSUP {
                    return Err(io::Error::new(
                        io::ErrorKind::Unsupported,
                        "drmtap_grab_desc: no transferable dma-buf (ENOTSUP)",
                    ));
                }
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("drmtap_grab_desc failed: errno {errno}"),
                ));
            }
            self.last_plane_rotation = self.read_plane_rotation();
            let raw_fd = if desc.dma_buf_fd >= 0 {
                desc.dma_buf_fd
            } else {
                frame.dma_buf_fd
            };
            if raw_fd < 0 {
                (self.lib.frame_release)(self.ctx, &mut frame);
                return Err(io::ErrorKind::WouldBlock.into());
            }
            let w = desc.width;
            let h = desc.height;
            if w == 0 || h == 0 || w > MAX_DIM || h > MAX_DIM {
                (self.lib.frame_release)(self.ctx, &mut frame);
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("DRM scanout geometry {w}x{h} out of range"),
                ));
            }
            let planes = if desc.num_planes == 0 { 1 } else { desc.num_planes };
            if planes > 4 {
                (self.lib.frame_release)(self.ctx, &mut frame);
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("DRM scanout num_planes {} out of range (1..=4)", desc.num_planes),
                ));
            }
            for p in 0..(planes as usize) {
                let extent = (desc.pitches[p] as usize)
                    .checked_mul(h as usize)
                    .and_then(|rows| rows.checked_add(desc.offsets[p] as usize));
                match extent {
                    Some(end) if end <= MAX_FRAME_BYTES => {}
                    other => {
                        (self.lib.frame_release)(self.ctx, &mut frame);
                        return Err(io::Error::new(
                            io::ErrorKind::Other,
                            format!(
                                "DRM scanout plane {p} out of range (offset {} pitch {} over {h} rows -> {other:?}, cap {MAX_FRAME_BYTES})",
                                desc.offsets[p], desc.pitches[p]
                            ),
                        ));
                    }
                }
            }
            let dup_fd = hbb_common::libc::fcntl(raw_fd, hbb_common::libc::F_DUPFD_CLOEXEC, 0);
            if dup_fd < 0 {
                let e = io::Error::last_os_error();
                (self.lib.frame_release)(self.ctx, &mut frame);
                return Err(e);
            }
            let owned = OwnedFd::from_raw_fd(dup_fd);
            (self.lib.frame_release)(self.ctx, &mut frame);
            desc.num_planes = planes;
            desc.dma_buf_fd = -1;
            Ok((owned, desc))
        }
    }

    pub fn cursor(&mut self) -> Option<CursorSnapshot> {
        unsafe {
            let mut c: drmtap_cursor_info = std::mem::zeroed();
            let cret = (self.lib.get_cursor)(self.ctx, &mut c);
            if cret != 0 {
                return None;
            }
            let out = if c.visible == 0 {
                Some(CursorSnapshot {
                    id: HIDDEN_CURSOR_ID,
                    width: 1,
                    height: 1,
                    hotx: 0,
                    hoty: 0,
                    hot_measured: false,
                    colors: vec![0, 0, 0, 0],
                    x: 0,
                    y: 0,
                })
            } else if !c.pixels.is_null()
                && c.width > 0
                && c.height > 0
                && (c.width as i64) * (c.height as i64) <= 256 * 256
            {
                let cw = c.width as i32;
                let ch = c.height as i32;
                let n = (cw * ch) as usize;
                let src = std::slice::from_raw_parts(c.pixels, n);
                let mut hash: u64 = 1469598103934665603;
                let mut colors = Vec::with_capacity(n * 4);
                for &p in src.iter() {
                    colors.push(((p >> 16) & 0xff) as u8);
                    colors.push(((p >> 8) & 0xff) as u8);
                    colors.push((p & 0xff) as u8);
                    colors.push(((p >> 24) & 0xff) as u8);
                    hash ^= p as u64;
                    hash = hash.wrapping_mul(1099511628211);
                }
                let answered = self.lib.cursor_hotspot_valid.and_then(|f| {
                    let mut valid: c_int = 0;
                    (f(&c, &mut valid) == 0).then(|| valid != 0)
                });
                let hot_measured = hot_measured_from(answered, c.hot_x, c.hot_y);
                self.note_hotspot_provenance(answered, hot_measured);
                let (hotx, hoty) = if hot_measured {
                    (c.hot_x, c.hot_y)
                } else {
                    infer_hotspot(&colors, cw as usize, ch as usize)
                };
                let id = cursor_id(hash, cw as u32, ch as u32, hotx, hoty, hot_measured);
                Some(CursorSnapshot {
                    id,
                    width: cw as u32,
                    height: ch as u32,
                    hotx,
                    hoty,
                    hot_measured,
                    colors,
                    x: c.x,
                    y: c.y,
                })
            } else {
                None
            };
            (self.lib.cursor_release)(self.ctx, &mut c);
            out
        }
    }

    pub fn displays(&mut self) -> Vec<DisplaySnapshot> {
        unsafe {
            let mut raw = vec![std::mem::zeroed::<drmtap_display>(); 16];
            let cap = raw.len() as i32;
            let n = (self.lib.list_displays)(self.ctx, raw.as_mut_ptr(), cap);
            if n <= 0 {
                return Vec::new();
            }
            let count = (n as usize).min(raw.len());
            (0..count)
                .map(|i| {
                    let name_bytes: Vec<u8> = raw[i]
                        .name
                        .iter()
                        .take_while(|&&ch| ch != 0)
                        .map(|&ch| ch as u8)
                        .collect();
                    DisplaySnapshot {
                        name: String::from_utf8_lossy(&name_bytes).to_string(),
                        crtc_id: raw[i].crtc_id,
                        x: raw[i].x as i32,
                        y: raw[i].y as i32,
                        width: raw[i].width,
                        height: raw[i].height,
                        refresh_hz: raw[i].refresh_hz,
                        active: raw[i].active != 0,
                    }
                })
                .collect()
        }
    }
}

impl Drop for DrmReader {
    fn drop(&mut self) {
        if !self.ctx.is_null() {
            unsafe { (self.lib.close)(self.ctx) };
            self.ctx = std::ptr::null_mut();
        }
    }
}

#[cfg(test)]
mod hotspot_provenance_tests {
    use super::hot_measured_from;

    #[test]
    fn a_measured_zero_hotspot_is_measured() {
        assert!(hot_measured_from(Some(true), 0, 0));
    }
}

#[cfg(test)]
mod hotspot_guess_tests {
    use super::infer_hotspot;

    fn sprite(rows: &[&str]) -> (Vec<u8>, usize, usize) {
        let (h, w) = (rows.len(), rows[0].len());
        let mut px = vec![0u8; w * h * 4];
        for (y, row) in rows.iter().enumerate() {
            assert_eq!(row.len(), w, "ragged picture");
            for (x, c) in row.bytes().enumerate() {
                if c == b'#' {
                    px[(y * w + x) * 4 + 3] = 255;
                }
            }
        }
        (px, w, h)
    }

    #[test]
    fn an_arrow_points_at_the_corner_of_its_box() {
        let (px, w, h) = sprite(&[
            ".........",
            ".#.......",
            ".##......",
            ".###.....",
            ".####....",
            ".#####...",
            ".###.....",
            ".#..##...",
            "......##.",
        ]);
        assert_eq!(infer_hotspot(&px, w, h), (1, 1));
    }

    #[test]
    fn a_symmetric_shape_is_held_by_its_middle() {
        let (px, w, h) = sprite(&[
            "....#....",
            "....#....",
            "....#....",
            "....#....",
            "#########",
            "....#....",
            "....#....",
            "....#....",
            "....#....",
        ]);
        assert_eq!(infer_hotspot(&px, w, h), (4, 4));
    }

    #[test]
    fn a_directional_resize_cursor_is_held_by_its_bar() {
        let (px, w, h) = sprite(&[
            "......####",
            "..##..####",
            ".###..####",
            "####..####",
            ".###..####",
            "..##..####",
            "......####",
        ]);
        assert_eq!(infer_hotspot(&px, w, h), (9, 3));
        let (px, w, h) = sprite(&[
            "####......",
            "####..##..",
            "####..###.",
            "####..####",
            "####..###.",
            "####..##..",
            "####......",
        ]);
        assert_eq!(infer_hotspot(&px, w, h), (0, 3));
    }

    #[test]
    fn an_elongated_shape_keeps_its_centre_even_when_it_is_lopsided() {
        let (px, w, h) = sprite(&["##########.", "#####......", "##########."]);
        assert_eq!(infer_hotspot(&px, w, h), (4, 1));
    }
}

const DRM_MODE_ROTATE_0: u32 = 1 << 0;

fn plane_rotation_answer(rc: c_int, rotation: u32) -> Option<u32> {
    if rc == 0 {
        Some(rotation)
    } else if rc == -hbb_common::libc::ENOTSUP {
        Some(DRM_MODE_ROTATE_0)
    } else {
        None
    }
}

#[cfg(test)]
mod plane_rotation_tests {
    use super::*;

    #[test]
    fn a_plane_without_the_property_turned_nothing() {
        assert_eq!(plane_rotation_answer(0, 0x4), Some(0x4));
        assert_eq!(plane_rotation_answer(0, 0x1), Some(0x1));
        assert_eq!(plane_rotation_answer(-hbb_common::libc::ENOTSUP, 0), Some(0x1));
        assert_eq!(plane_rotation_answer(-hbb_common::libc::ENOENT, 0), None);
        assert_eq!(plane_rotation_answer(-hbb_common::libc::EINVAL, 0), None);
        assert_eq!(plane_rotation_answer(-hbb_common::libc::EIO, 7), None);
    }
}

#[cfg(test)]
mod crtc_refresh_tests {
    use super::*;
    use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};

    static RC: AtomicI32 = AtomicI32::new(0);
    static NUM: AtomicU64 = AtomicU64::new(0);
    static DEN: AtomicU64 = AtomicU64::new(0);

    unsafe extern "C" fn crtc_refresh(_: *mut drmtap_ctx, num: *mut u64, den: *mut u64) -> c_int {
        *num = NUM.load(Ordering::SeqCst);
        *den = DEN.load(Ordering::SeqCst);
        RC.load(Ordering::SeqCst)
    }
    unsafe extern "C" fn grab_mapped(_: *mut drmtap_ctx, _: *mut drmtap_frame_info) -> c_int {
        -1
    }
    unsafe extern "C" fn frame_release(_: *mut drmtap_ctx, _: *mut drmtap_frame_info) {}
    unsafe extern "C" fn grab_desc(
        _: *mut drmtap_ctx,
        _: *mut drmtap_dmabuf_desc,
        _: *mut drmtap_frame_info,
    ) -> c_int {
        -1
    }
    unsafe extern "C" fn plane_rotation(_: *mut drmtap_ctx, _: *mut u32) -> c_int {
        -1
    }

    fn reader(with_call: bool) -> DrmReader {
        let mut lib = DrmtapLib::fake(grab_mapped, frame_release, grab_desc, plane_rotation);
        if with_call {
            lib.crtc_refresh = Some(crtc_refresh);
        }
        DrmReader {
            lib: Box::leak(Box::new(lib)),
            ctx: std::ptr::null_mut(),
            buf: Vec::new(),
            last_provenance: -1,
            last_plane_rotation: None,
        }
    }

    #[test]
    fn the_crtc_refresh_is_a_fraction_or_nothing() {
        assert_eq!(reader(false).crtc_refresh(), None, "a library before 0.5.9");
        let mut r = reader(true);
        RC.store(0, Ordering::SeqCst);
        NUM.store(148352, Ordering::SeqCst);
        DEN.store(2475, Ordering::SeqCst);
        assert_eq!(r.crtc_refresh(), Some((148352, 2475)));
        RC.store(-hbb_common::libc::ENODATA, Ordering::SeqCst);
        assert_eq!(r.crtc_refresh(), None, "a CRTC with no mode");
        RC.store(0, Ordering::SeqCst);
        DEN.store(0, Ordering::SeqCst);
        assert_eq!(r.crtc_refresh(), None, "a zero denominator is no answer");
    }
}
