//! 에뮬레이터 세션 관리 — Android(JNI)/iOS(C ABI) 브리지가 공유.
//!
//! Threading model: start()가 tick 루프를 도는 전용 에뮬레이터 스레드를 만든다.
//! UI 스레드는 채널로 키 이벤트를 보내고 take_frame()으로 최신 프레임을 폴링한다.
//! 콜백 없음 — 브리지 계층에 런타임 의존성이 생기지 않는다.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc::{Receiver, Sender, channel},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use wie_backend::{Emulator, Event, GuestMemory, KeyCode};

use crate::{
    ScreenMode,
    cheat::Cheats,
    platform::{CapturedFrame, MobilePlatform, SharedPlatform, VibrationRequest},
};

/// 에뮬레이터 스레드에서 실행할 치트 작업. 게스트 메모리는 에뮬레이터가 소유하므로 그 스레드만 만진다.
type CheatJob = Box<dyn FnOnce(&mut Cheats, &mut dyn GuestMemory) + Send>;

enum KeyEvent {
    Down(KeyCode),
    Up(KeyCode),
}

/// 에러 종류 — 호스트가 사용자 안내 문구를 고르는 기준. message는 영어 진단 원문(로그/제보용).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ErrorKind {
    /// 게임 로드 실패 (지원하지 않는 형식, 손상된 파일 등)
    LoadFailed,
    /// 실행 중 오류 (미구현 API 등 호환성 문제)
    Runtime,
}

#[derive(Clone, Debug)]
pub struct SessionError {
    pub kind: ErrorKind,
    pub message: String,
}

/// A tick shorter than this ran no guest code: every task was asleep or waiting.
const IDLE_TICK: Duration = Duration::from_micros(20);
/// How long the emulator thread sleeps after an idle tick. A title's timers and `Thread.sleep`
/// wake up to this much late, which is far below a frame; spinning instead keeps a core at 100%
/// for a title that needs a tenth of it, and a phone answers that with heat and a throttled CPU.
const IDLE_SLEEP: Duration = Duration::from_micros(500);

struct Session {
    key_tx: Sender<KeyEvent>,
    cheat_tx: Sender<CheatJob>,
    stop: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    /// Share of the last second the emulator thread spent running the title, in percent. At 100
    /// the title is asking for more than the interpreter delivers and its frame rate is ours.
    load: Arc<AtomicU32>,
    platform: Arc<MobilePlatform>,
    error: Arc<Mutex<Option<SessionError>>>,
    thread: Option<JoinHandle<()>>,
}

static SESSION: Mutex<Option<Session>> = Mutex::new(None);

fn parse_key(key: &str) -> Option<KeyCode> {
    Some(match key {
        "UP" => KeyCode::UP,
        "DOWN" => KeyCode::DOWN,
        "LEFT" => KeyCode::LEFT,
        "RIGHT" => KeyCode::RIGHT,
        "OK" => KeyCode::OK,
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
        "#" => KeyCode::HASH,
        "*" => KeyCode::STAR,
        "CLR" => KeyCode::CLEAR,
        "SOFT_L" => KeyCode::LEFT_SOFT_KEY,
        "SOFT_R" => KeyCode::RIGHT_SOFT_KEY,
        "CALL" => KeyCode::CALL,
        "HANGUP" => KeyCode::HANGUP,
        _ => return None,
    })
}

