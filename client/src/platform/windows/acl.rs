
use super::{read_token_user_buffer, wide_string, ResultType};
use hbb_common::{anyhow::anyhow, bail};
use std::{
    fs, io,
    os::windows::{ffi::OsStrExt, fs::MetadataExt},
    path::Path,
};
use windows::{
    core::{PCWSTR, PWSTR},
    Win32::{
        Foundation::{CloseHandle, LocalFree, HANDLE, HLOCAL},
        Security::{
            Authorization::{
                ConvertSidToStringSidW, ConvertStringSidToSidW, GetNamedSecurityInfoW,
                SetEntriesInAclW, SetNamedSecurityInfoW, EXPLICIT_ACCESS_W, SET_ACCESS,
                SE_FILE_OBJECT, TRUSTEE_IS_GROUP, TRUSTEE_IS_SID, TRUSTEE_IS_USER, TRUSTEE_W,
            },
            ACE_FLAGS, ACL, CONTAINER_INHERIT_ACE, DACL_SECURITY_INFORMATION, NO_INHERITANCE,
            OBJECT_INHERIT_ACE, PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
            TOKEN_QUERY, TOKEN_USER,
        },
        Storage::FileSystem::{FILE_ALL_ACCESS, FILE_GENERIC_WRITE},
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    },
};

const FILE_ATTRIBUTE_REPARSE_POINT_U32: u32 = 0x400;

#[inline]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    (metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT_U32) != 0
}

