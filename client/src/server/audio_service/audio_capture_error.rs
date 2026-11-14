use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[derive(Clone, Default)]
pub(super) struct CaptureErrorHandler {
    interrupted: Arc<AtomicBool>,
}

impl CaptureErrorHandler {
    pub(super) fn handle(&self, error: cpal::StreamError) {
        if matches!(
            error,
            cpal::StreamError::StreamInterrupted { .. } | cpal::StreamError::DeviceNotAvailable
        ) {
            hbb_common::log::error!("Audio capture stream interrupted: {error}");
            self.interrupted.store(true, Ordering::Relaxed);
        } else {
            hbb_common::log::trace!("an error occurred on stream: {error}");
        }
    }

    pub(super) fn process_frame(&self, process: impl FnOnce() -> hbb_common::ResultType<()>) {
        if self.needs_restart() {
            return;
        }
        if let Err(error) = process() {
            self.interrupted.store(true, Ordering::Relaxed);
            hbb_common::log::error!(
                "Failed to process captured audio frame; requesting stream restart: {error:#}"
            );
        }
    }

    pub(super) fn needs_restart(&self) -> bool {
        self.interrupted.load(Ordering::Relaxed)
    }
}
