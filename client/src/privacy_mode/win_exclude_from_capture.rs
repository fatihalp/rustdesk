use base::platform::windows::is_windows_version_or_greater;

pub use super::win_topmost_window::PrivacyModeImpl;

pub(super) const PRIVACY_MODE_IMPL: &str = super::PRIVACY_MODE_IMPL_WIN_EXCLUDE_FROM_CAPTURE;

pub(super) fn is_supported() -> bool {
    is_windows_version_or_greater(10, 0, 19041, 0, 0)
}
