//! Least-privilege permissions for Hanni's local sensitive files.
//!
//! Windows security changes are performed against one open handle. This avoids
//! resolving a checked path a second time and applying an ACL to a swapped
//! target. Reparse points are rejected rather than followed.

use std::ffi::OsStr;
use std::io;
use std::path::Path;

pub fn ensure_private_dir(path: &Path) -> io::Result<()> {
    #[cfg(windows)]
    match std::fs::create_dir(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    #[cfg(not(windows))]
    std::fs::create_dir_all(path)?;
    restrict_dir(path)
}

pub fn restrict_file(path: &Path) -> io::Result<()> {
    apply(path, false)
}

pub fn restrict_dir(path: &Path) -> io::Result<()> {
    apply(path, true)
}

pub fn restrict_file_if_present(path: &Path) -> io::Result<()> {
    repair_if_present(path, false)
}

/// Repair only known sensitive paths. Do not recursively rewrite the complete
/// data directory: it also contains models, web assets and user media.
pub fn startup_repair(data_dir: &Path) -> io::Result<()> {
    restrict_dir(data_dir)?;

    for name in [
        "hanni.db",
        "hanni.db-wal",
        "hanni.db-shm",
        "api_token.txt",
        "jobs_api_token.txt",
        "updater.log",
    ] {
        repair_if_present(&data_dir.join(name), false)?;
    }

    let backup_dir = data_dir.join("backups");
    repair_if_present(&backup_dir, true)?;
    let entries = match std::fs::read_dir(&backup_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        if is_managed_backup_name(&entry.file_name()) {
            repair_if_present(&entry.path(), false)?;
        }
    }
    Ok(())
}

fn is_managed_backup_name(name: &OsStr) -> bool {
    let Some(name) = name.to_str() else {
        return false;
    };
    let Some(suffix) = name.strip_prefix("hanni_") else {
        return false;
    };
    [".db", ".db-wal", ".db-shm"].iter().any(|extension| {
        suffix
            .strip_suffix(extension)
            .is_some_and(|stem| !stem.is_empty())
    })
}

#[cfg(windows)]
fn apply(path: &Path, directory: bool) -> io::Result<()> {
    windows_acl::apply(path, directory)
}

#[cfg(windows)]
fn repair_if_present(path: &Path, directory: bool) -> io::Result<()> {
    windows_acl::repair_if_present(path, directory)
}

#[cfg(unix)]
fn apply(path: &Path, directory: bool) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "refusing to secure a symbolic link",
        ));
    }
    if metadata.is_dir() != directory {
        let expected = if directory { "directory" } else { "file" };
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("secure path is not a {expected}"),
        ));
    }
    let mode = if directory { 0o700 } else { 0o600 };
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
}

