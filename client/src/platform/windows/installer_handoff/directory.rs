use super::super::{
    installer_shell::{get_known_folder, get_system_executable, path_for_cmd_environment},
    wide_string,
};
use hbb_common::{anyhow::anyhow, bail, log, ResultType};
use std::{
    fs, mem,
    os::windows::fs::MetadataExt,
    path::{Path, PathBuf},
};
use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::{LocalFree, HLOCAL},
        Security::{
            Authorization::{
                ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
            },
            PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES,
        },
        Storage::FileSystem::{CreateDirectoryW, FILE_ATTRIBUTE_REPARSE_POINT},
        UI::Shell::FOLDERID_ProgramData,
    },
};

pub(in crate::platform::windows) fn path() -> ResultType<PathBuf> {
    let system = get_system_executable("")?;
    let windows = system
        .parent()
        .ok_or_else(|| anyhow!("System directory has no parent"))?;
    let temporary = windows.join("Temp");
    let parent = temporary_parent(&temporary)?;
    Ok(parent.join(format!(
        "supportdesk-install-{}",
        uuid::Uuid::new_v4().simple()
    )))
}

fn temporary_parent(temporary: &Path) -> ResultType<PathBuf> {
    let parent = if has_reparse_parent(temporary)? {
        let fallback = get_known_folder(&FOLDERID_ProgramData)?;
        if has_reparse_parent(&fallback)? {
            bail!(
                "Installer fallback directory contains a reparse point: {}",
                fallback.display()
            );
        }
        log::warn!(
            "Installer temporary directory contains a reparse point; using {} instead of {}",
            fallback.display(),
            temporary.display()
        );
        fallback
    } else {
        temporary.to_owned()
    };
    if !supports_acls(&parent).map_err(|error| {
        anyhow!(
            "Failed to inspect installer volume {}: {error}",
            parent.display()
        )
    })? {
        bail!(
            "Installer temporary volume does not support persistent ACLs: {}",
            parent.display()
        );
    }
    Ok(parent)
}

fn has_reparse_parent(path: &Path) -> ResultType<bool> {
    for parent in path.ancestors() {
        let metadata = fs::symlink_metadata(parent).map_err(|err| {
            anyhow!(
                "Failed to inspect installer temporary parent {}: {err}",
                parent.display()
            )
        })?;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0 {
            return Ok(true);
        }
        if !metadata.is_dir() {
            bail!(
                "Installer temporary parent is not a regular directory: {}",
                parent.display()
            );
        }
    }
    Ok(false)
}

fn supports_acls(path: &Path) -> ResultType<bool> {
    use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
    use windows::Win32::{
        Foundation::HANDLE,
        Storage::FileSystem::{
            GetVolumeInformationByHandleW, FILE_FLAG_BACKUP_SEMANTICS, FILE_READ_ATTRIBUTES,
            FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
        },
    };

    let directory = fs::OpenOptions::new()
        .access_mode(FILE_READ_ATTRIBUTES.0)
        .share_mode((FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE).0)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS.0)
        .open(path)?;
    let mut flags = 0;
    unsafe {
        GetVolumeInformationByHandleW(
            HANDLE(directory.as_raw_handle()),
            None,
            None,
            None,
            Some(&mut flags),
            None,
        )?;
    }
    Ok(flags & winapi::um::winnt::FILE_PERSISTENT_ACLS != 0)
}

pub(super) fn create(path: &Path) -> ResultType<()> {
    let wide_path = wide_string(path_for_cmd_environment(path)?);
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            w!("D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;RC;;;OW)"),
            SDDL_REVISION_1,
            &mut descriptor,
            None,
        )?;
    }
    let attributes = SECURITY_ATTRIBUTES {
        nLength: mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: false.into(),
    };
    let result = unsafe { CreateDirectoryW(PCWSTR(wide_path.as_ptr()), Some(&attributes)) };
    unsafe { LocalFree(Some(HLOCAL(descriptor.0))) };
    result.map_err(|err| {
        anyhow!(
            "Failed to create protected installer directory {}: {err}",
            path.display()
        )
    })
}
