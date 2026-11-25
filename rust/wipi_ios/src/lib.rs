//! iOS host for the wie emulator core — C ABI over wipi_core.
//! 헤더는 include/wipi_ios.h. jni_bridge와 대칭인 폴링 모델:
//! wipi_start / wipi_get_frame(60fps 폴링) / wipi_key_down / wipi_key_up / wipi_get_error / wipi_stop.

use std::{
    ffi::{CStr, c_char},
    path::PathBuf,
};

use wipi_core::{extract_metadata, session};

pub use wipi_core::{SCREEN_HEIGHT, SCREEN_WIDTH};

/// 로깅(stderr)과 panic hook 초기화. 앱 시작 시 1회 호출.
#[unsafe(no_mangle)]
pub extern "C" fn wipi_init() {
    // TRACE 전체 출력은 에뮬레이션을 수십 배 느리게 함 — INFO 이상만
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::new("info"))
        .try_init();

    std::panic::set_hook(Box::new(|info| {
        tracing::error!("panic: {info}");
    }));
}

/// # Safety
/// `game_data`는 `game_data_len` 바이트의 유효한 버퍼, 나머지는 유효한 C 문자열이어야 한다.
/// `soundfont_path`는 빈 문자열이면 MIDI 무음.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wipi_start(
    game_data: *const u8,
    game_data_len: usize,
    filename: *const c_char,
    data_dir: *const c_char,
    soundfont_path: *const c_char,
) -> bool {
    let result = (|| -> anyhow::Result<()> {
        if game_data.is_null() || filename.is_null() || data_dir.is_null() || soundfont_path.is_null() {
            anyhow::bail!("null argument");
        }

        let game_data = unsafe { std::slice::from_raw_parts(game_data, game_data_len) }.to_vec();
        let filename = unsafe { CStr::from_ptr(filename) }.to_str()?.to_owned();
        let data_dir = unsafe { CStr::from_ptr(data_dir) }.to_str()?.to_owned();
        let soundfont_path = unsafe { CStr::from_ptr(soundfont_path) }.to_str()?;
        let soundfont_path = (!soundfont_path.is_empty()).then(|| PathBuf::from(soundfont_path));

        session::start(filename, game_data, PathBuf::from(data_dir), soundfont_path)
    })();

    match result {
        Ok(()) => true,
        Err(e) => {
            tracing::error!("wipi_start failed: {e}");
            false
        }
    }
}

/// 최신 프레임을 RGBA 바이트로 복사한다. 새 프레임이 있었으면 true.
///
/// # Safety
/// `out_rgba`는 `capacity` 바이트 이상의 유효한 버퍼여야 하고,
/// capacity는 SCREEN_WIDTH*SCREEN_HEIGHT*4 이상이어야 한다.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wipi_get_frame(out_rgba: *mut u8, capacity: usize) -> bool {
    let Some(frame) = session::take_frame() else {
        return false;
    };

    if out_rgba.is_null() || capacity < frame.pixels.len() {
        tracing::error!("wipi_get_frame: buffer too small ({capacity} < {})", frame.pixels.len());
        return false;
    }

    unsafe { std::ptr::copy_nonoverlapping(frame.pixels.as_ptr(), out_rgba, frame.pixels.len()) };
    true
}

/// # Safety
/// `key`는 유효한 C 문자열이어야 한다 ("UP", "OK", "1", "*", "SOFT_L"...).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wipi_key_down(key: *const c_char) {
    if let Some(key) = unsafe { to_str(key) } {
        session::key_down(key);
    }
}

/// # Safety
/// `key`는 유효한 C 문자열이어야 한다.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wipi_key_up(key: *const c_char) {
    if let Some(key) = unsafe { to_str(key) } {
        session::key_up(key);
    }
}

unsafe fn to_str<'a>(s: *const c_char) -> Option<&'a str> {
    if s.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(s) }.to_str().ok()
}

/// 보류 중인 오류를 가져오고 지운다. 오류가 있었으면 true.
/// `out_kind`: 0=로드 실패(형식/손상), 1=실행 중 오류(호환성). `buf`에는 영어 진단 원문(UTF-8).
/// 호스트는 kind로 사용자 문구를 고르고, 원문은 상세/제보용으로 표시한다.
///
/// # Safety
/// `buf`는 `capacity` 바이트 이상의 유효한 버퍼, `out_kind`는 유효한 쓰기 가능 위치(또는 null)여야 한다.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wipi_get_error(buf: *mut c_char, capacity: usize, out_kind: *mut u8) -> bool {
    let Some(error) = session::take_error() else {
        return false;
    };
    let message = error.message;

    if !out_kind.is_null() {
        unsafe {
            *out_kind = match error.kind {
                session::ErrorKind::LoadFailed => 0,
                session::ErrorKind::Runtime => 1,
            };
        }
    }

    if buf.is_null() || capacity == 0 {
        return true;
    }

    // capacity-1 바이트 안에서 UTF-8 경계를 지켜 자른다
    let mut len = message.len().min(capacity - 1);
    while len > 0 && !message.is_char_boundary(len) {
        len -= 1;
    }

    unsafe {
        std::ptr::copy_nonoverlapping(message.as_ptr() as *const c_char, buf, len);
        *buf.add(len) = 0;
    }
    true
}

