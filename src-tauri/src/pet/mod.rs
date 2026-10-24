//! ペットの形(詩織Ver6.0の段階4)。
//!
//! ウィンドウの形とペットの形を切り替える。ペットの形にすると、本体のウィンドウを隠し(閉じない。
//! 会話・録音・予約は止まらない)、透明で常に最前面の小さなウィンドウ(段階0の方式A)にキャラクターを出す。
//! 本のメニューと小道具の枠(`pet-panel`)、知らせの吹き出し(`pet-bubble`)は、ペットとは別の透明な
//! ウィンドウにする(ペットのウィンドウを大きくすると、クリックを通す範囲が広がるため)。
//!
//! 位置・状態・絵のコマは本体の側(`brain.rs`)で決め、画面へは `pet://frame` で送る。
//! 絵の透明な点のクリックは、下のアプリに通す(カーソルの位置を見て `set_ignore_cursor_events` を切り替える)。
//! Macでは、ペットの形のあいだだけ、詩織をDockとCmd+Tabから外す(本体のウィンドウを隠している間だけなので、
//! ペットのウィンドウだけを外す仕組みは要らない)。

pub mod brain;
pub mod os;
pub mod sprites;

use crate::error_codes;
use crate::notify::{Kind, Notice};
use crate::shiori_log;
use brain::{Brain, Event, Input, World};
use serde::Serialize;
use serde_json::Value;
use sprites::{Sprites, FRAME_H, FRAME_W};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

const PART: &str = "PET";
pub const PET: &str = "pet";
pub const PANEL: &str = "pet-panel";
pub const BUBBLE: &str = "pet-bubble";
/// キャラクターの背丈が、画面の高さに占める割合(仮の絵で見て決める。企画書の未確定事項)
const HEIGHT_RATIO: f64 = 0.18;
/// 計算の間隔(1秒に60回)
const TICK: Duration = Duration::from_millis(16);
/// 絵のコマの速さ
const FPS: f64 = 8.0;
/// 本と小道具の枠(100%のとき)、吹き出しの大きさ(論理の画素)。本と枠は設定の割合を掛ける
const PANEL_SIZE: (f64, f64) = (440.0, 470.0);
const BUBBLE_SIZE: (f64, f64) = (280.0, 132.0);
/// 本がペットの押しで閉じたすぐあとの押しでは、開き直さない(もう一度押すと閉じる、のため)
const REOPEN_GUARD: Duration = Duration::from_millis(400);

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
struct FrameDto {
    state: brain::State,
    activity: brain::Activity,
    anim: String,
    file: String,
    frames: u32,
    frame: u32,
    facing: f64,
    /// 1ドットが何画素か(物理の画素)
    scale: u32,
}

struct Inner {
    active: bool,
    brain: Option<Brain>,
    scale: u32,
    sprites: Sprites,
    ignoring: bool,
    last_pos: (i32, i32),
    last_frame: Option<FrameDto>,
    panel_visible: bool,
    panel_hidden_at: Option<Instant>,
    bubble_visible: bool,
    panel_items: Vec<String>,
    panel_tool: Option<Value>,
    /// いまの本と小道具の枠の大きさ(論理の画素)
    panel_size: (f64, f64),
    sleep_after: f64,
    /// 前面のアプリが全画面か(0.5秒ごとに見直す)
    fullscreen: bool,
    fullscreen_checked: Option<Instant>,
    world: Option<World>,
}

fn ctl() -> &'static Mutex<Inner> {
    static C: OnceLock<Mutex<Inner>> = OnceLock::new();
    C.get_or_init(|| {
        Mutex::new(Inner {
            active: false,
            brain: None,
            scale: 1,
            sprites: Sprites::load(&crate::project_root().join("data").join("ui").join("pet")),
            ignoring: false,
            last_pos: (i32::MIN, i32::MIN),
            last_frame: None,
            panel_visible: false,
            panel_hidden_at: None,
            bubble_visible: false,
            panel_items: Vec::new(),
            panel_tool: None,
            panel_size: PANEL_SIZE,
            sleep_after: 300.0,
            fullscreen: false,
            fullscreen_checked: None,
            world: None,
        })
    })
}

fn lock() -> std::sync::MutexGuard<'static, Inner> {
    ctl().lock().unwrap_or_else(|e| e.into_inner())
}

pub fn is_active() -> bool {
    lock().active
}

/// 絵のファイル(専用のURLの仕組みの `/_shiori/pet/` から返す)
pub fn sprite_file(file: &str) -> Option<Vec<u8>> {
    lock().sprites.file_bytes(file)
}

