//! ドライブの累積読み書きバイト数(`IOCTL_DISK_PERFORMANCE`)を取得する。
//!
//! Windowsのパフォーマンスカウンタ(PDH API)はwindows-sysクレートに含まれて
//! いないため、より低レベルなIOCTLを直接叩く。取得できるのは累積値なので、
//! 呼び出し側で前回ポーリングとの差分・経過時間からMB/sを算出する。
//! 取得に失敗した場合は素直にNoneを返す(管理者権限が必要な環境等でも
//! アプリ全体が壊れないようにするため)。

#![cfg(windows)]

use std::ffi::{c_void, OsStr};
use std::os::windows::ffi::OsStrExt;
use std::ptr::null_mut;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING};
use windows_sys::Win32::System::Ioctl::{DISK_PERFORMANCE, IOCTL_DISK_PERFORMANCE};
use windows_sys::Win32::System::IO::DeviceIoControl;

fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

/// `drive_letter`(例: 'C')の累積(読み込みバイト数, 書き込みバイト数)を返す。
pub fn query_disk_counters(drive_letter: char) -> Option<(i64, i64)> {
    let path = format!(r"\\.\{drive_letter}:");
    let wide = to_wide(&path);

    unsafe {
        let handle: HANDLE = CreateFileW(
            wide.as_ptr(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            null_mut(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        );
        if handle == INVALID_HANDLE_VALUE {
            return None;
        }

        let mut perf: DISK_PERFORMANCE = std::mem::zeroed();
        let mut bytes_returned: u32 = 0;
        let ok = DeviceIoControl(
            handle,
            IOCTL_DISK_PERFORMANCE,
            null_mut(),
            0,
            &mut perf as *mut _ as *mut c_void,
            std::mem::size_of::<DISK_PERFORMANCE>() as u32,
            &mut bytes_returned,
            null_mut(),
        );
        CloseHandle(handle);

        if ok == 0 {
            return None;
        }
        Some((perf.BytesRead, perf.BytesWritten))
    }
}
