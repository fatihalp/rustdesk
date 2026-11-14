use super::{
    FILE_NAME_FIELD_SIZE, FLAGS_FD_ATTRIBUTES, FLAGS_FD_LAST_WRITE, FLAGS_FD_UNIX_MODE,
    LDAP_EPOCH_DELTA,
};
use crate::CliprdrError;
use hbb_common::{
    bytes::{Buf, Bytes},
    log,
};
use serde_derive::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    time::{Duration, SystemTime},
};
use utf16string::WStr;

#[cfg(target_os = "linux")]
pub type Inode = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileType {
    File,
    Directory,
    Symlink,
}

pub const PERM_READ: u16 = 0o444;
pub const PERM_RW: u16 = 0o644;
pub const PERM_SELF_RO: u16 = 0o400;
pub const PERM_RWX: u16 = 0o755;
#[allow(dead_code)]
pub const MAX_NAME_LEN: usize = 255;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileDescription {
    pub conn_id: i32,
    pub name: PathBuf,
    pub kind: FileType,
    pub atime: SystemTime,
    pub last_modified: SystemTime,
    pub last_metadata_changed: SystemTime,
    pub creation_time: SystemTime,
    pub size: u64,
    pub perm: u16,
    #[serde(default)]
    pub clip_data_id: i32,
}

pub fn file_list_id(file_descriptor_pdu: &[u8]) -> i32 {
    file_descriptor_pdu
        .iter()
        .fold(0x811c_9dc5_u32, |id, byte| {
            (id ^ *byte as u32).wrapping_mul(0x0100_0193)
        }) as i32
}

pub(super) fn validate_file_name(name: &str) -> Result<(), CliprdrError> {
    if matches!(name.as_bytes(), [letter, b':', b'/', ..] if letter.is_ascii_alphabetic())
        || name
            .split('/')
            .any(|component| component.is_empty() || component == ".")
    {
        return Err(CliprdrError::InvalidRequest {
            description: "clipboard file name is not a normalized relative path".to_string(),
        });
    }
    base::fs::validate_file_name_no_traversal(name).map_err(|error| CliprdrError::InvalidRequest {
        description: error.to_string(),
    })
}

impl FileDescription {
    fn parse_file_descriptor(
        bytes: &mut Bytes,
        conn_id: i32,
        clip_data_id: i32,
    ) -> Result<FileDescription, CliprdrError> {
        let flags = bytes.get_u32_le();
        bytes.advance(32);
        let attributes = bytes.get_u32_le();

        bytes.advance(12);
        let perm = bytes.get_u32_le() as u16;

        let last_write_time = bytes.get_u64_le();
        let file_size_high = bytes.get_u32_le();
        let file_size_low = bytes.get_u32_le();
        let block = bytes.clone();
        bytes.advance(FILE_NAME_FIELD_SIZE);

        let block = &block[..FILE_NAME_FIELD_SIZE];
        let utf16_unit_size = std::mem::size_of::<u16>();
        let name_end = block
            .chunks_exact(utf16_unit_size)
            .position(|unit| unit == [0_u8, 0_u8])
            .ok_or_else(|| CliprdrError::InvalidRequest {
                description: "clipboard file name is not null-terminated".to_string(),
            })?
            * utf16_unit_size;
        let wstr = WStr::from_utf16le(&block[..name_end]).map_err(|e| {
            log::error!("cannot convert file descriptor path: {:?}", e);
            CliprdrError::ConversionFailure
        })?;

        let from_unix = flags & FLAGS_FD_UNIX_MODE != 0;

        let valid_attributes = flags & FLAGS_FD_ATTRIBUTES != 0;
        if !valid_attributes {
            return Err(CliprdrError::InvalidRequest {
                description: "file description must have valid attributes".to_string(),
            });
        }

        let directory = attributes & 0x10 != 0;
        let normal = attributes == 0x80;
        let hidden = attributes & 0x02 != 0;
        let readonly = attributes & 0x01 != 0;

        let perm = if from_unix {
            perm
        } else if normal {
            PERM_RWX
        } else if directory {
            PERM_RWX
        } else if readonly {
            PERM_READ
        } else if hidden {
            PERM_SELF_RO
        } else {
            PERM_RW
        };

        let kind = if directory {
            FileType::Directory
        } else {
            FileType::File
        };

        let valid_size = true;
        let size = if valid_size {
            ((file_size_high as u64) << 32) + file_size_low as u64
        } else {
            0
        };

        let valid_write_time = flags & FLAGS_FD_LAST_WRITE != 0;
        let last_modified = if valid_write_time && last_write_time >= LDAP_EPOCH_DELTA {
            let last_write_time = last_write_time - LDAP_EPOCH_DELTA;
            let last_write_time = Duration::new(
                last_write_time / 10_000_000,
                (last_write_time % 10_000_000) as u32 * 100,
            );
            SystemTime::UNIX_EPOCH + last_write_time
        } else {
            SystemTime::UNIX_EPOCH
        };

        let name = wstr.to_utf8().replace('\\', "/");
        validate_file_name(&name)?;
        let name = PathBuf::from(name);

        let desc = FileDescription {
            conn_id,
            name,
            kind,
            atime: last_modified,
            last_modified,
            last_metadata_changed: last_modified,
            creation_time: last_modified,
            size,
            perm,
            clip_data_id,
        };

        Ok(desc)
    }

    pub fn parse_file_descriptors(
        file_descriptor_pdu: Vec<u8>,
        conn_id: i32,
    ) -> Result<Vec<Self>, CliprdrError> {
        let clip_data_id = file_list_id(&file_descriptor_pdu);
        let mut data = Bytes::from(file_descriptor_pdu);
        if data.remaining() < 4 {
            return Err(CliprdrError::InvalidRequest {
                description: "file descriptor request with infficient length".to_string(),
            });
        }

        let count = data.get_u32_le() as usize;
        if data.remaining() == 0 && count == 0 {
            return Ok(Vec::new());
        }

        if data.remaining() != 592 * count {
            return Err(CliprdrError::InvalidRequest {
                description: "file descriptor request with invalid length".to_string(),
            });
        }

        let mut files = Vec::with_capacity(count);
        for _ in 0..count {
            let desc = Self::parse_file_descriptor(&mut data, conn_id, clip_data_id)?;
            files.push(desc);
        }

        Ok(files)
    }
}