fn emulator_thread(
    platform: Arc<MobilePlatform>,
    filename: String,
    game_data: Vec<u8>,
    screen_mode: ScreenMode,
    key_rx: Receiver<KeyEvent>,
    cheat_rx: Receiver<CheatJob>,
    stop: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    load: Arc<AtomicU32>,
    error: Arc<Mutex<Option<SessionError>>>,
) {
    raise_thread_priority();

    let mut emulator = match crate::create_emulator(Box::new(SharedPlatform(platform.clone())), &filename, &game_data, screen_mode) {
        Ok(x) => x,
        Err(e) => {
            *error.lock().unwrap() = Some(SessionError {
                kind: ErrorKind::LoadFailed,
                message: e.to_string(),
            });
            return;
        }
    };

    let mut pressed: HashMap<KeyCode, Instant> = HashMap::new();
    let mut cheats = Cheats::default();
    let mut load_window = Instant::now();
    let mut busy = Duration::ZERO;

    while !stop.load(Ordering::SeqCst) {
        // 치트 작업은 일시정지 중에도 처리한다 (치트 화면은 일시정지 메뉴에서 연다).
        // 게스트 메모리가 없는 타이틀이면 작업을 버려 요청 쪽이 "미지원"을 받는다.
        run_cheat_jobs(emulator.as_mut(), &mut cheats, &cheat_rx);

        // 일시정지: tick을 멈춰 게임 세계를 얼린다 (RetroArch 등 에뮬 표준의 auto-pause).
        // 키 이벤트는 채널에 남아 재개 후 처리된다.
        if paused.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(50));
            load.store(0, Ordering::Relaxed);
            load_window = Instant::now();
            busy = Duration::ZERO;
            continue;
        }

        let tick_start = Instant::now();

        if cheats.has_frozen()
            && let Some(memory) = emulator.memory()
        {
            cheats.apply(memory);
        }

        // 코어의 panic(에뮬레이션 버그)이 스레드를 조용히 죽이면 앱은 "프리즈"로만
        // 보인다 — catch_unwind로 Runtime 에러로 승격해 호스트가 표시하게 한다.
        // panic 후엔 emulator를 다시 만지지 않고 곧장 break하므로 AssertUnwindSafe가 안전하다.
        let step = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            while let Ok(event) = key_rx.try_recv() {
                match event {
                    KeyEvent::Down(key) => {
                        if let std::collections::hash_map::Entry::Vacant(e) = pressed.entry(key) {
                            emulator.handle_event(Event::Keydown(key));
                            e.insert(Instant::now());
                        }
                    }
                    KeyEvent::Up(key) => {
                        if pressed.remove(&key).is_some() {
                            emulator.handle_event(Event::Keyup(key));
                        }
                    }
                }
            }

            let now = Instant::now();
            for (key, time) in pressed.iter_mut() {
                if now.duration_since(*time).as_millis() > 100 {
                    emulator.handle_event(Event::Keyrepeat(*key));
                    *time = now;
                }
            }

            if platform.screen_capture().take_redraw_request() {
                emulator.handle_event(Event::Redraw);
            }

            emulator.tick()
        }));

        let tick_time = tick_start.elapsed();
        if tick_time < IDLE_TICK {
            std::thread::sleep(IDLE_SLEEP);
        } else {
            busy += tick_time;
        }
        let window = load_window.elapsed();
        if window >= Duration::from_secs(1) {
            load.store((busy.as_micros() * 100 / window.as_micros().max(1)).min(100) as u32, Ordering::Relaxed);
            load_window = Instant::now();
            busy = Duration::ZERO;
        }

        match step {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                *error.lock().unwrap() = Some(SessionError {
                    kind: ErrorKind::Runtime,
                    message: e.to_string(),
                });
                break;
            }
            Err(payload) => {
                let message = payload
                    .downcast_ref::<&str>()
                    .map(|s| (*s).to_string())
                    .or_else(|| payload.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "panic in emulator core".to_string());
                tracing::error!("emulator core panicked: {message}");
                *error.lock().unwrap() = Some(SessionError {
                    kind: ErrorKind::Runtime,
                    message: format!("core panic: {message}"),
                });
                break;
            }
        }
    }
}

fn run_cheat_jobs(emulator: &mut dyn Emulator, cheats: &mut Cheats, cheat_rx: &Receiver<CheatJob>) {
    while let Ok(job) = cheat_rx.try_recv() {
        if let Some(memory) = emulator.memory() {
            job(cheats, memory);
        }
    }
}

/// 치트 작업을 에뮬레이터 스레드에서 실행하고 결과를 기다린다. 세션이 없거나 타이틀에 게스트
/// 메모리가 없으면(순수 자바 타이틀) None. 검색은 수백 ms가 걸릴 수 있으니 UI 스레드에서 부르지 말 것.
fn with_cheats<R: Send + 'static>(job: impl FnOnce(&mut Cheats, &mut dyn GuestMemory) -> R + Send + 'static) -> Option<R> {
    // 기다리는 동안 SESSION을 잡고 있으면 UI 스레드의 프레임 폴링이 막힌다
    let cheat_tx = SESSION.lock().unwrap().as_ref()?.cheat_tx.clone();
    let (reply_tx, reply_rx) = channel();
    cheat_tx
        .send(Box::new(move |cheats, memory| {
            let _ = reply_tx.send(job(cheats, memory));
        }))
        .ok()?;

    reply_rx.recv().ok()
}

