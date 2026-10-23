//! ペットがOSから取るもの(詩織Ver6.0の段階4。段階0の試作2・3から移した)。
//! 最後の入力からの秒数、マウスの左ボタンの状態、前面のウィンドウが全画面か。

/// 最後のマウス・キーボードの入力からの秒数。取れなければNone。
#[cfg(windows)]
pub fn idle_seconds() -> Option<f64> {
    use windows_sys::Win32::System::SystemInformation::GetTickCount;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    let mut info = LASTINPUTINFO { cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
    // SAFETY: cbSizeを設定した構造体を渡す
    if unsafe { GetLastInputInfo(&mut info) } == 0 {
        return None;
    }
    // どちらも起動からのミリ秒(約49日で一周する)。wrapping_subで一周をまたいでも正しく引ける
    let now = unsafe { GetTickCount() };
    Some(now.wrapping_sub(info.dwTime) as f64 / 1000.0)
}

/// マウスの左ボタンが押されているか(ウィンドウの外で離すと画面のmouseupが届かないため、OSで見る)。
#[cfg(windows)]
pub fn left_button_down() -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
    // SAFETY: 引数は仮想キーコードだけ
    (unsafe { GetAsyncKeyState(VK_LBUTTON as i32) } as u16 & 0x8000) != 0
}

/// 前面のウィンドウが、渡した画面(物理の画素の左上と大きさ)を全部覆っているか。
/// デスクトップ(壁紙)そのものは、画面と同じ大きさでも全画面のアプリとはみなさない。
#[cfg(windows)]
pub fn foreground_covers(x: i32, y: i32, w: u32, h: u32) -> bool {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetClassNameW, GetForegroundWindow, GetWindowRect};
    // SAFETY: 前面のウィンドウを聞くだけ
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_null() {
        return false;
    }
    let mut class = [0u16; 64];
    // SAFETY: 長さを渡した配列に書かせる
    let n = unsafe { GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32) };
    let class = String::from_utf16_lossy(&class[..n.max(0) as usize]);
    if class == "Progman" || class == "WorkerW" || class == "Shell_TrayWnd" {
        return false;
    }
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    // SAFETY: 書き込み先の構造体を渡す
    if unsafe { GetWindowRect(hwnd, &mut r) } == 0 {
        return false;
    }
    r.left <= x && r.top <= y && r.right >= x + w as i32 && r.bottom >= y + h as i32
}

#[cfg(target_os = "macos")]
mod cg {
    // kCGEventSourceStateCombinedSessionState
    pub const COMBINED_SESSION_STATE: i32 = 0;
    // kCGAnyInputEventType
    pub const ANY_INPUT_EVENT: u32 = !0;
    // kCGMouseButtonLeft
    pub const MOUSE_BUTTON_LEFT: u32 = 0;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        pub fn CGEventSourceSecondsSinceLastEventType(state: i32, event_type: u32) -> f64;
        pub fn CGEventSourceButtonState(state: i32, button: u32) -> bool;
    }
}

#[cfg(target_os = "macos")]
pub fn idle_seconds() -> Option<f64> {
    // SAFETY: 定数だけを渡す
    let s = unsafe { cg::CGEventSourceSecondsSinceLastEventType(cg::COMBINED_SESSION_STATE, cg::ANY_INPUT_EVENT) };
    (s >= 0.0).then_some(s)
}

#[cfg(target_os = "macos")]
pub fn left_button_down() -> bool {
    // SAFETY: 定数だけを渡す
    unsafe { cg::CGEventSourceButtonState(cg::COMBINED_SESSION_STATE, cg::MOUSE_BUTTON_LEFT) }
}

/// Macでは、全画面のアプリの上でも作業領域が画面の下端まで広がるので、ここでは見ない
/// (Dockを常に表示した場合の床は、Macでの確認のときに確かめる。企画書の未確定事項)。
#[cfg(target_os = "macos")]
pub fn foreground_covers(_x: i32, _y: i32, _w: u32, _h: u32) -> bool {
    false
}
