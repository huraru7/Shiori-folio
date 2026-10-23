//! ペットの状態と動き(詩織Ver6.0の段階4)。構想メモの「状態と居場所・動き」の表のとおりに動かす。
//!
//! 位置と速さは物理の画素で持ち、時間とカーソル・ボタン・無操作の秒数は外から渡す(テストで決められるように)。
//! 押したのか、つまんだのかは、ボタンを押したまま少し動かしたかで分ける。

use serde::Serialize;

/// 重力(ドット/秒²)。倍率を掛けて画素にする
const GRAVITY: f64 = 2400.0;
/// 歩く速さ(ドット/秒)
const WALK_SPEED: f64 = 40.0;
/// 押したまま、これより動かしたら「つまんだ」とみなす(ドット)
const DRAG_START: f64 = 4.0;
/// 投げたとみなす速さ(ドット/秒)
const THROW_SPEED: f64 = 120.0;
/// 普段のとき、画面の端からこの割合の中で過ごす
const EDGE_ZONE: f64 = 0.2;

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Appear,
    Normal,
    Sleep,
    Wake,
    Notify,
    Happy,
    React,
    Dangle,
    Fly,
    Land,
}

/// 普段のときに、していること
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Activity {
    Walk,
    Idle,
    Read,
}

/// いまいる画面のようす(物理の画素)。
#[derive(Clone, Copy, Debug)]
pub struct World {
    /// 絵の左上のxが取れる範囲
    pub left: f64,
    pub right: f64,
    /// 絵の左上のy(床に立ったとき)
    pub floor: f64,
    /// 天井(作業領域の上端)
    pub top: f64,
    /// 1ドットが何画素か
    pub scale: f64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    pub cursor: Option<(f64, f64)>,
    pub button_down: bool,
    /// 最後の入力からの秒数(取れなければNone。眠る動きを止める)
    pub idle_seconds: Option<f64>,
    pub sleep_after: f64,
}

/// 1回の計算で起きたこと(本体が、本のメニューを開くなどに使う)
#[derive(Debug, PartialEq, Eq)]
pub enum Event {
    Pressed,
    StateChanged(State),
}

pub struct Brain {
    pub x: f64,
    pub y: f64,
    vx: f64,
    vy: f64,
    /// 1: 右向き、-1: 左向き
    pub facing: f64,
    pub state: State,
    pub activity: Activity,
    /// いまの状態になってからの秒数(絵のコマはこれで決める)
    pub state_t: f64,
    /// いまの状態を終える秒数(Noneなら終わりを決めない)
    until: Option<f64>,
    /// 普段のときに、次の過ごし方を選ぶまでの秒数
    activity_left: f64,
    target_x: Option<f64>,
    /// 押した点(つまむ前)。押したまま動かしたら、つまむ
    press: Option<(f64, f64)>,
    grab: Option<(f64, f64)>,
    drag_prev: Option<(f64, f64)>,
    rng: u64,
}

impl Brain {
    /// 画面の下の右端に現れる
    pub fn new(w: &World, seed: u64) -> Self {
        let mut b = Self {
            x: w.right - 20.0 * w.scale,
            y: w.floor,
            vx: 0.0,
            vy: 0.0,
            facing: -1.0,
            state: State::Appear,
            activity: Activity::Idle,
            state_t: 0.0,
            until: Some(1.0),
            activity_left: 0.0,
            target_x: None,
            press: None,
            grab: None,
            drag_prev: None,
            rng: seed | 1,
        };
        b.x = b.x.clamp(w.left, w.right.max(w.left));
        b
    }

