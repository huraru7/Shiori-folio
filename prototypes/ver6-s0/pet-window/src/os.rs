//! OSの機能で取るもの(試作3の無操作の秒数と、ドラッグを離したかどうか)。

/// 最後のマウス・キーボードの入力からの秒数。
#[cfg(windows)]
pub fn idle_seconds() -> f64 {
    use windows_sys::Win32::System::SystemInformation::GetTickCount;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    let mut info = LASTINPUTINFO { cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
    // SAFETY: cbSizeを設定した構造体を渡す
    let ok = unsafe { GetLastInputInfo(&mut info) };
    if ok == 0 {
        return -1.0;
    }
    // どちらも起動からのミリ秒(約49日で一周する)。wrapping_subで一周をまたいでも正しく引ける
    let now = unsafe { GetTickCount() };
    now.wrapping_sub(info.dwTime) as f64 / 1000.0
}

/// マウスの左ボタンが押されているか。
#[cfg(windows)]
pub fn left_button_down() -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
    // SAFETY: 引数は仮想キーコードだけ
    (unsafe { GetAsyncKeyState(VK_LBUTTON as i32) } as u16 & 0x8000) != 0
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
pub fn idle_seconds() -> f64 {
    // SAFETY: 定数だけを渡す
    unsafe { cg::CGEventSourceSecondsSinceLastEventType(cg::COMBINED_SESSION_STATE, cg::ANY_INPUT_EVENT) }
}

#[cfg(target_os = "macos")]
pub fn left_button_down() -> bool {
    // SAFETY: 定数だけを渡す
    unsafe { cg::CGEventSourceButtonState(cg::COMBINED_SESSION_STATE, cg::MOUSE_BUTTON_LEFT) }
}
