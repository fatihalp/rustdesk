use crate::{
    platform::unix::{
        filetype::file_list_id,
        local_file::{construct_file_list, LocalFile},
    },
    ClipboardFile, CliprdrError,
};
use hbb_common::{
    bytes::{BufMut, BytesMut},
    log,
};
use parking_lot::Mutex;
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{atomic::Ordering, Arc},
    time::SystemTime,
    usize,
};

const MAX_RANGE_READ: u64 = 16 * 1024 * 1024;

lazy_static::lazy_static! {
    static ref CLIP_FILES: Arc<Mutex<ClipFiles>> = Default::default();
    static ref RETIRED_CLIP_FILES: Mutex<VecDeque<ClipFiles>> = Default::default();
}

const MAX_RETIRED_CLIP_FILES: usize = 4;

const LIST_NONCE: std::ops::Range<usize> = 8..24;

#[derive(Debug)]
enum FileContentsRequest {
    Size {
        stream_id: i32,
        file_idx: usize,
    },

    Range {
        stream_id: i32,
        file_idx: usize,
        offset: u64,
        length: u64,
    },
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct FileSig {
    size: u64,
    mtime: Option<SystemTime>,
    is_dir: bool,
}

fn fingerprint(files: &[String]) -> Vec<FileSig> {
    files
        .iter()
        .map(|s| match std::fs::metadata(s) {
            Ok(mt) => FileSig {
                size: mt.len(),
                mtime: mt.modified().ok(),
                is_dir: mt.is_dir(),
            },
            Err(_) => FileSig::default(),
        })
        .collect()
}

#[derive(Default)]
struct ClipFiles {
    files: Vec<String>,
    sigs: Vec<FileSig>,
    file_list: Vec<LocalFile>,
    first_file_index: usize,
    files_pdu: Vec<u8>,
    id: i32,
    served_to: Vec<i32>,
}

impl ClipFiles {
    fn clear(&mut self) {
        self.files.clear();
        self.sigs.clear();
        self.file_list.clear();
        self.first_file_index = usize::MAX;
        self.files_pdu.clear();
        self.id = 0;
        self.served_to.clear();
    }

    fn sync_files(
        &mut self,
        clipboard_files: &[String],
        sigs: Vec<FileSig>,
    ) -> Result<(), CliprdrError> {
        let clipboard_paths = clipboard_files
            .iter()
            .map(|s| PathBuf::from(s))
            .collect::<Vec<_>>();
        self.file_list = construct_file_list(&clipboard_paths)?;
        self.first_file_index = self
            .file_list
            .iter()
            .position(|f| !f.path.is_dir())
            .unwrap_or(usize::MAX);
        self.files = clipboard_files.to_vec();
        self.sigs = sigs;
        Ok(())
    }

    fn build_file_list_pdu(&mut self) -> Result<(), CliprdrError> {
        let mut data = BytesMut::with_capacity(4 + 592 * self.file_list.len());
        data.put_u32_le(self.file_list.len() as u32);
        for file in self.file_list.iter() {
            data.put(file.as_bin()?.as_slice());
        }
        self.files_pdu = data.to_vec();
        if let Some(nonce) = self.files_pdu.get_mut(LIST_NONCE) {
            nonce.copy_from_slice(&rand::random::<[u8; 16]>());
        }
        self.id = file_list_id(&self.files_pdu);
        Ok(())
    }

    fn release_handles(&mut self) {
        for file in self.file_list.iter_mut() {
            if file.offset.load(Ordering::Relaxed) == 0 {
                file.handle = None;
            }
        }
    }

    fn check_retired_file(&mut self, request: &FileContentsRequest) -> Result<(), CliprdrError> {
        let FileContentsRequest::Range { file_idx, .. } = request else {
            return Ok(());
        };
        let Some(file) = self.file_list.get_mut(*file_idx) else {
            return Ok(());
        };
        if file.is_dir {
            return Ok(());
        }
        file.load_handle()?;
        let unchanged = file
            .handle
            .as_ref()
            .and_then(|handle| handle.get_ref().metadata().ok())
            .map_or(false, |md| {
                md.len() == file.size && md.modified().ok() == Some(file.last_write_time)
            });
        if unchanged {
            Ok(())
        } else {
            Err(CliprdrError::InvalidRequest {
                description: format!("file {} changed since its list was sent", file.name),
            })
        }
    }