fn apply_grant_sid_allow_ace_to_path(
    path: &Path,
    sid_ptr: *mut std::ffi::c_void,
    access_mask: u32,
    is_group: bool,
    is_dir: bool,
) -> ResultType<()> {
    let mut old_dacl: *mut ACL = std::ptr::null_mut();
    let mut security_descriptor = PSECURITY_DESCRIPTOR::default();
    let path_utf16: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let get_named_result = unsafe {
        GetNamedSecurityInfoW(
            PCWSTR::from_raw(path_utf16.as_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(&mut old_dacl),
            None,
            &mut security_descriptor,
        )
    };
    if get_named_result.0 != 0 {
        bail!(
            "GetNamedSecurityInfoW failed for '{}': win32_error={}",
            path.display(),
            get_named_result.0
        );
    }
    let _sd_guard = LocalAllocGuard(security_descriptor.0);

    let inherit_flags = if is_dir {
        ACE_FLAGS(OBJECT_INHERIT_ACE.0 | CONTAINER_INHERIT_ACE.0)
    } else {
        NO_INHERITANCE
    };
    let explicit_access = [make_sid_trustee_entry(
        sid_ptr,
        access_mask,
        inherit_flags,
        is_group,
    )];
    let old_acl_option = if old_dacl.is_null() {
        None
    } else {
        Some(old_dacl as *const ACL)
    };
    let mut new_acl: *mut ACL = std::ptr::null_mut();
    let set_entries_result = unsafe {
        SetEntriesInAclW(
            Some(explicit_access.as_slice()),
            old_acl_option,
            &mut new_acl,
        )
    };
    if set_entries_result.0 != 0 {
        bail!(
            "SetEntriesInAclW failed for '{}': win32_error={}",
            path.display(),
            set_entries_result.0
        );
    }
    if new_acl.is_null() {
        bail!(
            "SetEntriesInAclW returned null ACL for '{}'",
            path.display()
        );
    }
    let _acl_guard = LocalAllocGuard(new_acl as *mut std::ffi::c_void);

    let set_named_result = unsafe {
        SetNamedSecurityInfoW(
            PCWSTR::from_raw(path_utf16.as_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(new_acl),
            None,
        )
    };
    if set_named_result.0 != 0 {
        bail!(
            "SetNamedSecurityInfoW failed for '{}': win32_error={}",
            path.display(),
            set_named_result.0
        );
    }
    Ok(())
}

pub fn set_path_permission(dir: &Path, access_mask: u32) -> ResultType<()> {
    let metadata = fs::symlink_metadata(dir).map_err(|e| {
        anyhow!(
            "Failed to inspect ACL target directory '{}': {}",
            dir.display(),
            e
        )
    })?;
    if is_reparse_point(&metadata) {
        bail!(
            "ACL target directory is a reparse point and is rejected: '{}'",
            dir.display()
        );
    }
    if !metadata.file_type().is_dir() {
        bail!("ACL target is not a directory: '{}'", dir.display());
    }

    let everyone_sid = sid_string_to_local_alloc_guard("S-1-1-0")?;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(path) = stack.pop() {
        let metadata = fs::symlink_metadata(&path)
            .map_err(|e| anyhow!("Failed to inspect ACL target '{}': {}", path.display(), e))?;
        if is_reparse_point(&metadata) {
            continue;
        }
        let is_dir = metadata.file_type().is_dir();
        apply_grant_sid_allow_ace_to_path(
            &path,
            everyone_sid.as_sid_ptr(),
            access_mask,
            true,
            is_dir,
        )?;
        if !is_dir {
            continue;
        }
        for entry in fs::read_dir(&path)
            .map_err(|e| anyhow!("Failed to list ACL target dir '{}': {}", path.display(), e))?
        {
            let entry = entry.map_err(|e| {
                anyhow!(
                    "Failed to read ACL target dir entry under '{}': {}",
                    path.display(),
                    e
                )
            })?;
            stack.push(entry.path());
        }
    }
    Ok(())
}

pub(crate) fn current_process_user_sid_string() -> ResultType<String> {
    let mut token = HANDLE::default();
    let result = (|| -> ResultType<String> {
        unsafe {
            OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)
                .map_err(|e| anyhow!("Failed to open current process token: {}", e))?;
        }

        let buffer = unsafe { read_token_user_buffer(token, "current process")? };
        let token_user: TOKEN_USER =
            unsafe { std::ptr::read_unaligned(buffer.as_ptr() as *const TOKEN_USER) };
        if token_user.User.Sid.0.is_null() {
            bail!("Token SID is null");
        }

        let mut sid_string_ptr = PWSTR::null();
        unsafe {
            ConvertSidToStringSidW(token_user.User.Sid, &mut sid_string_ptr).map_err(|e| {
                anyhow!(
                    "ConvertSidToStringSidW failed for current process token SID: {}",
                    e
                )
            })?;
        }
        if sid_string_ptr.is_null() {
            bail!("ConvertSidToStringSidW returned null SID string pointer");
        }
        let _sid_string_guard = LocalAllocGuard(sid_string_ptr.0 as *mut std::ffi::c_void);
        unsafe {
            sid_string_ptr
                .to_string()
                .map_err(|e| anyhow!("Failed to decode SID string as UTF-16: {}", e))
        }
    })();

    if !token.is_invalid() {
        unsafe {
            let _ = CloseHandle(token);
        }
    }
    result
}

pub fn set_path_permission_for_portable_service_shmem_dir(path: &Path) -> ResultType<()> {
    set_path_permission_for_portable_service_shmem_impl(path, true)
}

#[inline]
pub fn validate_path_for_portable_service_shmem_dir(path: &Path) -> ResultType<()> {
    validate_portable_service_shmem_dir_target(path)
}

#[inline]
pub fn set_path_permission_for_portable_service_shmem_file(path: &Path) -> ResultType<()> {
    set_path_permission_for_portable_service_shmem_impl(path, false)
}

#[derive(Debug)]
pub(super) struct LocalAllocGuard(*mut std::ffi::c_void);

impl LocalAllocGuard {
    #[inline]
    pub(super) fn as_sid_ptr(&self) -> *mut std::ffi::c_void {
        self.0
    }
}

impl Drop for LocalAllocGuard {
    fn drop(&mut self) {
        if self.0.is_null() {
            return;
        }
        unsafe {
            let _ = LocalFree(Some(HLOCAL(self.0)));
        }
    }
}

#[inline]
pub(super) fn sid_string_to_local_alloc_guard(sid: &str) -> ResultType<LocalAllocGuard> {
    let sid_utf16 = wide_string(sid);
    let mut sid_ptr = PSID::default();
    unsafe {
        ConvertStringSidToSidW(PCWSTR::from_raw(sid_utf16.as_ptr()), &mut sid_ptr)
            .map_err(|e| anyhow!("ConvertStringSidToSidW failed for '{}': {}", sid, e))?;
    }
    if sid_ptr.0.is_null() {
        bail!("ConvertStringSidToSidW returned null SID for '{}'", sid);
    }
    Ok(LocalAllocGuard(sid_ptr.0))
}

#[inline]
fn make_sid_trustee_entry(
    sid_ptr: *mut std::ffi::c_void,
    access_permissions: u32,
    inheritance: ACE_FLAGS,
    is_group: bool,
) -> EXPLICIT_ACCESS_W {
    EXPLICIT_ACCESS_W {
        grfAccessPermissions: access_permissions,
        grfAccessMode: SET_ACCESS,
        grfInheritance: inheritance,
        Trustee: TRUSTEE_W {
            pMultipleTrustee: std::ptr::null_mut(),
            MultipleTrusteeOperation: Default::default(),
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: if is_group {
                TRUSTEE_IS_GROUP
            } else {
                TRUSTEE_IS_USER
            },
            ptstrName: PWSTR::from_raw(sid_ptr as *mut u16),
        },
    }
}

fn validate_portable_service_shmem_dir_target(path: &Path) -> ResultType<()> {
    let metadata = fs::symlink_metadata(path).map_err(|e| {
        anyhow!(
            "Failed to inspect portable service shared-memory ACL directory '{}': {}",
            path.display(),
            e
        )
    })?;
    if is_reparse_point(&metadata) {
        bail!(
            "Portable service shared-memory ACL directory target is a reparse point and is rejected: '{}'",
            path.display()
        );
    }
    if !metadata.file_type().is_dir() {
        bail!(
            "Portable service shared-memory ACL target is not a directory: '{}'",
            path.display()
        );
    }
    Ok(())
}

fn set_path_permission_for_portable_service_shmem_impl(
    path: &Path,
    expect_dir: bool,
) -> ResultType<()> {
    if expect_dir {
        validate_portable_service_shmem_dir_target(path)?;
    } else {
        let metadata_result = fs::symlink_metadata(path);
        match metadata_result {
            Ok(metadata) => {
                if metadata.file_type().is_dir() {
                    bail!(
                        "Portable service shared-memory ACL target is a directory, expected file-like path: '{}'",
                        path.display()
                    );
                }
                if is_reparse_point(&metadata) {
                    bail!(
                        "Portable service shared-memory ACL file target is a reparse point and is rejected: '{}'",
                        path.display()
                    );
                }
            }
            Err(e)
                if e.kind() == io::ErrorKind::NotFound
                    || e.kind() == io::ErrorKind::PermissionDenied =>
            {
            }
            Err(e) => {
                bail!(
                    "Failed to inspect portable service shared-memory ACL target '{}': {}",
                    path.display(),
                    e
                );
            }
        }
    }

    let user_sid = current_process_user_sid_string()?;
    let local_system_sid = sid_string_to_local_alloc_guard("S-1-5-18")?;
    let administrators_sid = sid_string_to_local_alloc_guard("S-1-5-32-544")?;
    let current_user_sid = sid_string_to_local_alloc_guard(&user_sid)?;
    let authenticated_users_sid = if expect_dir {
        Some(sid_string_to_local_alloc_guard("S-1-5-11")?)
    } else {
        None
    };

    let inherit_flags = if expect_dir {
        ACE_FLAGS(OBJECT_INHERIT_ACE.0 | CONTAINER_INHERIT_ACE.0)
    } else {
        NO_INHERITANCE
    };
    let mut entries = vec![
        make_sid_trustee_entry(
            local_system_sid.as_sid_ptr(),
            FILE_ALL_ACCESS.0,
            inherit_flags,
            false,
        ),
        make_sid_trustee_entry(
            administrators_sid.as_sid_ptr(),
            FILE_ALL_ACCESS.0,
            inherit_flags,
            true,
        ),
        make_sid_trustee_entry(
            current_user_sid.as_sid_ptr(),
            FILE_ALL_ACCESS.0,
            inherit_flags,
            false,
        ),
    ];
    if let Some(auth_sid) = authenticated_users_sid.as_ref() {
        entries.push(make_sid_trustee_entry(
            auth_sid.as_sid_ptr(),
            FILE_GENERIC_WRITE.0,
            NO_INHERITANCE,
            true,
        ));
    }

    let mut new_acl: *mut ACL = std::ptr::null_mut();
    let set_entries_result =
        unsafe { SetEntriesInAclW(Some(entries.as_slice()), None, &mut new_acl) };
    if set_entries_result.0 != 0 {
        bail!(
            "SetEntriesInAclW failed for '{}': win32_error={}",
            path.display(),
            set_entries_result.0
        );
    }
    if new_acl.is_null() {
        bail!(
            "SetEntriesInAclW returned null ACL for '{}'",
            path.display()
        );
    }
    let _acl_guard = LocalAllocGuard(new_acl as *mut std::ffi::c_void);

    let path_utf16: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let security_info = DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION;
    let set_named_result = unsafe {
        SetNamedSecurityInfoW(
            PCWSTR::from_raw(path_utf16.as_ptr()),
            SE_FILE_OBJECT,
            security_info,
            None,
            None,
            Some(new_acl),
            None,
        )
    };
    if set_named_result.0 != 0 {
        bail!(
            "SetNamedSecurityInfoW failed for '{}': win32_error={}",
            path.display(),
            set_named_result.0
        );
    }
    Ok(())
}