#[cfg(unix)]
fn repair_if_present(path: &Path, directory: bool) -> io::Result<()> {
    match apply(path, directory) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

#[cfg(not(any(unix, windows)))]
fn apply(path: &Path, directory: bool) -> io::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.is_dir() != directory {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "secure path has the wrong object type",
        ));
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn repair_if_present(path: &Path, directory: bool) -> io::Result<()> {
    match apply(path, directory) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

#[cfg(windows)]
mod windows_acl {
    use std::io;
    use std::mem::size_of;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use std::ptr;

    use windows::{
        core::{Error as WindowsError, Owned, BOOL, HRESULT, PCWSTR, PWSTR},
        Win32::{
            Foundation::{
                ERROR_FILE_NOT_FOUND, ERROR_INSUFFICIENT_BUFFER, ERROR_PATH_NOT_FOUND, HANDLE,
                HLOCAL,
            },
            Security::{
                Authorization::{
                    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
                    GetSecurityInfo, SetSecurityInfo, SDDL_REVISION_1, SE_FILE_OBJECT,
                },
                GetSecurityDescriptorDacl, GetTokenInformation, IsValidSecurityDescriptor,
                TokenUser, ACL, DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION,
                PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID, TOKEN_QUERY,
                TOKEN_USER,
            },
            Storage::FileSystem::{
                CreateFileW, FileAttributeTagInfo, GetFileInformationByHandleEx,
                FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT, FILE_ATTRIBUTE_TAG_INFO,
                FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES,
                FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, READ_CONTROL,
                WRITE_DAC,
            },
            System::Threading::{GetCurrentProcess, OpenProcessToken},
        },
    };

    fn windows_error(context: &str, error: WindowsError) -> io::Error {
        io::Error::new(io::ErrorKind::Other, format!("{context}: {error}"))
    }

    fn not_found(error: &WindowsError) -> bool {
        error.code() == HRESULT::from_win32(ERROR_FILE_NOT_FOUND.0)
            || error.code() == HRESULT::from_win32(ERROR_PATH_NOT_FOUND.0)
    }

    fn wide_path(path: &Path) -> io::Result<Vec<u16>> {
        let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
        if wide.contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "secure path contains an embedded NUL",
            ));
        }
        wide.push(0);
        Ok(wide)
    }

    fn open_wide(wide: &[u16]) -> Result<Owned<HANDLE>, WindowsError> {
        open_wide_with_share(wide, false)
    }

    fn open_wide_with_share(
        wide: &[u16],
        allow_name_swap: bool,
    ) -> Result<Owned<HANDLE>, WindowsError> {
        let access = (FILE_READ_ATTRIBUTES | READ_CONTROL | WRITE_DAC).0;
        let flags = FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS;
        let mut share = FILE_SHARE_READ | FILE_SHARE_WRITE;
        if allow_name_swap {
            share |= FILE_SHARE_DELETE;
        }
        let handle = unsafe {
            CreateFileW(
                PCWSTR(wide.as_ptr()),
                access,
                share,
                None,
                OPEN_EXISTING,
                flags,
                None,
            )
        }?;
        Ok(unsafe { Owned::new(handle) })
    }

    fn open_object(path: &Path) -> io::Result<Owned<HANDLE>> {
        let wide = wide_path(path)?;
        open_wide(&wide).map_err(|error| windows_error("open secure path", error))
    }

    fn validate_object(handle: HANDLE, expected_directory: bool) -> io::Result<()> {
        let mut info = FILE_ATTRIBUTE_TAG_INFO::default();
        unsafe {
            GetFileInformationByHandleEx(
                handle,
                FileAttributeTagInfo,
                ptr::addr_of_mut!(info).cast(),
                size_of::<FILE_ATTRIBUTE_TAG_INFO>() as u32,
            )
        }
        .map_err(|error| windows_error("inspect secure path", error))?;

        if info.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "refusing to secure a reparse point",
            ));
        }
        let actual_directory = info.FileAttributes & FILE_ATTRIBUTE_DIRECTORY.0 != 0;
        if actual_directory != expected_directory {
            let expected = if expected_directory {
                "directory"
            } else {
                "file"
            };
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("secure path is not a {expected}"),
            ));
        }
        Ok(())
    }

    fn sid_to_string(sid: PSID) -> io::Result<String> {
        let mut string_sid = PWSTR(ptr::null_mut());
        unsafe { ConvertSidToStringSidW(sid, &mut string_sid) }
            .map_err(|error| windows_error("format SID", error))?;
        let _allocation = unsafe { Owned::<HLOCAL>::new(HLOCAL(string_sid.0.cast())) };
        unsafe { string_sid.to_string() }
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }

    fn current_user_sid() -> io::Result<String> {
        let mut raw_token = HANDLE::default();
        unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw_token) }
            .map_err(|error| windows_error("open process token", error))?;
        let token = unsafe { Owned::new(raw_token) };

        let mut required = 0u32;
        let first = unsafe { GetTokenInformation(*token, TokenUser, None, 0, &mut required) };
        let first_error = first.err().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "TokenUser sizing call unexpectedly succeeded",
            )
        })?;
        if first_error.code() != HRESULT::from_win32(ERROR_INSUFFICIENT_BUFFER.0) || required == 0 {
            return Err(windows_error("size TokenUser", first_error));
        }

        // TOKEN_USER needs pointer alignment; Vec<usize> provides it.
        let word_size = size_of::<usize>();
        let words = (required as usize + word_size - 1) / word_size;
        let mut buffer = vec![0usize; words];
        unsafe {
            GetTokenInformation(
                *token,
                TokenUser,
                Some(buffer.as_mut_ptr().cast()),
                required,
                &mut required,
            )
        }
        .map_err(|error| windows_error("read TokenUser", error))?;

        let token_user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
        sid_to_string(token_user.User.Sid)
    }

    fn object_owner_sid(handle: HANDLE) -> io::Result<String> {
        let mut owner = PSID::default();
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        unsafe {
            GetSecurityInfo(
                handle,
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION,
                Some(&mut owner),
                None,
                None,
                None,
                Some(&mut descriptor),
            )
        }
        .ok()
        .map_err(|error| windows_error("read secure path owner", error))?;
        let _descriptor = unsafe { Owned::<HLOCAL>::new(HLOCAL(descriptor.0)) };
        if owner.is_invalid() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "secure path has no owner",
            ));
        }
        sid_to_string(owner)
    }

    fn apply_descriptor_to_handle(handle: HANDLE, directory: bool) -> io::Result<()> {
        let user_sid = current_user_sid()?;
        if object_owner_sid(handle)? != user_sid {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "secure path owner is not the current process user",
            ));
        }
        let sddl = if directory {
            format!(
                "D:P(A;OICI;FA;;;{0})(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)",
                user_sid
            )
        } else {
            format!("D:P(A;;FA;;;{0})(A;;FA;;;SY)(A;;FA;;;BA)", user_sid)
        };
        let mut wide: Vec<u16> = sddl.encode_utf16().collect();
        wide.push(0);

        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(wide.as_ptr()),
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )
        }
        .map_err(|error| windows_error("build security descriptor", error))?;
        let _descriptor = unsafe { Owned::<HLOCAL>::new(HLOCAL(descriptor.0)) };

        if !unsafe { IsValidSecurityDescriptor(descriptor) }.as_bool() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "generated security descriptor is invalid",
            ));
        }

        let mut dacl_present = BOOL(0);
        let mut dacl_defaulted = BOOL(0);
        let mut dacl: *mut ACL = ptr::null_mut();
        unsafe {
            GetSecurityDescriptorDacl(
                descriptor,
                &mut dacl_present,
                &mut dacl,
                &mut dacl_defaulted,
            )
        }
        .map_err(|error| windows_error("read descriptor DACL", error))?;
        if !dacl_present.as_bool() || dacl.is_null() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "generated security descriptor has no DACL",
            ));
        }

        unsafe {
            SetSecurityInfo(
                handle,
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                None,
                None,
                Some(dacl.cast_const()),
                None,
            )
        }
        .ok()
        .map_err(|error| windows_error("apply secure DACL", error))?;
        Ok(())
    }

    pub(super) fn apply(path: &Path, directory: bool) -> io::Result<()> {
        let handle = open_object(path)?;
        validate_object(*handle, directory)?;
        apply_descriptor_to_handle(*handle, directory)
    }

    pub(super) fn repair_if_present(path: &Path, directory: bool) -> io::Result<()> {
        let wide = wide_path(path)?;
        let handle = match open_wide(&wide) {
            Ok(handle) => handle,
            Err(error) if not_found(&error) => return Ok(()),
            Err(error) => return Err(windows_error("open repair path", error)),
        };
        validate_object(*handle, directory)?;
        apply_descriptor_to_handle(*handle, directory)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use windows::Win32::Security::Authorization::ConvertSecurityDescriptorToStringSecurityDescriptorW;

        fn descriptor_sddl(handle: HANDLE) -> io::Result<String> {
            let mut descriptor = PSECURITY_DESCRIPTOR::default();
            unsafe {
                GetSecurityInfo(
                    handle,
                    SE_FILE_OBJECT,
                    OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                    None,
                    None,
                    None,
                    None,
                    Some(&mut descriptor),
                )
            }
            .ok()
            .map_err(|error| windows_error("read applied descriptor", error))?;
            let _descriptor = unsafe { Owned::<HLOCAL>::new(HLOCAL(descriptor.0)) };

            let mut string = PWSTR(ptr::null_mut());
            unsafe {
                ConvertSecurityDescriptorToStringSecurityDescriptorW(
                    descriptor,
                    SDDL_REVISION_1,
                    OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                    &mut string,
                    None,
                )
            }
            .map_err(|error| windows_error("format applied descriptor", error))?;
            let _string = unsafe { Owned::<HLOCAL>::new(HLOCAL(string.0.cast())) };
            unsafe { string.to_string() }
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
        }

        fn assert_restricted(path: &Path, directory: bool) {
            let handle = open_object(path).expect("open secured object");
            validate_object(*handle, directory).expect("validate secured object");
            let sddl = descriptor_sddl(*handle).expect("read secured descriptor");
            let sid = current_user_sid().expect("current user SID");
            assert_eq!(object_owner_sid(*handle).expect("object owner SID"), sid);
            assert!(sddl.contains("D:P"), "{sddl}");
            assert_eq!(sddl.matches("(A;").count(), 3, "{sddl}");
            let flags = if directory { "OICI" } else { "" };
            assert!(sddl.contains(&format!("(A;{flags};FA;;;{sid})")), "{sddl}");
            assert!(
                sddl.contains(&format!("(A;{flags};FA;;;SY)"))
                    || sddl.contains(&format!("(A;{flags};FA;;;S-1-5-18)")),
                "{sddl}"
            );
            assert!(
                sddl.contains(&format!("(A;{flags};FA;;;BA)"))
                    || sddl.contains(&format!("(A;{flags};FA;;;S-1-5-32-544)")),
                "{sddl}"
            );
        }

        #[test]
        fn secures_file_and_directory_handles() {
            let temp = tempfile::tempdir().expect("temp dir");
            let file = temp.path().join("secret.txt");
            std::fs::write(&file, b"synthetic secret").expect("write fixture");

            super::apply(&file, false).expect("secure file");
            assert_restricted(&file, false);
            super::apply(temp.path(), true).expect("secure directory");
            assert_restricted(temp.path(), true);
        }

        #[test]
        fn rejects_wrong_object_kind() {
            let temp = tempfile::tempdir().expect("temp dir");
            let file = temp.path().join("secret.txt");
            std::fs::write(&file, b"fixture").expect("write fixture");

            assert_eq!(
                super::apply(temp.path(), false).unwrap_err().kind(),
                io::ErrorKind::InvalidInput
            );
            assert_eq!(
                super::apply(&file, true).unwrap_err().kind(),
                io::ErrorKind::InvalidInput
            );
        }

        #[test]
        fn rejects_file_symlink_without_touching_target() {
            use std::os::windows::fs::symlink_file;

            let temp = tempfile::tempdir().expect("temp dir");
            let target = temp.path().join("target.txt");
            let link = temp.path().join("link.txt");
            std::fs::write(&target, b"sentinel").expect("write target");
            if let Err(error) = symlink_file(&target, &link) {
                if error.raw_os_error() == Some(1314) {
                    eprintln!("symlink privilege unavailable; reparse test not executed");
                    return;
                }
                panic!("create symlink: {error}");
            }

            assert_eq!(
                super::apply(&link, false).unwrap_err().kind(),
                io::ErrorKind::InvalidInput
            );
            assert_eq!(std::fs::read(&target).expect("read target"), b"sentinel");
        }

        #[test]
        fn rejects_broken_file_symlink_without_creating_target() {
            use std::os::windows::fs::symlink_file;

            let temp = tempfile::tempdir().expect("temp dir");
            let missing = temp.path().join("missing-target.txt");
            let link = temp.path().join("api_token.txt");
            if let Err(error) = symlink_file(&missing, &link) {
                if error.raw_os_error() == Some(1314) {
                    eprintln!("symlink privilege unavailable; broken-link test not executed");
                    return;
                }
                panic!("create symlink: {error}");
            }

            assert_eq!(
                super::super::restrict_file_if_present(&link)
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::InvalidInput
            );
            assert!(!missing.exists());
        }

        #[test]
        fn open_handle_blocks_name_swap_before_acl_write() {
            let temp = tempfile::tempdir().expect("temp dir");
            let original = temp.path().join("original.txt");
            let moved = temp.path().join("moved.txt");
            std::fs::write(&original, b"sentinel").expect("write fixture");

            let handle = open_object(&original).expect("open original");
            validate_object(*handle, false).expect("validate original");
            assert!(std::fs::rename(&original, &moved).is_err());
            apply_descriptor_to_handle(*handle, false).expect("apply by handle");
            drop(handle);
            assert_restricted(&original, false);
        }

        #[test]
        fn same_handle_acl_does_not_follow_a_swapped_name() {
            let temp = tempfile::tempdir().expect("temp dir");
            let original = temp.path().join("original.txt");
            let moved = temp.path().join("moved.txt");
            std::fs::write(&original, b"original").expect("write original");

            let wide = wide_path(&original).expect("wide path");
            let handle = open_wide_with_share(&wide, true).expect("open swappable object");
            validate_object(*handle, false).expect("validate original");
            std::fs::rename(&original, &moved).expect("rename original");
            std::fs::write(&original, b"replacement").expect("write replacement");

            let replacement = open_object(&original).expect("open replacement");
            let replacement_before =
                descriptor_sddl(*replacement).expect("read replacement descriptor");
            drop(replacement);

            apply_descriptor_to_handle(*handle, false).expect("secure original handle");
            drop(handle);

            assert_restricted(&moved, false);
            let replacement = open_object(&original).expect("reopen replacement");
            let replacement_after =
                descriptor_sddl(*replacement).expect("reread replacement descriptor");
            assert_eq!(replacement_after, replacement_before);
        }

        #[test]
        fn startup_repair_is_bounded_to_managed_paths() {
            let temp = tempfile::tempdir().expect("temp dir");
            let database = temp.path().join("hanni.db");
            let unknown = temp.path().join("user-media.bin");
            std::fs::write(&database, b"synthetic database").expect("write database");
            std::fs::write(&unknown, b"sentinel").expect("write unknown file");

            super::super::startup_repair(temp.path()).expect("startup repair");

            assert_restricted(temp.path(), true);
            assert_restricted(&database, false);
            assert_eq!(std::fs::read(&unknown).expect("read unknown"), b"sentinel");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_allowlist_is_bounded() {
        for name in [
            "hanni_20260901_010203.db",
            "hanni_20260901_010203.db-wal",
            "hanni_20260901_010203.db-shm",
        ] {
            assert!(is_managed_backup_name(OsStr::new(name)), "{name}");
        }
        for name in [
            "hanni_.db",
            "other.db",
            "hanni_20260901.json",
            "hanni_20260901.db.exe",
        ] {
            assert!(!is_managed_backup_name(OsStr::new(name)), "{name}");
        }
    }
}