/// 실행 중인 타이틀이 메모리 치트를 지원하는지 (ARM 코어로 도는 KTF·LGT 타이틀)
pub fn cheat_supported() -> bool {
    with_cheats(|_, _| ()).is_some()
}

/// 값 검색. `refine`이 false면 새 검색(`width` 1·2·4바이트), true면 직전 후보를 좁힌다.
/// 일치 수를 반환한다 — cheat::MAX_CANDIDATES를 넘으면 앞쪽 후보만 보관된 상태다.
pub fn cheat_search(value: u32, width: usize, refine: bool) -> Option<u64> {
    with_cheats(move |cheats, memory| {
        if refine {
            cheats.refine(memory, value)
        } else {
            cheats.search(memory, value, width)
        }
    })
}

/// 검색 후보 앞쪽 `limit`개의 (주소, 현재 값)
pub fn cheat_results(limit: usize) -> Vec<(u32, u32)> {
    with_cheats(move |cheats, memory| cheats.results(memory, limit)).unwrap_or_default()
}

pub fn cheat_read(address: u32, width: usize) -> Option<u32> {
    with_cheats(move |_, memory| Cheats::read(memory, address, width)).flatten()
}

pub fn cheat_write(address: u32, width: usize, value: u32) -> bool {
    with_cheats(move |_, memory| Cheats::write(memory, address, width, value)).unwrap_or(false)
}

/// 값을 쓰고 풀 때까지 고정한다 (매 tick 다시 씀)
pub fn cheat_freeze(address: u32, width: usize, value: u32) -> bool {
    with_cheats(move |cheats, memory| cheats.freeze(memory, address, width, value)).unwrap_or(false)
}

pub fn cheat_unfreeze(address: u32) {
    with_cheats(move |cheats, _| cheats.unfreeze(address));
}

/// Asks the scheduler to treat the emulator thread as the one the user is looking at. Without it
/// iOS is free to run the thread on an efficiency core, several times slower than a performance
/// core, and the interpreter has no headroom to give away.
fn raise_thread_priority() {
    #[cfg(target_vendor = "apple")]
    {
        const QOS_CLASS_USER_INTERACTIVE: u32 = 0x21;
        unsafe extern "C" {
            fn pthread_set_qos_class_self_np(qos_class: u32, relative_priority: i32) -> i32;
        }
        let result = unsafe { pthread_set_qos_class_self_np(QOS_CLASS_USER_INTERACTIVE, 0) };
        if result != 0 {
            tracing::warn!("pthread_set_qos_class_self_np failed: {result}");
        }
    }
    #[cfg(target_os = "android")]
    {
        // The nice value Android gives its own display threads (THREAD_PRIORITY_DISPLAY). On Linux
        // `who` 0 with PRIO_PROCESS is the calling thread.
        const PRIO_PROCESS: i32 = 0;
        unsafe extern "C" {
            fn setpriority(which: i32, who: u32, priority: i32) -> i32;
        }
        if unsafe { setpriority(PRIO_PROCESS, 0, -4) } != 0 {
            tracing::warn!("setpriority failed");
        }
    }
}

/// 게임을 로드하고 에뮬레이터 스레드를 시작한다.
/// 단일 세션 앱이므로 이전 세션이 남아 있으면(호스트 정리 누락 등) 오류 대신 정리하고 시작한다.
pub fn start(
    filename: String,
    game_data: Vec<u8>,
    data_dir: PathBuf,
    soundfont_path: Option<PathBuf>,
    screen_mode: ScreenMode,
) -> anyhow::Result<()> {
    let mut session = SESSION.lock().unwrap();
    if let Some(mut stale) = session.take() {
        tracing::warn!("session already running - stopping stale session before start");
        stale.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = stale.thread.take() {
            let _ = thread.join();
        }
    }

    let platform = Arc::new(MobilePlatform::new(data_dir, crate::SCREEN_WIDTH, crate::SCREEN_HEIGHT, soundfont_path));
    let (key_tx, key_rx) = channel();
    let (cheat_tx, cheat_rx) = channel();
    let stop = Arc::new(AtomicBool::new(false));
    let paused = Arc::new(AtomicBool::new(false));
    let load = Arc::new(AtomicU32::new(0));
    let error = Arc::new(Mutex::new(None));

    let thread = {
        let (platform, stop, paused, load, error) = (platform.clone(), stop.clone(), paused.clone(), load.clone(), error.clone());
        std::thread::Builder::new()
            .name("wie-emulator".into())
            .stack_size(16 * 1024 * 1024)
            .spawn(move || emulator_thread(platform, filename, game_data, screen_mode, key_rx, cheat_rx, stop, paused, load, error))?
    };

    *session = Some(Session {
        key_tx,
        cheat_tx,
        stop,
        paused,
        load,
        platform,
        error,
        thread: Some(thread),
    });

    Ok(())
}