/// 게임이 요청한 보류 중인 진동을 폴링한다. 요청이 있었으면 true를 반환하고
/// `out_duration_ms`(ms)와 `out_intensity`(0~100)를 채운다.
///
/// # Safety
/// 두 포인터 모두 유효한 쓰기 가능 위치여야 한다.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wipi_poll_vibrate(out_duration_ms: *mut u64, out_intensity: *mut u8) -> bool {
    let Some(request) = session::take_vibration() else {
        return false;
    };

    if !out_duration_ms.is_null() {
        unsafe { *out_duration_ms = request.duration_ms };
    }
    if !out_intensity.is_null() {
        unsafe { *out_intensity = request.intensity };
    }
    true
}

/// 에뮬레이션 일시정지/재개. 일시정지 중엔 tick이 멈춰 게임 세계가 얼어붙는다
/// (백그라운드 진입 시 호출 — 에뮬레이터 표준 auto-pause).
#[unsafe(no_mangle)]
pub extern "C" fn wipi_set_paused(paused: bool) {
    session::set_paused(paused);
}

/// 볼륨 설정 (0.0~1.0, 0이면 음소거) — PCM(효과음)/MIDI(배경음악) 분리.
/// 사운드폰트와 게임 내장 샘플의 음량 차이를 사용자가 보정할 수 있게 한다. 게임 상태에는 영향 없다.
#[unsafe(no_mangle)]
pub extern "C" fn wipi_set_volume(pcm_volume: f32, midi_volume: f32) {
    session::set_volume(pcm_volume, midi_volume);
}

/// Share of the last second the emulator thread spent running the game, 0..100. Near 100 the
/// device is the limit on the frame rate; well below it the game is pacing itself.
#[unsafe(no_mangle)]
pub extern "C" fn wipi_cpu_load() -> u32 {
    session::cpu_load()
}

/// 게임이 종료를 요청했는지 폴링한다. true면 호스트가 wipi_stop 후 UI를 되돌려야 한다.
#[unsafe(no_mangle)]
pub extern "C" fn wipi_poll_exit() -> bool {
    session::take_exit_requested()
}

#[unsafe(no_mangle)]
pub extern "C" fn wipi_stop() {
    session::stop();
}

/// 게임 패키지(zip/jar)에서 표지 아이콘 PNG를 `out`에 복사한다.
/// 실제 PNG 길이를 반환하며, 0이면 아이콘이 없거나 버퍼가 부족한 경우다.
/// `out`을 null로 주면 필요한 크기만 반환한다(버퍼 크기 협상용).
///
/// # Safety
/// `game_data`는 `game_data_len` 바이트의 유효한 버퍼여야 한다.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wipi_game_icon(game_data: *const u8, game_data_len: usize, out: *mut u8, out_cap: usize) -> usize {
    if game_data.is_null() {
        return 0;
    }
    let buf = unsafe { std::slice::from_raw_parts(game_data, game_data_len) };
    let Some(icon) = extract_metadata(buf).icon_png else {
        return 0;
    };

    if out.is_null() {
        return icon.len(); // 크기 협상
    }
    if icon.len() > out_cap {
        return 0;
    }
    unsafe { std::ptr::copy_nonoverlapping(icon.as_ptr(), out, icon.len()) };
    icon.len()
}

/// 게임 패키지에서 게임명(__adf__의 Name, EUC-KR raw 바이트)을 `out`에 복사한다.
/// 실제 길이를 반환하며, 0이면 이름이 없거나 버퍼가 부족한 경우다.
/// 호스트가 EUC-KR(CP949)로 디코딩해야 한다.
///
/// # Safety
/// `game_data`는 `game_data_len` 바이트의 유효한 버퍼여야 한다.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wipi_game_name(game_data: *const u8, game_data_len: usize, out: *mut u8, out_cap: usize) -> usize {
    if game_data.is_null() {
        return 0;
    }
    let buf = unsafe { std::slice::from_raw_parts(game_data, game_data_len) };
    let Some(name) = extract_metadata(buf).name_euckr else {
        return 0;
    };

    if out.is_null() {
        return name.len();
    }
    if name.len() > out_cap {
        return 0;
    }
    unsafe { std::ptr::copy_nonoverlapping(name.as_ptr(), out, name.len()) };
    name.len()
}
