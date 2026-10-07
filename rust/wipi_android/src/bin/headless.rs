//! Milestone 2: run a game headlessly on an Android device (via adb shell)
//! and dump the first painted frame to a BMP file.
//!
//! Usage: headless <game file> <output bmp> [ticks]

use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use tracing_subscriber::{Layer, layer::SubscriberExt, util::SubscriberInitExt};

use wie_backend::{Event, KeyCode};

use wipi_android::platform::{CapturedFrame, MobilePlatform, SharedPlatform};

/// 진행성(T2/T3) 검증용 기본 키 시나리오 ("초:키" 목록, 키는 300ms 후 up)
/// 타이틀 통과(OK×2) → 메뉴 이동(DOWN) → 선택(OK) → 게임 내 입력(5)
const DEFAULT_KEYS: &str = "4:OK,6:OK,8:DOWN,10:OK,12:5";

fn parse_key(name: &str) -> Option<KeyCode> {
    Some(match name {
        "UP" => KeyCode::UP,
        "DOWN" => KeyCode::DOWN,
        "LEFT" => KeyCode::LEFT,
        "RIGHT" => KeyCode::RIGHT,
        "OK" => KeyCode::OK,
        "CLR" => KeyCode::CLEAR,
        "SOFT_L" => KeyCode::LEFT_SOFT_KEY,
        "SOFT_R" => KeyCode::RIGHT_SOFT_KEY,
        "STAR" => KeyCode::STAR,
        "HASH" => KeyCode::HASH,
        "0" => KeyCode::NUM0,
        "1" => KeyCode::NUM1,
        "2" => KeyCode::NUM2,
        "3" => KeyCode::NUM3,
        "4" => KeyCode::NUM4,
        "5" => KeyCode::NUM5,
        "6" => KeyCode::NUM6,
        "7" => KeyCode::NUM7,
        "8" => KeyCode::NUM8,
        "9" => KeyCode::NUM9,
        _ => return None,
    })
}

/// "4:OK,6:2" 형식 → (ms, key, down) 이벤트 목록 (down 후 300ms 뒤 up)
fn parse_key_script(spec: &str) -> Vec<(u64, KeyCode, bool)> {
    let mut script = Vec::new();
    for entry in spec.split(',').filter(|x| !x.is_empty()) {
        let Some((at, key_name)) = entry.split_once(':') else { continue };
        let (Ok(at_s), Some(key)) = (at.trim().parse::<f64>(), parse_key(key_name.trim())) else {
            eprintln!("ignoring invalid key entry: {entry}");
            continue;
        };
        let at_ms = (at_s * 1000.0) as u64;
        script.push((at_ms, key, true));
        script.push((at_ms + 300, key, false));
    }
    script.sort_by_key(|x| x.0);
    script
}

/// Counts every `stub …` / `Unimplemented` / `Unknown …` warning by API name so a
/// sweep can list what a game reached that we do not implement, independent of RUST_LOG.
#[derive(Default, Clone)]
struct StubCounter(Arc<Mutex<BTreeMap<String, u64>>>);

impl<S: tracing::Subscriber> Layer<S> for StubCounter {
    fn on_event(&self, event: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
        struct Message(Option<String>);
        impl tracing::field::Visit for Message {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                if field.name() == "message" {
                    self.0 = Some(format!("{value:?}"));
                }
            }
        }
        let mut message = Message(None);
        event.record(&mut message);
        let Some(message) = message.0 else { return };
        let name = if let Some(rest) = message.strip_prefix("stub ") {
            rest.split(['(', ' ', ':']).next().unwrap_or(rest).to_string()
        } else if message.starts_with("Unimplemented") || message.starts_with("Unknown") {
            message.chars().take(80).collect()
        } else {
            return;
        };
        *self.0.lock().unwrap().entry(name).or_default() += 1;
    }
}