/// 에뮬레이션 일시정지/재개. 일시정지 중엔 tick이 멈춰 게임 세계가 얼어붙는다.
pub fn set_paused(paused: bool) {
    let session = SESSION.lock().unwrap();
    if let Some(session) = session.as_ref() {
        session.paused.store(paused, Ordering::SeqCst);
    }
}

/// 볼륨 (0.0~1.0, 0이면 음소거) — PCM(효과음)/MIDI(배경음악) 분리, 호스트 사운드 설정용
pub fn set_volume(pcm: f32, midi: f32) {
    let session = SESSION.lock().unwrap();
    if let Some(session) = session.as_ref() {
        session.platform.set_volume(pcm, midi);
    }
}

/// 게임 속도 (퍼센트, 100 = 원래 속도). 게임이 보는 시계를 그 배율로 흘린다.
pub fn set_speed(percent: u32) {
    let session = SESSION.lock().unwrap();
    if let Some(session) = session.as_ref() {
        session.platform.set_speed(percent);
    }
}

/// 마지막 paint 이후 새 프레임이 있으면 반환한다.
pub fn take_frame() -> Option<CapturedFrame> {
    let session = SESSION.lock().unwrap();
    session.as_ref()?.platform.screen_capture().take_frame()
}

/// Frames the title has presented since the session started. The host shows the difference over
/// a second as the frame rate.
pub fn frame_count() -> u64 {
    let session = SESSION.lock().unwrap();
    session.as_ref().map_or(0, |s| s.platform.screen_capture().frame_count())
}

/// Share of the last second the emulator thread spent running the title, in percent.
pub fn cpu_load() -> u32 {
    let session = SESSION.lock().unwrap();
    session.as_ref().map_or(0, |s| s.load.load(Ordering::Relaxed))
}

/// 게임이 요청한 보류 중인 진동을 가져온다 (호스트가 실제 진동 하드웨어로 전달).
pub fn take_vibration() -> Option<VibrationRequest> {
    let session = SESSION.lock().unwrap();
    session.as_ref()?.platform.take_vibration()
}

/// 게임이 종료를 요청했는지 확인한다 (호스트가 세션을 정리하고 UI를 되돌린다).
pub fn take_exit_requested() -> bool {
    let session = SESSION.lock().unwrap();
    session.as_ref().is_some_and(|s| s.platform.take_exit_requested())
}

pub fn key_down(key: &str) {
    send_key(key, KeyEvent::Down)
}

pub fn key_up(key: &str) {
    send_key(key, KeyEvent::Up)
}

fn send_key(key: &str, make: fn(KeyCode) -> KeyEvent) {
    let Some(keycode) = parse_key(key) else {
        tracing::warn!("unknown key: {key}");
        return;
    };

    let session = SESSION.lock().unwrap();
    if let Some(session) = session.as_ref() {
        let _ = session.key_tx.send(make(keycode));
    }
}

/// 보류 중인 오류(종류+진단 원문)를 가져오고 지운다.
pub fn take_error() -> Option<SessionError> {
    let session = SESSION.lock().unwrap();
    session.as_ref().and_then(|s| s.error.lock().unwrap().take())
}

/// 에뮬레이터 스레드를 정지하고 세션을 정리한다.
pub fn stop() {
    let mut session = SESSION.lock().unwrap();
    if let Some(mut session) = session.take() {
        session.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = session.thread.take() {
            let _ = thread.join();
        }
    }
}
