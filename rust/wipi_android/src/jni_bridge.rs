//! JNI surface for the Android app (package com.parkjeongseop.wipi, class WipiNative).
//! 실제 세션 로직은 wipi_core::session에 있고 여기는 JNI 변환만 담당한다.

use std::path::PathBuf;

use jni::{
    JNIEnv,
    objects::{JByteArray, JClass, JIntArray, JObject, JString},
    sys::{jboolean, jint, jlong, jlongArray, jstring},
};

use wipi_core::session;

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_parkjeongseop_wipi_WipiNative_nativeStart(
    mut env: JNIEnv,
    _class: JClass,
    game_data: JByteArray,
    filename: JString,
    data_dir: JString,
    soundfont_path: JString,
) -> jboolean {
    let result = (|| -> anyhow::Result<()> {
        let game_data = env.convert_byte_array(&game_data)?;
        let filename: String = env.get_string(&filename)?.into();
        let data_dir: String = env.get_string(&data_dir)?.into();
        let soundfont_path: String = env.get_string(&soundfont_path)?.into();
        let soundfont_path = (!soundfont_path.is_empty()).then(|| PathBuf::from(soundfont_path));

        session::start(filename, game_data, PathBuf::from(data_dir), soundfont_path)
    })();

    match result {
        Ok(()) => 1,
        Err(e) => {
            tracing::error!("nativeStart failed: {e}");
            0
        }
    }
}

/// Copies the latest frame into `out` as ARGB ints. Returns true when a new
/// frame was written. `out` must hold at least width*height ints (240*320).
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_parkjeongseop_wipi_WipiNative_nativeGetFrame(env: JNIEnv, _class: JClass, out: JIntArray) -> jboolean {
    let Some(frame) = session::take_frame() else {
        return 0;
    };

    let argb: Vec<i32> = frame
        .pixels
        .chunks_exact(4)
        .map(|p| i32::from_be_bytes([0xff, p[0], p[1], p[2]]))
        .collect();

    match env.set_int_array_region(&out, 0, &argb) {
        Ok(()) => 1,
        Err(e) => {
            tracing::error!("nativeGetFrame: {e}");
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_parkjeongseop_wipi_WipiNative_nativeKeyDown(mut env: JNIEnv, _class: JClass, key: JString) {
    if let Ok(key) = env.get_string(&key) {
        session::key_down(&String::from(key));
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_parkjeongseop_wipi_WipiNative_nativeKeyUp(mut env: JNIEnv, _class: JClass, key: JString) {
    if let Ok(key) = env.get_string(&key) {
        session::key_up(&String::from(key));
    }
}

/// 보류 중인 오류를 가져오고 지운다. 없으면 null.
/// `out_kind[0]`: 0=로드 실패(형식/손상), 1=실행 중 오류(호환성). 반환 문자열은 영어 진단 원문.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_parkjeongseop_wipi_WipiNative_nativeGetError(
    env: JNIEnv,
    _class: JClass,
    out_kind: jni::objects::JIntArray,
) -> jstring {
    match session::take_error() {
        Some(error) => {
            let kind = match error.kind {
                session::ErrorKind::LoadFailed => 0,
                session::ErrorKind::Runtime => 1,
            };
            let _ = env.set_int_array_region(&out_kind, 0, &[kind]);
            env.new_string(error.message).map(|s| s.into_raw()).unwrap_or(std::ptr::null_mut())
        }
        None => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_parkjeongseop_wipi_WipiNative_nativeStop(_env: JNIEnv, _class: JClass) {
    session::stop();
}

/// 게임이 요청한 보류 중인 진동을 폴링한다. 요청이 있었으면 true를 반환하고
/// `out`(길이 2 이상)에 [duration_ms, intensity(0~100)]를 채운다.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_parkjeongseop_wipi_WipiNative_nativePollVibrate(
    env: JNIEnv,
    _class: JClass,
    out: jni::objects::JLongArray,
) -> jboolean {
    let Some(request) = session::take_vibration() else {
        return 0;
    };

    let values = [request.duration_ms as i64, request.intensity as i64];
    match env.set_long_array_region(&out, 0, &values) {
        Ok(()) => 1,
        Err(e) => {
            tracing::error!("nativePollVibrate: {e}");
            0
        }
    }
}

/// 에뮬레이션 일시정지/재개 (백그라운드 auto-pause — tick 루프가 얼어붙음)
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_parkjeongseop_wipi_WipiNative_nativeSetPaused(_env: JNIEnv, _class: JClass, paused: jboolean) {
    session::set_paused(paused != 0);
}

/// 볼륨 (0.0~1.0, 0이면 음소거) — PCM(효과음)/MIDI(배경음악) 분리, 호스트 사운드 설정
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_parkjeongseop_wipi_WipiNative_nativeSetVolume(_env: JNIEnv, _class: JClass, pcm: f32, midi: f32) {
    session::set_volume(pcm, midi);
}

/// 게임 속도 (퍼센트, 100 = 원래 속도). 세션 시작 뒤에 호출.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_parkjeongseop_wipi_WipiNative_nativeSetSpeed(_env: JNIEnv, _class: JClass, percent: jint) {
    session::set_speed(percent.max(0) as u32);
}

// 메모리 치트 — ARM 코어로 도는 타이틀(KTF·LGT)만 지원. 모두 에뮬레이터 스레드의 응답을
// 기다리므로 메인 스레드가 아닌 곳에서 호출할 것. 주소·값은 u32 비트 패턴을 Int로 주고받는다.

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_parkjeongseop_wipi_WipiNative_nativeCheatSupported(_env: JNIEnv, _class: JClass) -> jboolean {
    session::cheat_supported() as jboolean
}

/// 값 검색 (refine이면 직전 후보 좁히기). 일치 수, 미지원이면 -1.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_parkjeongseop_wipi_WipiNative_nativeCheatSearch(
    _env: JNIEnv,
    _class: JClass,
    value: jint,
    width: jint,
    refine: jboolean,
) -> jlong {
    session::cheat_search(value as u32, width as usize, refine != 0).map_or(-1, |count| count as jlong)
}

/// 검색 후보 앞쪽 `limit`개. 원소마다 상위 32비트 = 주소, 하위 32비트 = 현재 값.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_parkjeongseop_wipi_WipiNative_nativeCheatResults(env: JNIEnv, _class: JClass, limit: jint) -> jlongArray {
    let results: Vec<jlong> = session::cheat_results(limit.max(0) as usize)
        .into_iter()
        .map(|(address, value)| ((address as jlong) << 32) | value as jlong)
        .collect();
    let array = env
        .new_long_array(results.len() as _)
        .and_then(|array| env.set_long_array_region(&array, 0, &results).map(|_| array));

    array.map_or(std::ptr::null_mut(), |array| array.into_raw())
}

/// 주소의 현재 값 (0..=u32::MAX), 읽을 수 없으면 -1
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_parkjeongseop_wipi_WipiNative_nativeCheatRead(_env: JNIEnv, _class: JClass, address: jint, width: jint) -> jlong {
    session::cheat_read(address as u32, width as usize).map_or(-1, |value| value as jlong)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_parkjeongseop_wipi_WipiNative_nativeCheatWrite(
    _env: JNIEnv,
    _class: JClass,
    address: jint,
    width: jint,
    value: jint,
) -> jboolean {
    session::cheat_write(address as u32, width as usize, value as u32) as jboolean
}

/// 값을 쓰고 nativeCheatUnfreeze까지 고정한다 (매 tick 다시 씀)
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_parkjeongseop_wipi_WipiNative_nativeCheatFreeze(
    _env: JNIEnv,
    _class: JClass,
    address: jint,
    width: jint,
    value: jint,
) -> jboolean {
    session::cheat_freeze(address as u32, width as usize, value as u32) as jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_parkjeongseop_wipi_WipiNative_nativeCheatUnfreeze(_env: JNIEnv, _class: JClass, address: jint) {
    session::cheat_unfreeze(address as u32);
}

/// Share of the last second the emulator thread spent running the game, 0..100.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_parkjeongseop_wipi_WipiNative_nativeCpuLoad(_env: JNIEnv, _class: JClass) -> jni::sys::jint {
    session::cpu_load() as _
}

/// 게임이 종료를 요청했는지 폴링. true면 호스트가 nativeStop 후 라이브러리로 복귀.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_parkjeongseop_wipi_WipiNative_nativePollExit(_env: JNIEnv, _class: JClass) -> jboolean {
    session::take_exit_requested() as jboolean
}

/// 게임 패키지에서 표지 아이콘 PNG를 추출 (없으면 null)
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_parkjeongseop_wipi_WipiNative_nativeGameIcon<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass,
    game_data: JByteArray,
) -> jni::sys::jbyteArray {
    extract_blob(&mut env, game_data, |meta| meta.icon_png)
}

/// 게임 패키지에서 게임명 raw 바이트(EUC-KR)를 추출 (없으면 null)
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_parkjeongseop_wipi_WipiNative_nativeGameName<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass,
    game_data: JByteArray,
) -> jni::sys::jbyteArray {
    extract_blob(&mut env, game_data, |meta| meta.name_euckr)
}