    fn get_files_for_audit(&self, request: &FileContentsRequest) -> Option<ClipboardFile> {
        if let FileContentsRequest::Range {
            file_idx, offset, ..
        } = request
        {
            if *file_idx == self.first_file_index && *offset == 0 {
                let files: Vec<(String, u64)> = self
                    .file_list
                    .iter()
                    .filter_map(|f| {
                        if f.path.is_file() {
                            Some((f.path.to_string_lossy().to_string(), f.size))
                        } else {
                            None
                        }
                    })
                    .collect::<_>();
                if files.is_empty() {
                    return None;
                } else {
                    return Some(ClipboardFile::Files { files });
                }
            }
        }
        None
    }

    fn serve_file_contents(
        &mut self,
        conn_id: i32,
        request: FileContentsRequest,
    ) -> Result<ClipboardFile, CliprdrError> {
        let (file_idx, file_contents_resp) = match request {
            FileContentsRequest::Size {
                stream_id,
                file_idx,
            } => {
                log::debug!("file contents (size) requested from conn: {}", conn_id);
                let Some(file) = self.file_list.get(file_idx) else {
                    log::error!(
                        "invalid file index {} requested from conn: {}",
                        file_idx,
                        conn_id
                    );
                    return Err(CliprdrError::InvalidRequest {
                        description: format!(
                            "invalid file index {} requested from conn: {}",
                            file_idx, conn_id
                        ),
                    });
                };

                log::debug!(
                    "conn {} requested file-{}: {}",
                    conn_id,
                    file_idx,
                    file.name
                );

                let size = file.size;
                (
                    file_idx,
                    ClipboardFile::FileContentsResponse {
                        msg_flags: 0x1,
                        stream_id,
                        requested_data: size.to_le_bytes().to_vec(),
                    },
                )
            }
            FileContentsRequest::Range {
                stream_id,
                file_idx,
                offset,
                length,
            } => {
                log::debug!(
                    "file contents (range from {} length {}) request from conn: {}",
                    offset,
                    length,
                    conn_id
                );
                let Some(file) = self.file_list.get_mut(file_idx) else {
                    log::error!(
                        "invalid file index {} requested from conn: {}",
                        file_idx,
                        conn_id
                    );
                    return Err(CliprdrError::InvalidRequest {
                        description: format!(
                            "invalid file index {} requested from conn: {}",
                            file_idx, conn_id
                        ),
                    });
                };
                log::debug!(
                    "conn {} requested file-{}: {}",
                    conn_id,
                    file_idx,
                    file.name
                );

                if offset > file.size {
                    log::error!("invalid reading offset requested from conn: {}", conn_id);
                    return Err(CliprdrError::InvalidRequest {
                        description: format!(
                            "invalid reading offset requested from conn: {}",
                            conn_id
                        ),
                    });
                }
                let read_size = if length > file.size - offset {
                    file.size - offset
                } else {
                    length
                };

                if read_size > MAX_RANGE_READ {
                    return Err(CliprdrError::InvalidRequest {
                        description: format!(
                            "file contents request of {} bytes exceeds the {} byte limit, conn: {}",
                            read_size, MAX_RANGE_READ, conn_id
                        ),
                    });
                }

                let mut buf = vec![0u8; read_size as usize];

                file.read_exact_at(&mut buf, offset)?;

                (
                    file_idx,
                    ClipboardFile::FileContentsResponse {
                        msg_flags: 0x1,
                        stream_id,
                        requested_data: buf,
                    },
                )
            }
        };

        log::debug!("file contents sent to conn: {}", conn_id);
        for next_file in self.file_list.iter_mut().skip(file_idx + 1) {
            if !next_file.is_dir {
                if let Err(e) = next_file.load_handle() {
                    log::trace!("failed to preload the next file: {:?}", e);
                }
                break;
            }
        }
        Ok(file_contents_resp)
    }
}

#[inline]
pub fn clear_files() {
    CLIP_FILES.lock().clear();
    RETIRED_CLIP_FILES.lock().clear();
}

fn select_clip_files<'a>(
    current: &'a mut ClipFiles,
    retired: &'a mut VecDeque<ClipFiles>,
    conn_id: i32,
    clip_data_id: Option<i32>,
) -> Option<&'a mut ClipFiles> {
    match clip_data_id {
        Some(id) if current.id == id => Some(current),
        Some(id) => retired
            .iter_mut()
            .rev()
            .find(|files| files.id == id && files.served_to.contains(&conn_id)),
        None if current.served_to.contains(&conn_id) => Some(current),
        None => retired
            .iter_mut()
            .rev()
            .find(|files| files.served_to.contains(&conn_id))
            .or(Some(current)),
    }
}