// ---- ホットキー ----

static SHORTCUT: OnceLock<tauri_plugin_global_shortcut::Shortcut> = OnceLock::new();

/// 形の切り替えのホットキーを登録する。登録できなくても起動は止めない(ボタンからは切り替えられる)。
pub fn register_hotkey(app: &AppHandle, hotkey: &str) {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let result = hotkey
        .parse::<tauri_plugin_global_shortcut::Shortcut>()
        .map_err(|e| e.to_string())
        .and_then(|s| app.global_shortcut().register(s).map(|_| s).map_err(|e| e.to_string()));
    match result {
        Ok(s) => {
            let _ = SHORTCUT.set(s);
            shiori_log::info(PART, "形の切り替えのホットキーを登録した", &[("キー", &hotkey)]);
        }
        Err(e) => shiori_log::warn(error_codes::PET_003.code, "形の切り替えのホットキーを登録できない", &[("キー", &hotkey), ("理由", &e)]),
    }
}

pub fn is_pet_shortcut(s: &tauri_plugin_global_shortcut::Shortcut) -> bool {
    SHORTCUT.get().is_some_and(|p| p == s)
}

/// ホットキーから切り替える(ウィンドウを作ることがあるので、非同期の中で行う)
pub fn toggle_from_hotkey(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let result = if is_active() { leave(&app) } else { enter(&app) };
        if let Err(e) = result {
            shiori_log::warn(error_codes::PET_001.code, "形を切り替えられない", &[("理由", &e)]);
        }
    });
}

// ---- 窓 ----

