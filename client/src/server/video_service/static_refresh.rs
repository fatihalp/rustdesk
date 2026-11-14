use super::{GenericService, VideoFrameController, VideoSource};
use base::message_proto::Message;
use hbb_common::{bail, ResultType};
use scrap::{
    codec::{Encoder, EncoderCfg, BR_BALANCED, BR_BEST, BR_SPEED},
    record::Recorder,
    CodecFormat, EncodeInput,
};
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const INITIAL_REPEAT_INTERVAL: Duration = Duration::from_millis(100);
const MAX_REPEAT_FPS: u32 = 30;
const MAX_REPEAT_FAILURES: usize = 3;
const MAX_REPEAT_NO_OUTPUTS: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HardwareBackend {
    Nvenc,
    Qsv,
    Amf,
    Vaapi,
    VideoToolbox,
    MediaCodec,
    Other,
}

fn max_repeat_frames(codec: CodecFormat, quality: f32, backend: HardwareBackend) -> usize {
    use HardwareBackend::*;
    let (low, low_to_balanced, balanced_to_best, best) = match (codec, backend) {
        (CodecFormat::VP8, _) => (20, 20, 20, 30),
        (CodecFormat::VP9, _) => (40, 20, 20, 20),
        (CodecFormat::H264, Nvenc) => (80, 50, 60, 30),
        (CodecFormat::H264, Qsv) => (80, 70, 130, 110),
        (CodecFormat::H264, Amf) => (150, 150, 170, 110),
        (CodecFormat::H264, Vaapi) => (150, 100, 90, 60),
        (CodecFormat::H265, Nvenc) => (160, 90, 60, 30),
        (CodecFormat::H265, Qsv) => (150, 30, 30, 30),
        (CodecFormat::H265, Amf) => (200, 200, 200, 120),
        (CodecFormat::H265, VideoToolbox) => (90, 90, 50, 90),
        (CodecFormat::H264, MediaCodec) => (50, 20, 20, 10),
        (CodecFormat::H265, MediaCodec) => (90, 20, 20, 20),
        (CodecFormat::H264, _) => (90, 60, 60, 30),
        (CodecFormat::H265, _) => (90, 60, 60, 30),
        (CodecFormat::AV1 | CodecFormat::Unknown, _) => return 0,
    };
    if quality >= BR_BEST {
        best
    } else if quality >= BR_BALANCED {
        balanced_to_best
    } else if quality >= BR_SPEED {
        low_to_balanced
    } else {
        low
    }
}

#[derive(Debug)]
pub(super) struct FrameEncodeResult {
    pub(super) send_conn_ids: HashSet<i32>,
    pub(super) vpx_no_output: bool,
}

pub(super) struct StaticRefresh<'a> {
    source: VideoSource,
    codec_format: CodecFormat,
    hardware_backend: HardwareBackend,
    sp: &'a GenericService,
    recorder: &'a Arc<Mutex<Option<Recorder>>>,
    display_idx: usize,
    width: usize,
    height: usize,
    last_encode: Instant,
    repeat_output_counter: usize,
    repeat_output_limit: usize,
    repeat_no_outputs: usize,
    repeat_failures: usize,
    source_ready: bool,
    #[cfg(all(windows, feature = "vram"))]
    has_texture: bool,
}

impl<'a> StaticRefresh<'a> {
    pub(super) fn new(
        source: VideoSource,
        codec_format: CodecFormat,
        encoder_config: &EncoderCfg,
        sp: &'a GenericService,
        recorder: &'a Arc<Mutex<Option<Recorder>>>,
        display_idx: usize,
        width: usize,
        height: usize,
    ) -> Self {
        let hardware_backend = match encoder_config {
            #[cfg(feature = "hwcodec")]
            EncoderCfg::HWRAM(config) => match config.name.as_str() {
                "h264_nvenc" | "hevc_nvenc" => HardwareBackend::Nvenc,
                "h264_qsv" | "hevc_qsv" => HardwareBackend::Qsv,
                "h264_amf" | "hevc_amf" => HardwareBackend::Amf,
                "h264_vaapi" => HardwareBackend::Vaapi,
                "hevc_videotoolbox" => HardwareBackend::VideoToolbox,
                "h264_mediacodec" | "hevc_mediacodec" => HardwareBackend::MediaCodec,
                _ => HardwareBackend::Other,
            },
            #[cfg(feature = "vram")]
            EncoderCfg::VRAM(config) => match config.feature.vendor {
                scrap::vram::Driver::NV => HardwareBackend::Nvenc,
                scrap::vram::Driver::MFX => HardwareBackend::Qsv,
                scrap::vram::Driver::AMF => HardwareBackend::Amf,
                _ => HardwareBackend::Other,
            },
            _ => HardwareBackend::Other,
        };
        Self {
            source,
            codec_format,
            hardware_backend,
            sp,
            recorder,
            display_idx,
            width,
            height,
            last_encode: Instant::now(),
            repeat_output_counter: 0,
            repeat_output_limit: 0,
            repeat_no_outputs: 0,
            repeat_failures: 0,
            source_ready: false,
            #[cfg(all(windows, feature = "vram"))]
            has_texture: false,
        }
    }

