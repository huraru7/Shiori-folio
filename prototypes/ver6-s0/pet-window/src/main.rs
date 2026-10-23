//! Ver6.0 段階0の試作2・3: ペットのウィンドウと無操作の判定。
//!
//! 実行: cargo run --release -- [--mode a|b]
//!   a: 絵と同じ大きさの小さなウィンドウを、set_positionで毎フレーム動かす(既定)
//!   b: 作業領域と同じ大きさの透明なウィンドウを置き、その中で絵を動かす
//! 絵を左ドラッグでつまみ、離すと投げられる。右クリックで終わる。
//!
//! 位置と速さは、すべて画面の物理ピクセルで持つ(Tauriのカーソル位置と作業領域が物理ピクセルのため)。

mod os;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{
    AppHandle, Emitter, Manager, Monitor, PhysicalPosition, PhysicalSize, State, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};

/// 絵の大きさ(論理ピクセル。キャラクターのドット絵と同じ横128×縦192)
const SPRITE_W: f64 = 128.0;
const SPRITE_H: f64 = 192.0;
/// 重力(論理ピクセル/秒²)。画面の拡大率を掛けて使う
const GRAVITY: f64 = 2400.0;
/// 歩く速さ(論理ピクセル/秒)
const WALK_SPEED: f64 = 40.0;
/// 物理の計算の間隔(目安は1秒に60回)
const TICK: Duration = Duration::from_millis(16);

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Mode {
    A,
    B,
}

struct Pet {
    /// 絵の左上(物理ピクセル)
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    grabbed: bool,
    /// つまんだ点と絵の左上の差
    grab_dx: f64,
    grab_dy: f64,
    /// 1: 右向き、-1: 左向き
    facing: f64,
    walking: bool,
    /// 歩く・止まるを切り替えるまでの秒数
    walk_timer: f64,
    /// 方式Bで、いまクリックを下のアプリに通しているか
    ignoring: bool,
    /// 方式Aで、いまのウィンドウの拡大率(画面をまたいで変わったら大きさを直す)
    scale: f64,
    rng: u64,
}

impl Pet {
    /// 試作なので、乱数は簡単な線形合同法で足りる
    fn rand(&mut self) -> f64 {
        self.rng = self.rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.rng >> 33) as f64 / (1u64 << 31) as f64
    }
}

type Shared = Arc<Mutex<Pet>>;

#[derive(Clone, Serialize)]
struct PetEvent {
    /// 方式Bで、ウィンドウの中での絵の左上(CSSピクセル)
    x: f64,
    y: f64,
    facing: f64,
    state: &'static str,
}

#[derive(Clone, Serialize)]
struct StatsEvent {
    mode: Mode,
    idle_seconds: f64,
    tick_avg_ms: f64,
    tick_max_ms: f64,
    scale: f64,
    monitor: String,
    ignoring: bool,
}

/// 絵をつまむ。画面のmousedownから呼ぶ
#[tauri::command]
fn grab(app: AppHandle, pet: State<Shared>) {
    let Ok(c) = app.cursor_position() else { return };
    let mut p = pet.lock().unwrap();
    p.grabbed = true;
    p.grab_dx = c.x - p.x;
    p.grab_dy = c.y - p.y;
    p.vx = 0.0;
    p.vy = 0.0;
}

#[tauri::command]
fn quit(app: AppHandle) {
    app.exit(0);
}

fn monitor_at(app: &AppHandle, x: f64, y: f64) -> Option<Monitor> {
    app.monitor_from_point(x, y).ok().flatten().or_else(|| app.primary_monitor().ok().flatten())
}

fn monitor_label(m: &Monitor) -> String {
    let wa = m.work_area();
    format!(
        "{} 作業領域 {}x{} @({},{})",
        m.name().cloned().unwrap_or_default(),
        wa.size.width,
        wa.size.height,
        wa.position.x,
        wa.position.y
    )
}

