use dashmap::DashMap;
use lazy_static::lazy_static;

mod filetype;
pub use filetype::{FileDescription, FileType};
#[cfg(target_os = "linux")]
pub mod fuse;
#[cfg(target_os = "macos")]
pub mod macos;

pub mod local_file;
pub mod serv_files;

pub const FLAGS_FD_ATTRIBUTES: u32 = 0x04;
pub const FLAGS_FD_SIZE: u32 = 0x40;
pub const FLAGS_FD_LAST_WRITE: u32 = 0x20;
pub const FLAGS_FD_PROGRESSUI: u32 = 0x4000;
pub const FLAGS_FD_UNIX_MODE: u32 = 0x08;

pub const FILEDESCRIPTOR_FORMAT_ID: i32 = 49334;
pub const FILEDESCRIPTORW_FORMAT_NAME: &str = "FileGroupDescriptorW";
pub const FILECONTENTS_FORMAT_ID: i32 = 49267;
pub const FILECONTENTS_FORMAT_NAME: &str = "FileContents";

pub(crate) const BLOCK_SIZE: u32 = 4 * 1024 * 1024;

pub(super) const FILE_NAME_CODE_UNITS: usize = 260;
pub(super) const FILE_NAME_FIELD_SIZE: usize = FILE_NAME_CODE_UNITS * std::mem::size_of::<u16>();

const LDAP_EPOCH_DELTA: u64 = 116_444_736_000_000_000;

lazy_static! {
    static ref REMOTE_FORMAT_MAP: DashMap<i32, String> = DashMap::from_iter(
        [
            (
                FILEDESCRIPTOR_FORMAT_ID,
                FILEDESCRIPTORW_FORMAT_NAME.to_string()
            ),
            (FILECONTENTS_FORMAT_ID, FILECONTENTS_FORMAT_NAME.to_string())
        ]
        .iter()
        .cloned()
    );
}

#[inline]
pub fn get_local_format(remote_id: i32) -> Option<String> {
    REMOTE_FORMAT_MAP.get(&remote_id).map(|s| s.clone())
}