    pub(super) fn on_frame(&mut self, _frame: &EncodeInput) {
        self.repeat_output_counter = 0;
        self.repeat_output_limit = 0;
        self.repeat_no_outputs = 0;
        self.source_ready = false;
        #[cfg(all(windows, feature = "vram"))]
        {
            self.has_texture =
                matches!(_frame, EncodeInput::Texture((texture, 0)) if !texture.is_null());
        }
    }

    pub(super) fn on_frame_encoded(&mut self, result: &FrameEncodeResult) {
        self.on_encoded(
            !result.send_conn_ids.is_empty()
                || (matches!(self.codec_format, CodecFormat::VP8 | CodecFormat::VP9)
                    && result.vpx_no_output),
        );
    }

    pub(super) fn on_encoded(&mut self, success: bool) {
        self.last_encode = Instant::now();
        if success {
            self.source_ready = true;
            self.repeat_failures = 0;
        }
    }

    pub(super) fn try_encode(
        &mut self,
        yuv: &[u8],
        spf: Duration,
        quality: f32,
        now: Instant,
        ms: i64,
        encoder: &mut Encoder,
        frame_controller: &mut VideoFrameController,
    ) -> ResultType<()> {
        let spf = spf.max(Duration::from_secs(1) / MAX_REPEAT_FPS);
        self.repeat_output_limit = self.repeat_output_limit.max(max_repeat_frames(
            self.codec_format,
            quality,
            self.hardware_backend,
        ));
        if !self.source.is_monitor()
            || self.codec_format == CodecFormat::AV1
            || !self.source_ready
            || (self.repeat_failures >= MAX_REPEAT_FAILURES
                && !matches!(
                    self.codec_format,
                    CodecFormat::VP8 | CodecFormat::VP9 | CodecFormat::AV1
                ))
            || self.repeat_no_outputs >= MAX_REPEAT_NO_OUTPUTS
            || self.repeat_output_counter >= self.repeat_output_limit
            || {
                let elapsed = self.last_encode.elapsed();
                let interval = if matches!(self.codec_format, CodecFormat::VP8 | CodecFormat::VP9)
                    || (self.repeat_output_counter == 0 && self.repeat_no_outputs == 0)
                {
                    INITIAL_REPEAT_INTERVAL.max(spf)
                } else {
                    spf
                };
                elapsed < interval
            }
        {
            return Ok(());
        }
        let frame = if yuv.is_empty() {
            None
        } else {
            Some(EncodeInput::YUV(yuv))
        };
        #[cfg(all(windows, feature = "vram"))]
        let frame = frame.or_else(|| self.has_texture.then_some(EncodeInput::Repeat));
        if let Some(frame) = frame {
            self.sp.snapshot(|sps| {
                if sps.has_subscribes() {
                    bail!("SWITCH");
                }
                Ok(())
            })?;

            let result = encoder.encode_to_message(frame, ms);
            self.last_encode = if matches!(self.codec_format, CodecFormat::VP8 | CodecFormat::VP9) {
                Instant::now()
            } else {
                now
            };
            let mut vf = match result {
                Ok(vf) if vf.union.is_none() => {
                    self.repeat_no_outputs += 1;
                    return Ok(());
                }
                Ok(vf) => vf,
                Err(error) => {
                    self.repeat_no_outputs += 1;
                    self.repeat_failures += 1;
                    if error.is::<scrap::VpxNoOutput>() {
                        hbb_common::throttled_log!(
                            Duration::from_secs(60),
                            trace,
                            "static refresh produced no output: {error:?}"
                        );
                    } else {
                        hbb_common::throttled_log!(
                            Duration::from_secs(60),
                            debug,
                            "static refresh failed ({}): {error:?}",
                            self.repeat_failures
                        );
                    }
                    return Ok(());
                }
            };
            self.repeat_output_counter += 1;
            self.repeat_failures = 0;
            vf.display = self.display_idx as _;
            let mut msg = Message::new();
            msg.set_video_frame(vf);
            self.recorder
                .lock()
                .unwrap()
                .as_mut()
                .map(|r| r.write_message(&msg, self.width, self.height));
            let send_conn_ids = self.sp.send_video_frame(msg);
            frame_controller.set_send(now, send_conn_ids);
        }
        Ok(())
    }
}