fn retire_if_served(mut replaced: ClipFiles) {
    if replaced.served_to.is_empty() {
        return;
    }
    replaced.release_handles();
    let mut retired = RETIRED_CLIP_FILES.lock();
    if retired.len() == MAX_RETIRED_CLIP_FILES {
        retired.pop_front();
    }
    retired.push_back(replaced);
}

pub fn read_file_contents(
    conn_id: i32,
    stream_id: i32,
    list_index: i32,
    dw_flags: i32,
    n_position_low: i32,
    n_position_high: i32,
    cb_requested: i32,
    clip_data_id: Option<i32>,
) -> Vec<Result<ClipboardFile, CliprdrError>> {
    let fcr = if dw_flags == 0x1 {
        FileContentsRequest::Size {
            stream_id,
            file_idx: list_index as usize,
        }
    } else if dw_flags == 0x2 {
        let offset = (n_position_high as u64) << 32 | n_position_low as u32 as u64;
        let length = cb_requested as u32 as u64;

        FileContentsRequest::Range {
            stream_id,
            file_idx: list_index as usize,
            offset,
            length,
        }
    } else {
        return vec![Err(CliprdrError::InvalidRequest {
            description: format!("got invalid FileContentsRequest, dw_flats: {dw_flags}"),
        })];
    };

    let mut current = CLIP_FILES.lock();
    let mut retired = RETIRED_CLIP_FILES.lock();
    let current_ptr: *const ClipFiles = &*current;
    let Some(clip_files) = select_clip_files(&mut current, &mut retired, conn_id, clip_data_id)
    else {
        return vec![Err(CliprdrError::InvalidRequest {
            description: format!(
                "file list {:?} is not available to conn: {}",
                clip_data_id, conn_id
            ),
        })];
    };
    if !std::ptr::eq(&*clip_files, current_ptr) {
        if let Err(e) = clip_files.check_retired_file(&fcr) {
            return vec![Err(e)];
        }
    }
    let mut res = vec![];
    if let Some(files_res) = clip_files.get_files_for_audit(&fcr) {
        res.push(Ok(files_res));
    }
    res.push(clip_files.serve_file_contents(conn_id, fcr));
    res
}

pub fn sync_files(files: &[String]) -> Result<(), CliprdrError> {
    let current = fingerprint(files);
    let mut files_lock = CLIP_FILES.lock();
    if files_lock.files == files
        && files_lock.sigs == current
        && !current.iter().any(|sig| sig.is_dir)
    {
        return Ok(());
    }
    let mut next = ClipFiles::default();
    next.sync_files(files, current)?;
    next.build_file_list_pdu()?;
    let replaced = std::mem::replace(&mut *files_lock, next);
    retire_if_served(replaced);
    Ok(())
}

pub fn get_file_list_pdu(conn_id: i32) -> Vec<u8> {
    let mut clip_files = CLIP_FILES.lock();
    if !clip_files.files_pdu.is_empty() && !clip_files.served_to.contains(&conn_id) {
        clip_files.served_to.push(conn_id);
    }
    clip_files.files_pdu.clone()
}