fn extract_blob(env: &mut JNIEnv, game_data: JByteArray, pick: fn(wipi_core::GameMetadata) -> Option<Vec<u8>>) -> jni::sys::jbyteArray {
    let result = (|| -> anyhow::Result<Option<jni::sys::jbyteArray>> {
        let buf = env.convert_byte_array(&game_data)?;
        let Some(blob) = pick(wipi_core::extract_metadata(&buf)) else {
            return Ok(None);
        };
        Ok(Some(env.byte_array_from_slice(&blob)?.into_raw()))
    })();

    match result {
        Ok(Some(array)) => array,
        Ok(None) => std::ptr::null_mut(),
        Err(e) => {
            tracing::error!("extract_blob: {e}");
            std::ptr::null_mut()
        }
    }
}

/// Initializes logcat tracing and the ndk-context (required by cpal's AAudio
/// backend). Must be called once with the application Context before nativeStart.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_parkjeongseop_wipi_WipiNative_nativeInit(env: JNIEnv, _class: JClass, context: JObject) {
    if let Ok(vm) = env.get_java_vm()
        && let Ok(context_ref) = env.new_global_ref(&context)
    {
        unsafe {
            ndk_context::initialize_android_context(vm.get_java_vm_pointer() as *mut _, context_ref.as_obj().as_raw() as *mut _);
        }
        // 앱 수명 내내 유효해야 하므로 의도적으로 릴리스하지 않음
        std::mem::forget(context_ref);
    }

    use tracing_subscriber::layer::SubscriberExt;
    // TRACE 전체를 logcat으로 보내면 에뮬레이션이 수십 배 느려짐 — INFO 이상만
    let subscriber = tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("info"))
        .with(paranoid_android::layer("wie"));
    let _ = tracing::subscriber::set_global_default(subscriber);

    std::panic::set_hook(Box::new(|info| {
        tracing::error!("panic: {info}");
    }));
}
