// WIPI C(Clet) 데모 게임 "하트 캐치" — dlunch/wipi SDK(Rust→ARM)로 작성.
// 좌우(◀▶ 또는 4/6)로 바구니를 움직여 떨어지는 하트를 받는다.
// WIPI Emulator 앱의 내장 데모: KTF 포장 zip으로 배포되어 정통 WIPI C 경로(ARM 에뮬레이션)를 시연한다.
#![cfg_attr(not(test), no_main)]
#![no_std]
extern crate alloc;

use alloc::{format, rc::Rc};
use core::{cell::RefCell, time::Duration};

use wipi::{
    app::App,
    event::KeyCode,
    framebuffer::{Color, Framebuffer},
    timer::Timer,
    wipi_main,
};

const W: i32 = 240;
const H: i32 = 320;

// ★ SDK 버그 우회: color_to_pixel이 fb.bpp를 무시하고 항상 ARGB8888로 패킹하지만,
// wie의 프레임버퍼는 16bpp(RGB565)라 fgpxl 하위 16비트가 565로 해석됨.
// → 원하는 RGB565 값이 하위 16비트(g<<8|b)에 오도록 인코딩한다. (업스트림 수정 후 원복 예정)
const fn rgb(r: u8, g: u8, b: u8) -> Color {
    let v: u16 = (((r as u16) >> 3) << 11) | (((g as u16) >> 2) << 5) | ((b as u16) >> 3);
    Color {
        r: 0,
        g: (v >> 8) as u8,
        b: (v & 0xFF) as u8,
        a: 0,
    }
}

const BG: Color = rgb(0x12, 0x8C, 0x7F);
const FLOOR: Color = rgb(0x0E, 0x6E, 0x64);
const WHITE: Color = rgb(0xFF, 0xFF, 0xFF);
const MINT: Color = rgb(0xBF, 0xE8, 0xE2);
const CREAM: Color = rgb(0xFF, 0xE3, 0xC2);
const HEART: Color = rgb(0xFF, 0x4D, 0x6D);
const BASKET: Color = rgb(0xFF, 0x8A, 0x3D);
const BASKET_RIM: Color = rgb(0xD9, 0x66, 0x1F);

struct State {
    basket_x: i32,
    dir: i32,
    heart_x: i32,
    heart_y: i32,
    score: i32,
    missed: i32,
    seed: u64,
}

impl State {
    fn new() -> Self {
        Self {
            basket_x: W / 2,
            dir: 0,
            heart_x: 60,
            heart_y: 40,
            score: 0,
            missed: 0,
            seed: 20260721,
        }
    }

    fn next_random(&mut self, bound: i32) -> i32 {
        self.seed = self
            .seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let v = ((self.seed >> 33) as i32) % bound;
        if v < 0 { v + bound } else { v }
    }

    fn tick(&mut self) {
        self.basket_x += self.dir * 6;
        self.basket_x = self.basket_x.clamp(20, W - 20);

        self.heart_y += 4 + self.score / 5;
        if self.heart_y > H - 44 {
            let caught = self.heart_x > self.basket_x - 26 && self.heart_x < self.basket_x + 26;
            if caught {
                self.score += 1;
            } else {
                self.missed += 1;
            }
            self.heart_y = 40;
            self.heart_x = 20 + self.next_random(W - 40);
        }
    }
}

pub struct HeartCatchApp {
    state: Rc<RefCell<State>>,
    _timer: Timer,
}

impl HeartCatchApp {
    fn new() -> Self {
        let state = Rc::new(RefCell::new(State::new()));

        let timer_state = state.clone();
        let timer = Timer::periodic(Duration::from_millis(50), move || {
            timer_state.borrow_mut().tick();
            Framebuffer::screen_framebuffer().request_repaint();
        });

        Self { state, _timer: timer }
    }
}

impl App for HeartCatchApp {
    fn on_paint(&mut self) {
        let st = self.state.borrow();
        let mut fb = Framebuffer::screen_framebuffer();

        fb.fill_rect(0, 0, W, H, BG);

        // MC_grpDrawString의 y는 베이스라인(글자 하단) — 상단 위치 + 폰트 높이(12)로 지정
        fb.draw_text(38, 20, "WIPI EMULATOR DEMO", WHITE);
        fb.draw_text(30, 38, "LEFT/RIGHT: catch hearts!", MINT);

        fb.draw_text(8, 60, &format!("SCORE {}", st.score), CREAM);
        fb.draw_text(W - 70, 60, &format!("MISS {}", st.missed), CREAM);

        // 픽셀 하트 (rect 조합)
        let hx = st.heart_x;
        let hy = st.heart_y;
        fb.fill_rect(hx - 7, hy - 6, 6, 4, HEART);
        fb.fill_rect(hx + 1, hy - 6, 6, 4, HEART);
        fb.fill_rect(hx - 9, hy - 3, 18, 7, HEART);
        fb.fill_rect(hx - 6, hy + 4, 12, 4, HEART);
        fb.fill_rect(hx - 3, hy + 8, 6, 3, HEART);
        fb.fill_rect(hx - 1, hy + 11, 2, 2, HEART);

        // 바구니
        fb.fill_rect(st.basket_x - 20, H - 40, 40, 14, BASKET);
        fb.fill_rect(st.basket_x - 20, H - 40, 40, 3, BASKET_RIM);

        // 바닥
        fb.fill_rect(0, H - 20, W, 20, FLOOR);
    }

    fn on_keydown(&mut self, key_code: KeyCode) {
        let mut st = self.state.borrow_mut();
        match key_code {
            KeyCode::Left | KeyCode::Key4 => st.dir = -1,
            KeyCode::Right | KeyCode::Key6 => st.dir = 1,
            _ => {}
        }
    }

    fn on_keyup(&mut self, key_code: KeyCode) {
        let mut st = self.state.borrow_mut();
        match key_code {
            KeyCode::Left | KeyCode::Key4 | KeyCode::Right | KeyCode::Key6 => st.dir = 0,
            _ => {}
        }
    }
}

#[wipi_main]
pub fn main() -> HeartCatchApp {
    HeartCatchApp::new()
}