    fn rand(&mut self) -> f64 {
        self.rng = self.rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.rng >> 33) as f64 / (1u64 << 31) as f64
    }

    fn set(&mut self, state: State, until: Option<f64>, events: &mut Vec<Event>) {
        if self.state != state {
            events.push(Event::StateChanged(state));
        }
        self.state = state;
        self.state_t = 0.0;
        self.until = until;
    }

    /// 普段に戻る。`to_edge` なら、近いほうの端へ歩いていく
    fn to_normal(&mut self, w: &World, to_edge: bool, events: &mut Vec<Event>) {
        self.set(State::Normal, None, events);
        if to_edge {
            let mid = (w.left + w.right) / 2.0;
            let edge = if self.x < mid { w.left + 16.0 * w.scale } else { w.right - 16.0 * w.scale };
            self.walk_to(edge);
        } else {
            self.activity = Activity::Idle;
            self.activity_left = 3.0;
        }
    }

    fn walk_to(&mut self, x: f64) {
        self.target_x = Some(x);
        self.activity = Activity::Walk;
        self.facing = if x < self.x { -1.0 } else { 1.0 };
    }

    /// 普段のときの次の過ごし方: 待機・本を読む・端の近くで少し歩く
    fn choose_activity(&mut self, w: &World) {
        let r = self.rand();
        if r < 0.4 {
            self.activity = Activity::Read;
            self.activity_left = 10.0 + self.rand() * 15.0;
        } else if r < 0.75 {
            self.activity = Activity::Idle;
            self.activity_left = 4.0 + self.rand() * 6.0;
        } else {
            // 端の近くにとどまる(画面の端から EDGE_ZONE の中)
            let zone = (w.right - w.left) * EDGE_ZONE;
            let (lo, hi) = if self.x < (w.left + w.right) / 2.0 { (w.left, w.left + zone) } else { (w.right - zone, w.right) };
            let step = (30.0 + self.rand() * 90.0) * w.scale * if self.rand() < 0.5 { -1.0 } else { 1.0 };
            self.walk_to((self.x + step).clamp(lo, hi));
        }
    }

    /// 押された(画面のmousedown)。つまむか、押しただけかは、このあとの動きで決める
    pub fn press(&mut self, cursor: (f64, f64)) {
        if matches!(self.state, State::Dangle | State::Fly) {
            return;
        }
        self.press = Some(cursor);
    }

    /// 知らせ。Claudeの作業の終わりは喜ぶ。眠っていても起きて知らせる
    pub fn notice(&mut self, happy: bool) -> Vec<Event> {
        let mut events = Vec::new();
        if matches!(self.state, State::Dangle | State::Fly) {
            return events;
        }
        if happy {
            self.set(State::Happy, Some(2.0), &mut events);
        } else {
            self.set(State::Notify, Some(5.0), &mut events);
        }
        events
    }

    pub fn step(&mut self, dt: f64, w: &World, input: &Input) -> Vec<Event> {
        let mut events = Vec::new();
        self.state_t += dt;
        let s = w.scale;

        // 押したまま動かしたら、つまむ。動かさずに離したら、押しただけ
        if let Some((px, py)) = self.press {
            if !input.button_down {
                self.press = None;
                events.push(Event::Pressed);
                if !matches!(self.state, State::Notify | State::Happy) {
                    self.set(State::React, Some(0.4), &mut events);
                }
            } else if let Some((cx, cy)) = input.cursor {
                if ((cx - px).powi(2) + (cy - py).powi(2)).sqrt() > DRAG_START * s {
                    self.press = None;
                    self.grab = Some((px - self.x, py - self.y));
                    self.drag_prev = None;
                    self.vx = 0.0;
                    self.vy = 0.0;
                    self.set(State::Dangle, None, &mut events);
                }
            }
        }

        match self.state {
            State::Dangle => {
                if !input.button_down {
                    self.grab = None;
                    let fast = (self.vx.powi(2) + self.vy.powi(2)).sqrt() > THROW_SPEED * s;
                    if fast || self.y < w.floor - 1.0 {
                        self.set(State::Fly, None, &mut events);
                    } else {
                        self.y = w.floor;
                        self.set(State::Land, Some(0.5), &mut events);
                    }
                } else if let (Some((gx, gy)), Some((cx, cy))) = (self.grab, input.cursor) {
                    let (nx, ny) = (cx - gx, cy - gy);
                    if let Some((px, py)) = self.drag_prev {
                        if dt > 0.0 {
                            // ならした動きを、投げたときの勢いにする
                            self.vx = self.vx * 0.6 + (nx - px) / dt * 0.4;
                            self.vy = self.vy * 0.6 + (ny - py) / dt * 0.4;
                        }
                    }
                    self.drag_prev = Some((nx, ny));
                    self.x = nx;
                    self.y = ny;
                }
            }
            State::Fly => {
                self.vy += GRAVITY * s * dt;
                self.x += self.vx * dt;
                self.y += self.vy * dt;
                if self.x < w.left {
                    self.x = w.left;
                    self.vx = -self.vx * 0.5;
                } else if self.x > w.right {
                    self.x = w.right;
                    self.vx = -self.vx * 0.5;
                }
                if self.y < w.top {
                    self.y = w.top;
                    self.vy = 0.0;
                }
                if self.vx.abs() > 1.0 {
                    self.facing = self.vx.signum();
                }
                if self.y >= w.floor {
                    self.y = w.floor;
                    self.vx = 0.0;
                    self.vy = 0.0;
                    self.set(State::Land, Some(0.5), &mut events);
                }
            }
            State::Normal => {
                self.y = w.floor;
                let sleepy = input.idle_seconds.is_some_and(|i| i >= input.sleep_after);
                if sleepy {
                    self.target_x = None;
                    self.set(State::Sleep, None, &mut events);
                } else if self.activity == Activity::Walk {
                    let target = self.target_x.unwrap_or(self.x).clamp(w.left, w.right);
                    let step = WALK_SPEED * s * dt;
                    if (target - self.x).abs() <= step {
                        self.x = target;
                        self.target_x = None;
                        self.choose_activity(w);
                    } else {
                        self.facing = (target - self.x).signum();
                        self.x += self.facing * step;
                    }
                } else {
                    self.activity_left -= dt;
                    if self.activity_left <= 0.0 {
                        self.choose_activity(w);
                    }
                }
            }
            State::Sleep => {
                self.y = w.floor;
                if input.idle_seconds.is_some_and(|i| i < 1.0) {
                    self.set(State::Wake, Some(0.6), &mut events);
                }
            }
            _ => {
                // 現れる・起きる・知らせ・喜ぶ・反応する・着地は、その場で決まった時間だけ
                if self.y < w.floor - 1.0 && !matches!(self.state, State::Dangle) {
                    self.set(State::Fly, None, &mut events);
                } else {
                    self.y = w.floor;
                }
            }
        }

        // 画面の大きさが変わったときなど、範囲の外に出ないようにする
        if !matches!(self.state, State::Dangle) {
            self.x = self.x.clamp(w.left, w.right.max(w.left));
        }

        if let Some(until) = self.until {
            if self.state_t >= until {
                match self.state {
                    State::Appear | State::Land => self.to_normal(w, true, &mut events),
                    _ => self.to_normal(w, false, &mut events),
                }
            }
        }
        events
    }

    /// いまの絵の動きの名前(`sprites.json` の名前)
    pub fn anim(&self) -> &'static str {
        match self.state {
            State::Appear => "appear",
            State::Normal => match self.activity {
                Activity::Walk => "walk",
                Activity::Idle => "idle",
                Activity::Read => "read",
            },
            State::Sleep => "sleep",
            State::Wake => "wake",
            State::Notify => "notify",
            State::Happy => "happy",
            State::React => "react",
            State::Dangle => "dangle",
            State::Fly => {
                if self.vy < 0.0 {
                    "fly"
                } else {
                    "fall"
                }
            }
            State::Land => "land",
        }
    }

    pub fn grabbing(&self) -> bool {
        self.press.is_some() || self.grab.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> World {
        World { left: 0.0, right: 1792.0, floor: 840.0, top: 0.0, scale: 1.0 }
    }

    fn run(b: &mut Brain, secs: f64, input: &Input) -> Vec<Event> {
        let mut all = Vec::new();
        let n = (secs / 0.016).ceil() as usize;
        for _ in 0..n {
            all.extend(b.step(0.016, &world(), input));
        }
        all
    }

    fn awake() -> Input {
        Input { idle_seconds: Some(0.5), sleep_after: 300.0, ..Default::default() }
    }

    #[test]
    fn 現れてから普段になり_端に寄る() {
        let mut b = Brain::new(&world(), 7);
        assert_eq!(b.state, State::Appear);
        assert!((b.y - 840.0).abs() < 1e-9);
        run(&mut b, 1.1, &awake());
        assert_eq!(b.state, State::Normal);
        run(&mut b, 3.0, &awake());
        // 右の端(右から16ドット)に寄っている
        assert!(b.x > 1792.0 - 1792.0 * EDGE_ZONE - 1.0);
    }

    #[test]
    fn 触らないと眠り_触ると起きる() {
        let mut b = Brain::new(&world(), 7);
        run(&mut b, 1.1, &awake());
        let sleepy = Input { idle_seconds: Some(301.0), sleep_after: 300.0, ..Default::default() };
        run(&mut b, 0.1, &sleepy);
        assert_eq!(b.state, State::Sleep);
        run(&mut b, 0.1, &awake());
        assert_eq!(b.state, State::Wake);
        run(&mut b, 0.7, &awake());
        assert_eq!(b.state, State::Normal);
    }

    #[test]
    fn 押しただけなら反応し_動かしたらつまみ_離すと落ちて着地する() {
        let mut b = Brain::new(&world(), 7);
        run(&mut b, 1.1, &awake());
        let at = (b.x + 60.0, b.y + 100.0);
        b.press(at);
        let mut down = awake();
        down.button_down = true;
        down.cursor = Some(at);
        run(&mut b, 0.05, &down);
        let ev = run(&mut b, 0.02, &awake());
        assert!(ev.contains(&Event::Pressed));
        assert_eq!(b.state, State::React);

        // つまんで上へ持ち上げ、離す
        run(&mut b, 0.5, &awake());
        b.press(at);
        down.cursor = Some((at.0, at.1 - 300.0));
        run(&mut b, 0.1, &down);
        assert_eq!(b.state, State::Dangle);
        assert!(b.y < 840.0 - 200.0);
        run(&mut b, 0.02, &awake());
        assert_eq!(b.state, State::Fly);
        run(&mut b, 2.0, &awake());
        assert!((b.y - 840.0).abs() < 1e-9);
        assert!(matches!(b.state, State::Land | State::Normal));
    }

    #[test]
    fn 知らせはその場で_Claudeは喜ぶ() {
        let mut b = Brain::new(&world(), 7);
        run(&mut b, 1.1, &awake());
        let x = b.x;
        b.notice(false);
        assert_eq!(b.state, State::Notify);
        assert_eq!(b.anim(), "notify");
        run(&mut b, 1.0, &awake());
        assert!((b.x - x).abs() < 1e-9);
        b.notice(true);
        assert_eq!(b.state, State::Happy);
        run(&mut b, 2.1, &awake());
        assert_eq!(b.state, State::Normal);
    }
}