fn build(app: &AppHandle, label: &str, view: &str, size: (f64, f64)) -> Result<WebviewWindow, String> {
    if let Some(w) = app.get_webview_window(label) {
        return Ok(w);
    }
    WebviewWindowBuilder::new(app, label, WebviewUrl::App(format!("index.html?view={view}").into()))
        .title("詩織")
        .transparent(true)
        .decorations(false)
        .shadow(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible_on_all_workspaces(true)
        .inner_size(size.0, size.1)
        .visible(false)
        .focused(false)
        .build()
        .map_err(|e| format!("{label}: {e}"))
}

fn ensure_windows(app: &AppHandle) -> Result<(), String> {
    build(app, PET, "pet", (FRAME_W as f64, FRAME_H as f64))?;
    let panel = build(app, PANEL, "pet-panel", PANEL_SIZE)?;
    build(app, BUBBLE, "pet-bubble", BUBBLE_SIZE)?;
    // 本と小道具の枠は、外を押したら(詩織から離れたら)閉じる
    static HOOKED: OnceLock<()> = OnceLock::new();
    if HOOKED.set(()).is_ok() {
        panel.on_window_event(|e| {
            if let tauri::WindowEvent::Focused(false) = e {
                if let Some(app) = crate::notify::app() {
                    hide_panel(app, "外を押した");
                }
            }
        });
    }
    Ok(())
}

fn window(app: &AppHandle, label: &str) -> Option<WebviewWindow> {
    app.get_webview_window(label)
}

/// いまいる画面(点を含む画面。無ければ主な画面)
fn monitor_at(app: &AppHandle, x: f64, y: f64) -> Option<tauri::Monitor> {
    app.monitor_from_point(x, y).ok().flatten().or_else(|| app.primary_monitor().ok().flatten())
}

fn dot_scale(monitor: &tauri::Monitor) -> u32 {
    let h = monitor.size().height as f64;
    ((h * HEIGHT_RATIO / FRAME_H as f64).round() as u32).max(1)
}

fn world_of(monitor: &tauri::Monitor, scale: u32, fullscreen: bool) -> World {
    let wa = monitor.work_area();
    let (sw, sh) = ((FRAME_W * scale) as f64, (FRAME_H * scale) as f64);
    // 全画面のアプリが前面にあるときは、タスクバーの分も床にする(作業領域が変わらず浮くため)
    let bottom = if fullscreen {
        (monitor.position().y + monitor.size().height as i32) as f64
    } else {
        (wa.position.y + wa.size.height as i32) as f64
    };
    World {
        left: wa.position.x as f64,
        right: (wa.position.x + wa.size.width as i32) as f64 - sw,
        floor: bottom - sh,
        top: wa.position.y as f64,
        scale: scale as f64,
    }
}

fn set_dock(app: &AppHandle, visible: bool) {
    #[cfg(target_os = "macos")]
    {
        let policy = if visible { tauri::ActivationPolicy::Regular } else { tauri::ActivationPolicy::Accessory };
        let _ = app.set_activation_policy(policy);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (app, visible);
}

/// ペットの形にする
pub fn enter(app: &AppHandle) -> Result<(), String> {
    if is_active() {
        return Ok(());
    }
    ensure_windows(app).inspect_err(|e| {
        shiori_log::warn(error_codes::PET_001.code, "ペットのウィンドウを作れない", &[("理由", e)]);
    })?;
    let main = window(app, "main").ok_or("本体のウィンドウが無い")?;
    // 本体のウィンドウがある画面の、下の右端に現れる
    let center = main
        .outer_position()
        .ok()
        .zip(main.outer_size().ok())
        .map(|(p, s)| (p.x as f64 + s.width as f64 / 2.0, p.y as f64 + s.height as f64 / 2.0))
        .unwrap_or((0.0, 0.0));
    let monitor = monitor_at(app, center.0, center.1).ok_or("画面が見つからない")?;
    let scale = dot_scale(&monitor);
    let world = world_of(&monitor, scale, false);
    // 自己確認の起動では眠らせない。最後の入力から眠るまでの時間を過ぎてから走らせると、
    // ペットが出た直後に眠り、「現れてから普段」の確かめが走らせる環境で変わってしまうため(段階6)
    let sleep_after =
        if crate::self_test::is_enabled() { f64::INFINITY } else { crate::pet_sleep_minutes() as f64 * 60.0 };
    let brain = Brain::new(&world, crate::notify::now_ms());
    let pet = window(app, PET).ok_or("ペットのウィンドウが無い")?;
    pet.set_size(PhysicalSize::new(FRAME_W * scale, FRAME_H * scale)).map_err(|e| e.to_string())?;
    pet.set_position(PhysicalPosition::new(brain.x.round() as i32, brain.y.round() as i32)).map_err(|e| e.to_string())?;
    {
        let mut c = lock();
        c.brain = Some(brain);
        c.scale = scale;
        c.world = Some(world);
        c.sleep_after = sleep_after;
        c.last_pos = (i32::MIN, i32::MIN);
        c.last_frame = None;
        c.ignoring = false;
        c.active = true;
    }
    let _ = main.hide();
    let _ = pet.show();
    set_dock(app, false);
    start_loop(app.clone());
    shiori_log::info(PART, "ペットの形にした", &[("倍率", &scale), ("眠るまで秒", &sleep_after)]);
    let _ = app.emit("pet://mode", "pet");
    Ok(())
}

/// ウィンドウの形に戻す。本体のウィンドウは、隠す前の大きさ・位置で出る
pub fn leave(app: &AppHandle) -> Result<(), String> {
    if !is_active() {
        return Ok(());
    }
    {
        let mut c = lock();
        c.active = false;
        c.brain = None;
        c.panel_visible = false;
        c.bubble_visible = false;
    }
    for label in [PANEL, BUBBLE, PET] {
        if let Some(w) = window(app, label) {
            let _ = w.hide();
        }
    }
    set_dock(app, true);
    if let Some(main) = window(app, "main") {
        let _ = main.show();
        let _ = main.unminimize();
        let _ = main.set_focus();
    }
    shiori_log::info(PART, "ウィンドウの形に戻した", &[]);
    let _ = app.emit("pet://mode", "window");
    Ok(())
}

// ---- 本と吹き出し ----

/// ペットの横に置く。右に余地があれば右、無ければ左。下はペットの足もとにそろえる。
/// 右に置いたらtrue(本は、ペットの側のページに項目を並べるため、どちらに開いたかを使う)
fn place_beside(app: &AppHandle, label: &str, size: (f64, f64), above: bool) -> bool {
    let Some(w) = window(app, label) else { return true };
    let c = lock();
    let (Some(b), Some(world)) = (c.brain.as_ref(), c.world) else { return true };
    let (pw, ph) = ((FRAME_W * c.scale) as f64, (FRAME_H * c.scale) as f64);
    let Some(monitor) = monitor_at(app, b.x + pw / 2.0, b.y + ph / 2.0) else { return true };
    let f = monitor.scale_factor();
    let (w_px, h_px) = (size.0 * f, size.1 * f);
    let wa = monitor.work_area();
    let (min_x, max_x) = (wa.position.x as f64, (wa.position.x + wa.size.width as i32) as f64 - w_px);
    let (min_y, max_y) = (wa.position.y as f64, world.floor + ph - h_px);
    let right = b.x + pw <= max_x;
    let (x, y) = if above {
        // 吹き出しは頭の上
        (b.x + pw / 2.0 - w_px / 2.0, b.y - h_px + 28.0 * c.scale as f64)
    } else if right {
        (b.x + pw, b.y + ph - h_px)
    } else {
        (b.x - w_px, b.y + ph - h_px)
    };
    drop(c);
    let x = x.clamp(min_x, max_x.max(min_x));
    let y = y.clamp(min_y, max_y.max(min_y));
    let _ = w.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
    right
}

#[derive(Serialize, Clone)]
struct PanelOpen {
    side: &'static str,
    /// 本と小道具の枠の大きさ(1.0が100%)。中身も同じ割合で縮める
    scale: f64,
}

fn show_panel(app: &AppHandle) {
    let scale = crate::pet_panel_percent() as f64 / 100.0;
    let size = (PANEL_SIZE.0 * scale, PANEL_SIZE.1 * scale);
    lock().panel_size = size;
    if let Some(w) = window(app, PANEL) {
        let _ = w.set_size(tauri::LogicalSize::new(size.0, size.1));
    }
    let right = place_beside(app, PANEL, size, false);
    let _ = app.emit_to(PANEL, "pet://panel-open", PanelOpen { side: if right { "right" } else { "left" }, scale });
    if let Some(w) = window(app, PANEL) {
        let _ = w.show();
        let _ = w.set_focus();
    }
    lock().panel_visible = true;
    shiori_log::info(PART, "本を開いた", &[]);
}

fn hide_panel(app: &AppHandle, why: &str) {
    {
        let mut c = lock();
        if !c.panel_visible {
            return;
        }
        c.panel_visible = false;
        c.panel_hidden_at = Some(Instant::now());
    }
    if let Some(w) = window(app, PANEL) {
        let _ = w.hide();
    }
    let _ = app.emit_to(PANEL, "pet://panel-closed", ());
    shiori_log::info(PART, "本を閉じた", &[("理由", &why)]);
}

/// ペットを押した: 本が開いていれば閉じ、閉じていれば開く
fn on_pressed(app: &AppHandle) {
    let (visible, just_hidden) = {
        let c = lock();
        (c.panel_visible, c.panel_hidden_at.is_some_and(|t| t.elapsed() < REOPEN_GUARD))
    };
    if visible {
        hide_panel(app, "ペットを押した");
    } else if !just_hidden {
        show_panel(app);
    }
}

/// 知らせの入口(`notify::show`)から。ペットの形なら吹き出しで知らせ、trueを返す
pub fn on_notice(notice: &Notice) -> bool {
    let Some(app) = crate::notify::app() else { return false };
    {
        let mut c = lock();
        if !c.active {
            return false;
        }
        if let Some(b) = c.brain.as_mut() {
            b.notice(notice.kind == Kind::Claude);
        }
        c.bubble_visible = true;
    }
    let _ = place_beside(app, BUBBLE, BUBBLE_SIZE, true);
    if let Some(w) = window(app, BUBBLE) {
        let _ = w.show();
    }
    true
}

// ---- 1秒60回の計算 ----

fn start_loop(app: AppHandle) {
    static STARTED: OnceLock<()> = OnceLock::new();
    if STARTED.set(()).is_err() {
        return;
    }
    std::thread::spawn(move || {
        let mut last = Instant::now();
        loop {
            std::thread::sleep(TICK);
            let now = Instant::now();
            let dt = (now - last).as_secs_f64().min(0.05);
            last = now;
            if is_active() {
                tick(&app, dt);
            }
        }
    });
}

fn tick(app: &AppHandle, dt: f64) {
    let Some(pet) = window(app, PET) else { return };
    let cursor = app.cursor_position().ok().map(|c| (c.x, c.y));
    let idle = os::idle_seconds();
    if idle.is_none() {
        static WARNED: OnceLock<()> = OnceLock::new();
        if WARNED.set(()).is_ok() {
            shiori_log::warn(error_codes::PET_004.code, "無操作の秒数を取れないので、眠る動きを止めた", &[]);
        }
    }
    let button_down = os::left_button_down();

    let mut c = lock();
    let Some(b) = c.brain.as_ref() else { return };
    let (pw, ph) = ((FRAME_W * c.scale) as f64, (FRAME_H * c.scale) as f64);
    let center = (b.x + pw / 2.0, b.y + ph / 2.0);
    drop(c);
    let Some(monitor) = monitor_at(app, center.0, center.1) else { return };

    let mut c = lock();
    // 前面のアプリが全画面かは、0.5秒ごとに見る
    if c.fullscreen_checked.is_none_or(|t| t.elapsed() > Duration::from_millis(500)) {
        let (p, s) = (monitor.position(), monitor.size());
        c.fullscreen = os::foreground_covers(p.x, p.y, s.width, s.height);
        c.fullscreen_checked = Some(Instant::now());
    }
    // 画面をまたいだら、その画面の倍率に直す
    let scale = dot_scale(&monitor);
    let resized = scale != c.scale;
    c.scale = scale;
    let world = world_of(&monitor, scale, c.fullscreen);
    c.world = Some(world);
    let sleep_after = c.sleep_after;
    let input = Input { cursor, button_down, idle_seconds: idle, sleep_after };
    let Some(b) = c.brain.as_mut() else { return };
    let events = b.step(dt, &world, &input);
    let pos = (b.x.round() as i32, b.y.round() as i32);
    let facing_left = b.facing < 0.0;
    let anim_name = b.anim();
    let state = b.state;
    let activity = b.activity;
    let state_t = b.state_t;
    let grabbing = b.grabbing();

    // クリックを受けるのは、絵の不透明な点の上だけ(つまんでいる間は受け続ける)
    let over_sprite = cursor.is_some_and(|(cx, cy)| {
        let (dx, dy) = (((cx - pos.0 as f64) / scale as f64).floor() as i32, ((cy - pos.1 as f64) / scale as f64).floor() as i32);
        let frame = frame_of(&c.sprites, anim_name, state_t);
        c.sprites.opaque_at(anim_name, frame, facing_left, dx, dy)
    });
    let ignore = !(over_sprite || grabbing);
    let ignore_changed = ignore != c.ignoring;
    c.ignoring = ignore;

    let (real_anim, a) = c.sprites.anim(anim_name);
    let dto = FrameDto {
        state,
        activity,
        anim: real_anim.to_string(),
        file: a.info.file.clone(),
        frames: a.info.frames,
        frame: frame_of(&c.sprites, anim_name, state_t),
        facing: if facing_left { -1.0 } else { 1.0 },
        scale,
    };
    let frame_changed = c.last_frame.as_ref() != Some(&dto);
    if frame_changed {
        c.last_frame = Some(dto.clone());
    }
    let moved = pos != c.last_pos;
    c.last_pos = pos;
    let (panel_visible, bubble_visible, panel_size) = (c.panel_visible, c.bubble_visible, c.panel_size);
    drop(c);

    if resized {
        let _ = pet.set_size(PhysicalSize::new(FRAME_W * scale, FRAME_H * scale));
    }
    if moved {
        let _ = pet.set_position(PhysicalPosition::new(pos.0, pos.1));
        if panel_visible {
            let _ = place_beside(app, PANEL, panel_size, false);
        }
        if bubble_visible {
            let _ = place_beside(app, BUBBLE, BUBBLE_SIZE, true);
        }
    }
    if ignore_changed {
        let _ = pet.set_ignore_cursor_events(ignore);
    }
    if frame_changed {
        let _ = app.emit_to(PET, "pet://frame", &dto);
    }
    for e in events {
        match e {
            Event::Pressed => on_pressed(app),
            Event::StateChanged(s) => shiori_log::debug(PART, "状態が変わった", &[("状態", &format!("{s:?}"))]),
        }
    }
}

fn frame_of(sprites: &Sprites, anim: &str, state_t: f64) -> u32 {
    let (_, a) = sprites.anim(anim);
    let n = (state_t * FPS).floor() as u32;
    if a.info.r#loop {
        n % a.info.frames
    } else {
        n.min(a.info.frames - 1)
    }
}

// ---- 窓口 ----

#[tauri::command]
pub async fn pet_enter(app: AppHandle) -> Result<(), String> {
    enter(&app)
}

#[tauri::command]
pub async fn pet_leave(app: AppHandle) -> Result<(), String> {
    leave(&app)
}

/// ペットの絵を押した(画面のmousedown)
#[tauri::command]
pub fn pet_press(app: AppHandle) {
    let cursor = app.cursor_position().ok().map(|c| (c.x, c.y));
    if let (Some(cur), Some(b)) = (cursor, lock().brain.as_mut()) {
        b.press(cur);
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpritesDto {
    animations: std::collections::BTreeMap<String, sprites::AnimInfo>,
    overridden: bool,
}

#[tauri::command]
pub fn pet_sprites() -> SpritesDto {
    let c = lock();
    SpritesDto { animations: c.sprites.infos(), overridden: c.sprites.overridden }
}

/// 吹き出しの知らせが全部消えた
#[tauri::command]
pub fn pet_bubble_done(app: AppHandle) {
    lock().bubble_visible = false;
    if let Some(w) = window(&app, BUBBLE) {
        let _ = w.hide();
    }
}

/// 吹き出しを押した: タイマーと拡張機能は本の小道具の枠、Claudeと要確認はウィンドウに戻ってその画面へ
#[tauri::command]
pub async fn pet_open_source(app: AppHandle, notice: Notice) -> Result<(), String> {
    match (notice.kind, notice.source.as_deref()) {
        (Kind::Timer | Kind::Extension, Some(name)) => {
            show_panel(&app);
            let _ = app.emit_to(PANEL, "pet://open-tool", name);
        }
        _ => {
            leave(&app)?;
            let _ = app.emit_to("main", "shiori://open-source", &notice);
        }
    }
    Ok(())
}

/// 本の画面が、並べた項目と、小道具の自己確認の結果を知らせる
#[tauri::command]
pub fn pet_panel_report(items: Option<Vec<String>>, tool: Option<Value>) {
    let mut c = lock();
    if let Some(items) = items {
        c.panel_items = items;
    }
    if tool.is_some() {
        c.panel_tool = tool;
    }
}

/// 本を閉じる(本の画面の閉じるボタンから)
#[tauri::command]
pub fn pet_close_panel(app: AppHandle) {
    hide_panel(&app, "閉じるを押した");
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PetStatus {
    mode: &'static str,
    state: Option<brain::State>,
    anim: Option<&'static str>,
    scale: u32,
    position: Option<(i32, i32)>,
    size: Option<(u32, u32)>,
    floor: Option<f64>,
    on_floor: bool,
    main_visible: bool,
    pet_visible: bool,
    panel_visible: bool,
    bubble_visible: bool,
    panel_items: Vec<String>,
    panel_tool: Option<Value>,
    idle_seconds: Option<f64>,
    sprites_overridden: bool,
    /// 自己確認: 絵の外の点(0,0)と、絵の真ん中の不透明な点で、クリックを受けるか
    hit_outside: bool,
    hit_center: bool,
}

#[tauri::command]
pub fn pet_status(app: AppHandle) -> PetStatus {
    let visible = |label: &str| window(&app, label).and_then(|w| w.is_visible().ok()).unwrap_or(false);
    let size = window(&app, PET).and_then(|w| w.outer_size().ok()).map(|s| (s.width, s.height));
    let position = window(&app, PET).and_then(|w| w.outer_position().ok()).map(|p| (p.x, p.y));
    let c = lock();
    let floor = c.world.map(|w| w.floor);
    let on_floor = matches!((position, floor), (Some((_, y)), Some(f)) if (y as f64 - f).abs() <= 1.5);
    let center = c.sprites.center_opaque();
    PetStatus {
        mode: if c.active { "pet" } else { "window" },
        state: c.brain.as_ref().map(|b| b.state),
        anim: c.brain.as_ref().map(|b| b.anim()),
        scale: c.scale,
        position,
        size,
        floor,
        on_floor,
        main_visible: visible("main"),
        pet_visible: visible(PET),
        panel_visible: visible(PANEL),
        bubble_visible: visible(BUBBLE),
        panel_items: c.panel_items.clone(),
        panel_tool: c.panel_tool.clone(),
        idle_seconds: os::idle_seconds(),
        sprites_overridden: c.sprites.overridden,
        hit_outside: c.sprites.opaque_at("idle", 0, false, 0, 0),
        hit_center: center.is_some_and(|(x, y)| c.sprites.opaque_at("idle", 0, false, x, y)),
    }
}

/// 自己確認: 本を開き、その中で小道具を開いて、小道具の確かめを走らせる
#[tauri::command]
pub fn pet_selftest_panel(app: AppHandle, tool: Option<String>) {
    if !crate::self_test::is_enabled() {
        return;
    }
    if !lock().panel_visible {
        show_panel(&app);
    }
    if let Some(tool) = tool {
        lock().panel_tool = None;
        let _ = app.emit_to(PANEL, "pet://selftest-tool", tool);
    }
}

/// 自己確認: 本を閉じる
#[tauri::command]
pub fn pet_selftest_close_panel(app: AppHandle) {
    if crate::self_test::is_enabled() {
        hide_panel(&app, "自己確認");
    }
}