/// 物理の計算と、ウィンドウ・画面への反映を、一定の間隔で回す
fn run_loop(app: AppHandle, window: WebviewWindow, pet: Shared, mode: Mode, fixed: Option<Monitor>) {
    let mut last = Instant::now();
    let mut stats_at = Instant::now();
    let (mut sum_ms, mut max_ms, mut n) = (0.0f64, 0.0f64, 0u32);
    let mut last_pos = (i32::MIN, i32::MIN);
    // ドラッグ中の速さは、直近の動きをならして求める
    let mut drag_prev: Option<(f64, f64)> = None;
    loop {
        std::thread::sleep(TICK);
        let now = Instant::now();
        let dt = (now - last).as_secs_f64().min(0.05);
        let ms = (now - last).as_secs_f64() * 1000.0;
        last = now;
        sum_ms += ms;
        max_ms = max_ms.max(ms);
        n += 1;

        let cursor = app.cursor_position().ok();
        let mut p = pet.lock().unwrap();

        // 方式Bは置いた画面に固定。方式Aは、絵の中心がある画面の作業領域を使う
        let center = (p.x + SPRITE_W * p.scale / 2.0, p.y + SPRITE_H * p.scale / 2.0);
        let Some(monitor) = fixed.clone().or_else(|| monitor_at(&app, center.0, center.1)) else {
            continue;
        };
        let scale = monitor.scale_factor();
        let wa = monitor.work_area();
        let (wx, wy) = (wa.position.x as f64, wa.position.y as f64);
        let (ww, wh) = (wa.size.width as f64, wa.size.height as f64);
        let (sw, sh) = (SPRITE_W * scale, SPRITE_H * scale);
        let floor = wy + wh - sh;

        let mut state = "待機";
        if p.grabbed {
            // OSのボタンの状態で離したかを見る(画面のmouseupは、ウィンドウの外で離すと届かないため)
            if !os::left_button_down() {
                p.grabbed = false;
                drag_prev = None;
            } else if let Some(c) = cursor {
                let (nx, ny) = (c.x - p.grab_dx, c.y - p.grab_dy);
                if let Some((px, py)) = drag_prev {
                    // 指数移動平均で、投げたときの勢いを求める
                    p.vx = p.vx * 0.6 + (nx - px) / dt * 0.4;
                    p.vy = p.vy * 0.6 + (ny - py) / dt * 0.4;
                }
                drag_prev = Some((nx, ny));
                p.x = nx;
                p.y = ny;
                state = "つままれる";
            }
        }
        if !p.grabbed {
            let airborne = p.y < floor - 0.5 || p.vy < 0.0;
            if airborne {
                p.vy += GRAVITY * scale * dt;
                p.x += p.vx * dt;
                p.y += p.vy * dt;
                // 作業領域の左右の端で跳ね返る
                if p.x < wx {
                    p.x = wx;
                    p.vx = -p.vx * 0.5;
                } else if p.x > wx + ww - sw {
                    p.x = wx + ww - sw;
                    p.vx = -p.vx * 0.5;
                }
                if p.y < wy {
                    p.y = wy;
                    p.vy = 0.0;
                }
                if p.y >= floor {
                    p.y = floor;
                    p.vx = 0.0;
                    p.vy = 0.0;
                }
                state = "飛ぶ";
            } else {
                p.y = floor;
                p.walk_timer -= dt;
                if p.walk_timer <= 0.0 {
                    p.walking = !p.walking;
                    p.walk_timer = 2.0 + p.rand() * 4.0;
                    if p.walking && p.rand() < 0.5 {
                        p.facing = -p.facing;
                    }
                }
                if p.walking {
                    p.x += p.facing * WALK_SPEED * scale * dt;
                    if p.x < wx {
                        p.x = wx;
                        p.facing = 1.0;
                    } else if p.x > wx + ww - sw {
                        p.x = wx + ww - sw;
                        p.facing = -1.0;
                    }
                    state = "歩く";
                }
            }
        }

        match mode {
            Mode::A => {
                if (scale - p.scale).abs() > f64::EPSILON {
                    p.scale = scale;
                    let _ = window.set_size(PhysicalSize::new(sw.round() as u32, sh.round() as u32));
                }
                let pos = (p.x.round() as i32, p.y.round() as i32);
                if pos != last_pos {
                    let _ = window.set_position(PhysicalPosition::new(pos.0, pos.1));
                    last_pos = pos;
                }
            }
            Mode::B => {
                p.scale = scale;
                // 絵の上にカーソルがあるときだけクリックを受け、それ以外は下のアプリに通す
                let inside = cursor
                    .map(|c| c.x >= p.x && c.x < p.x + sw && c.y >= p.y && c.y < p.y + sh)
                    .unwrap_or(false);
                let ignore = !(inside || p.grabbed);
                if ignore != p.ignoring {
                    let _ = window.set_ignore_cursor_events(ignore);
                    p.ignoring = ignore;
                }
            }
        }

        let ev = PetEvent { x: (p.x - wx) / scale, y: (p.y - wy) / scale, facing: p.facing, state };
        let ignoring = p.ignoring;
        drop(p);
        let _ = window.emit("pet", ev);

        if stats_at.elapsed() >= Duration::from_millis(250) {
            let _ = window.emit(
                "stats",
                StatsEvent {
                    mode,
                    idle_seconds: os::idle_seconds(),
                    tick_avg_ms: sum_ms / n as f64,
                    tick_max_ms: max_ms,
                    scale,
                    monitor: monitor_label(&monitor),
                    ignoring,
                },
            );
            stats_at = Instant::now();
            (sum_ms, max_ms, n) = (0.0, 0.0, 0);
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = match args.iter().position(|a| a == "--mode").and_then(|i| args.get(i + 1)) {
        Some(m) if m == "b" => Mode::B,
        _ => Mode::A,
    };

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![grab, quit])
        .setup(move |app| {
            // Macでは skip_taskbar が効かないので、アクセサリにしてDockとCmd+Tabから外す
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let handle = app.handle().clone();
            let monitor = app.primary_monitor()?.ok_or("画面が見つからない")?;
            let scale = monitor.scale_factor();
            let wa = *monitor.work_area();
            let (sw, sh) = (SPRITE_W * scale, SPRITE_H * scale);
            // 起動したときは、画面の下の右端に立つ
            let start_x = wa.position.x as f64 + wa.size.width as f64 - sw - 40.0 * scale;
            let start_y = wa.position.y as f64 + wa.size.height as f64 - sh;

            let init = format!("window.PET_MODE = '{}';", if mode == Mode::A { "a" } else { "b" });
            let window = WebviewWindowBuilder::new(app, "pet", WebviewUrl::App("index.html".into()))
                .title("pet-window-proto")
                .transparent(true)
                .decorations(false)
                .shadow(false)
                .resizable(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .visible_on_all_workspaces(true)
                .visible(false)
                .initialization_script(&init)
                .build()?;

            match mode {
                Mode::A => {
                    window.set_size(PhysicalSize::new(sw.round() as u32, sh.round() as u32))?;
                    window.set_position(PhysicalPosition::new(start_x.round() as i32, start_y.round() as i32))?;
                }
                Mode::B => {
                    window.set_size(wa.size)?;
                    window.set_position(wa.position)?;
                    window.set_ignore_cursor_events(true)?;
                }
            }
            window.show()?;

            let pet: Shared = Arc::new(Mutex::new(Pet {
                x: start_x,
                y: start_y,
                vx: 0.0,
                vy: 0.0,
                grabbed: false,
                grab_dx: 0.0,
                grab_dy: 0.0,
                facing: -1.0,
                walking: false,
                walk_timer: 1.0,
                ignoring: mode == Mode::B,
                scale,
                rng: 0x5eed,
            }));
            app.manage(pet.clone());
            let fixed = if mode == Mode::B { Some(monitor) } else { None };
            std::thread::spawn(move || run_loop(handle, window, pet, mode, fixed));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("試作のアプリを起動できなかった");
}
