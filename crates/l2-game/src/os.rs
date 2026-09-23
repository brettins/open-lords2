//! The three things only Windows can answer. [D] ours: the original's own
//! message boxes are its DirectDraw failures, not these.

#[cfg(windows)]
mod imp {
    use std::os::windows::ffi::OsStrExt;
    use std::path::PathBuf;
    use windows_sys::Win32::System::Com::{CoInitializeEx, CoTaskMemFree, COINIT_APARTMENTTHREADED};
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
    use windows_sys::Win32::UI::Shell::{BIF_NEWDIALOGSTYLE, BIF_RETURNONLYFSDIRS};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, IDOK, MB_ICONERROR, MB_ICONINFORMATION, MB_OK, MB_OKCANCEL,
    };

    // windows-sys 0.52 gates these behind a feature it does not publish.
    #[allow(non_snake_case)]
    #[repr(C)]
    struct BROWSEINFOW {
        hwndOwner: isize,
        pidlRoot: *const core::ffi::c_void,
        pszDisplayName: *mut u16,
        lpszTitle: *const u16,
        ulFlags: u32,
        lpfn: Option<unsafe extern "system" fn(isize, u32, isize, isize) -> i32>,
        lParam: isize,
        iImage: i32,
    }

    #[link(name = "shell32")]
    extern "system" {
        fn SHBrowseForFolderW(bi: *const BROWSEINFOW) -> *mut core::ffi::c_void;
        fn SHGetPathFromIDListW(pidl: *const core::ffi::c_void, path: *mut u16) -> i32;
    }

    fn wide(s: &str) -> Vec<u16> {
        std::ffi::OsStr::new(s).encode_wide().chain([0]).collect()
    }

    fn from_wide(b: &[u16]) -> String {
        String::from_utf16_lossy(&b[..b.iter().position(|c| *c == 0).unwrap_or(b.len())])
    }

    pub fn error(title: &str, text: &str) {
        let (t, c) = (wide(text), wide(title));
        unsafe { MessageBoxW(0, t.as_ptr(), c.as_ptr(), MB_OK | MB_ICONERROR) };
    }

    pub fn ask(title: &str, text: &str) -> bool {
        let (t, c) = (wide(text), wide(title));
        unsafe { MessageBoxW(0, t.as_ptr(), c.as_ptr(), MB_OKCANCEL | MB_ICONINFORMATION) == IDOK }
    }

    pub fn pick_folder(title: &str) -> Option<PathBuf> {
        let t = wide(title);
        let mut name = [0u16; 260];
        let info = BROWSEINFOW {
            hwndOwner: 0,
            pidlRoot: std::ptr::null(),
            pszDisplayName: name.as_mut_ptr(),
            lpszTitle: t.as_ptr(),
            ulFlags: BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE,
            lpfn: None,
            lParam: 0,
            iImage: 0,
        };
        unsafe {
            CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32);
            let pidl = SHBrowseForFolderW(&info);
            if pidl.is_null() {
                return None;
            }
            let mut path = [0u16; 1024];
            let ok = SHGetPathFromIDListW(pidl, path.as_mut_ptr());
            CoTaskMemFree(pidl as *const _);
            (ok != 0).then(|| PathBuf::from(from_wide(&path)))
        }
    }

    pub fn registry_path(key: &str, value: &str) -> Option<PathBuf> {
        let (k, v) = (wide(key), wide(value));
        let mut buf = [0u16; 1024];
        let mut len = (buf.len() * 2) as u32;
        let rc = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                k.as_ptr(),
                v.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buf.as_mut_ptr().cast(),
                &mut len,
            )
        };
        (rc == 0).then(|| PathBuf::from(from_wide(&buf)))
    }
}

#[cfg(not(windows))]
mod imp {
    use std::path::PathBuf;
    pub fn error(title: &str, text: &str) {
        eprintln!("{title}: {text}");
    }
    pub fn ask(_: &str, _: &str) -> bool {
        false
    }
    pub fn pick_folder(_: &str) -> Option<PathBuf> {
        None
    }
    pub fn registry_path(_: &str, _: &str) -> Option<PathBuf> {
        None
    }
}

pub(crate) use imp::*;