fn main() -> anyhow::Result<()> {
    // The app runs the emulator on a 16 MB thread (wipi_core::session); match it so a guest call
    // chain that overflows there overflows here too, and one that fits there fits here.
    std::thread::Builder::new()
        .name("headless".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(run)?
        .join()
        .map_err(|_| anyhow::anyhow!("headless thread panicked"))?
}

fn run() -> anyhow::Result<()> {
    let stub_counter = StubCounter::default();
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_filter(tracing_subscriber::EnvFilter::from_default_env()),
        )
        .with(stub_counter.clone().with_filter(tracing_subscriber::filter::LevelFilter::WARN))
        .init();

    let args: Vec<String> = std::env::args().collect();
    let [_, game_path, out_path, rest @ ..] = args.as_slice() else {
        anyhow::bail!("usage: headless <game file> <output bmp prefix> [seconds] [keys: \"4:OK,6:2\" | \"none\"]");
    };
    let max_seconds: u64 = rest.first().map(|x| x.parse()).transpose()?.unwrap_or(10);
    let key_spec = rest.get(1).map(String::as_str).unwrap_or(DEFAULT_KEYS);
    let key_script = if key_spec == "none" { Vec::new() } else { parse_key_script(key_spec) };

    let out_path_buf = PathBuf::from(out_path);
    let data_dir = out_path_buf.parent().unwrap_or(std::path::Path::new(".")).join("wie_data");
    let platform = Arc::new(MobilePlatform::new(data_dir, 240, 320, None));

    // WIE_VCLOCK=1: 가상 시계(호출마다 1ms 전진) — 실행 간 결정성 확보 (flaky 조사용).
    // 루프 종료/키 주입/프레임 저장도 가상 시간 기준으로 동작한다.
    const VCLOCK_START_EPOCH_MS: u64 = 1_767_225_600_000; // 2026-01-01 UTC
    let virtual_clock = std::env::var("WIE_VCLOCK").is_ok_and(|v| v != "0" && !v.is_empty());
    if virtual_clock {
        platform.enable_virtual_clock(VCLOCK_START_EPOCH_MS);
        eprintln!("virtual clock enabled (deterministic, 1ms per now() call)");
    }

    let buf = fs::read(game_path)?;
    let mut emulator = wipi_android::create_emulator(Box::new(SharedPlatform(platform.clone())), game_path, &buf)?;

    let start = std::time::Instant::now();
    let elapsed_ms = {
        let platform = platform.clone();
        move || -> u64 {
            if virtual_clock {
                platform.virtual_now_ms().unwrap() - VCLOCK_START_EPOCH_MS
            } else {
                start.elapsed().as_millis() as u64
            }
        }
    };
    let mut last_frame: Option<CapturedFrame> = None;
    let mut saved_seconds = 0u64;
    let mut ticks = 0u64;
    let mut next_key = 0usize; // KEY_SCRIPT 진행 인덱스
    while elapsed_ms() / 1000 < max_seconds {
        if platform.screen_capture().take_redraw_request() {
            emulator.handle_event(Event::Redraw);
        }

        // 스크립트된 키 입력 주입 (진행성 검증)
        while next_key < key_script.len() && elapsed_ms() >= key_script[next_key].0 {
            let (at, key, down) = key_script[next_key];
            eprintln!("INPUT t={}ms {:?} {}", at, key, if down { "down" } else { "up" });
            emulator.handle_event(if down { Event::Keydown(key) } else { Event::Keyup(key) });
            next_key += 1;
        }

        emulator.tick()?;
        ticks += 1;

        if let Some(frame) = platform.screen_capture().take_frame() {
            last_frame = Some(frame);
        }

        // 1초마다 현재 프레임 저장 → 진행 과정 관찰용
        let elapsed = elapsed_ms() / 1000;
        if elapsed > saved_seconds {
            saved_seconds = elapsed;
            if let Some(frame) = &last_frame {
                let non_black = frame.pixels.chunks(4).filter(|p| p[0] != 0 || p[1] != 0 || p[2] != 0).count();
                write_bmp(&format!("{out_path}_{saved_seconds:03}.bmp"), frame)?;
                eprintln!("t={saved_seconds}s ticks={ticks} non_black_pixels={non_black}");
            } else {
                eprintln!("t={saved_seconds}s ticks={ticks} (no frame painted yet)");
            }
        }
    }

    // 미구현 API 요약 — 스윕이 "어떤 API가 부족한지"를 게임별로 바로 보이게 한다
    {
        let stubs = stub_counter.0.lock().unwrap();
        if !stubs.is_empty() {
            let mut sorted: Vec<_> = stubs.iter().collect();
            sorted.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
            eprintln!("STUBS: {}", sorted.iter().map(|(k, v)| format!("{k}x{v}")).collect::<Vec<_>>().join(" "));
        }
    }

    let Some(frame) = last_frame else {
        anyhow::bail!("no frame was painted after {max_seconds}s ({ticks} ticks)");
    };

    write_bmp(&format!("{out_path}_final.bmp"), &frame)?;
    println!(
        "OK: {}x{} final frame written to {out_path}_final.bmp after {ticks} ticks",
        frame.width, frame.height
    );

    Ok(())
}

/// 24-bit BMP writer (bottom-up, BGR) — 의존성 없이 프레임 덤프용
fn write_bmp(path: &str, frame: &CapturedFrame) -> anyhow::Result<()> {
    let (w, h) = (frame.width as usize, frame.height as usize);
    let row_size = (w * 3).div_ceil(4) * 4;
    let pixel_data_size = row_size * h;
    let file_size = 54 + pixel_data_size;

    let mut out = Vec::with_capacity(file_size);
    // BITMAPFILEHEADER
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(file_size as u32).to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&54u32.to_le_bytes());
    // BITMAPINFOHEADER
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(h as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&[0; 24]);

    for y in (0..h).rev() {
        let row_start = y * w * 4;
        for x in 0..w {
            let i = row_start + x * 4;
            out.extend_from_slice(&[frame.pixels[i + 2], frame.pixels[i + 1], frame.pixels[i]]);
        }
        out.resize(out.len() + (row_size - w * 3), 0);
    }

    let mut file = fs::File::create(path)?;
    file.write_all(&out)?;
    Ok(())
}
