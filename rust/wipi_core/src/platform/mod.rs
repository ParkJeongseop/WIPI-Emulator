mod audio;
mod database;
mod filesystem;
mod lcd;
mod screen;
mod storage;

use std::{
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use wie_backend::{Filesystem, Instant, Platform, Screen, canvas::Font};

pub use lcd::LcdPlatform;
use screen::CaptureScreen;
pub use screen::CapturedFrame;

/// 게임이 요청한 진동 (duration_ms, intensity 0~100).
#[derive(Clone, Copy)]
pub struct VibrationRequest {
    pub duration_ms: u64,
    pub intensity: u8,
}

/// Host platform for mobile targets (and headless testing). All persistent
/// state lives under `base_path` (filesDir on Android, Documents on iOS).
pub struct MobilePlatform {
    /// 게임 텍스트 렌더링용 폰트 (wie 데스크톱과 동일한 neodgm — 글자 규격 12px 기준).
    font: Font,
    screen: CaptureScreen,
    filesystem: filesystem::FsFilesystem,
    database_repository: database::FsDatabaseRepository,
    audio_engine: audio::AudioEngine,
    /// 최근 진동 요청. 호스트가 폴링으로 소비한다 (콜백 없는 설계 유지).
    vibration: Mutex<Option<VibrationRequest>>,
    /// 게임이 종료를 요청했는지. 호스트가 폴링으로 감지해 세션을 정리한다.
    exit_requested: AtomicBool,
    /// 가상 시계 (epoch ms). 0이면 비활성(실시간). now() 호출마다 1ms 전진하는
    /// 결정적 시계 — flaky(타이밍 레이스) 재현/차단 실험용 (headless 전용).
    virtual_clock_ms: AtomicU64,
    /// 게임 속도 배율이 적용된 시계
    game_clock: Mutex<GameClock>,
}

/// 게임이 보는 시계. 기준점(anchor) 이후 흐른 실제 시간에 속도 배율을 곱해 더한다 —
/// 속도를 바꿀 때 기준점을 현재로 옮겨 시각이 튀지 않게 한다. 100%에선 실제 시각 그대로.
struct GameClock {
    speed_percent: u64,
    real_anchor_ms: u64,
    game_anchor_ms: u64,
}

impl GameClock {
    fn now_ms(&self, real_ms: u64) -> u64 {
        self.game_anchor_ms + real_ms.saturating_sub(self.real_anchor_ms) * self.speed_percent / 100
    }

    fn set_speed(&mut self, real_ms: u64, percent: u32) {
        *self = Self {
            speed_percent: percent.clamp(10, 1000) as u64,
            real_anchor_ms: real_ms,
            game_anchor_ms: self.now_ms(real_ms),
        };
    }
}

fn real_now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as _
}

impl MobilePlatform {
    pub fn new(base_path: PathBuf, width: u32, height: u32, soundfont_path: Option<PathBuf>) -> Self {
        Self {
            font: Font::try_from_static(include_bytes!("../../assets/neodgm.ttf")).expect("bundled font"),
            screen: CaptureScreen::new(width, height),
            filesystem: filesystem::FsFilesystem::new(base_path.join("fs")),
            database_repository: database::FsDatabaseRepository::new(base_path.join("db")),
            audio_engine: audio::AudioEngine::new(soundfont_path.as_deref()),
            vibration: Mutex::new(None),
            exit_requested: AtomicBool::new(false),
            virtual_clock_ms: AtomicU64::new(0),
            game_clock: Mutex::new(GameClock {
                speed_percent: 100,
                real_anchor_ms: 0,
                game_anchor_ms: 0,
            }),
        }
    }

    /// 게임 속도 (퍼센트, 100 = 원래 속도, 10~1000으로 제한). 타이머·sleep이 그 배율로 흐른다.
    pub fn set_speed(&self, percent: u32) {
        self.game_clock.lock().unwrap().set_speed(real_now_ms(), percent);
    }

    /// 가상 시계 활성화 — now()가 실시간 대신 호출마다 1ms 전진하는 결정적 시계를
    /// 반환하게 한다. start_epoch_ms는 게임이 보는 시작 시각 (0 금지 — 비활성 의미).
    pub fn enable_virtual_clock(&self, start_epoch_ms: u64) {
        self.virtual_clock_ms.store(start_epoch_ms, Ordering::SeqCst);
    }

    /// 가상 시계의 현재 값(epoch ms). 시계를 전진시키지 않는다. 비활성이면 None.
    pub fn virtual_now_ms(&self) -> Option<u64> {
        match self.virtual_clock_ms.load(Ordering::SeqCst) {
            0 => None,
            x => Some(x),
        }
    }

    pub fn screen_capture(&self) -> &CaptureScreen {
        &self.screen
    }

    /// 보류 중인 진동 요청을 가져오고 지운다.
    pub fn take_vibration(&self) -> Option<VibrationRequest> {
        self.vibration.lock().unwrap().take()
    }

    /// 게임의 종료 요청이 있었는지 확인하고 플래그를 지운다.
    pub fn take_exit_requested(&self) -> bool {
        self.exit_requested.swap(false, Ordering::SeqCst)
    }

    /// 볼륨 설정 (0.0~1.0, 0이면 음소거) — PCM(효과음)/MIDI(배경음악) 분리, 호스트 사운드 설정용
    pub fn set_volume(&self, pcm: f32, midi: f32) {
        self.audio_engine.set_volume(pcm, midi);
    }
}

/// MobilePlatform을 Arc로 공유하기 위한 래퍼 — 에뮬레이터가 Box<dyn Platform>을
/// 소유하는 동안에도 호스트(UI/테스트 하니스)가 화면 캡처에 접근할 수 있게 함
pub struct SharedPlatform(pub std::sync::Arc<MobilePlatform>);

impl Platform for SharedPlatform {
    fn font(&self) -> &Font {
        self.0.font()
    }
    fn screen(&self) -> &dyn Screen {
        self.0.screen()
    }
    fn now(&self) -> Instant {
        self.0.now()
    }
    fn database_repository(&self) -> &dyn wie_backend::DatabaseRepository {
        self.0.database_repository()
    }
    fn filesystem(&self) -> &dyn Filesystem {
        self.0.filesystem()
    }
    fn audio_sink(&self) -> Box<dyn wie_backend::AudioSink> {
        self.0.audio_sink()
    }
    fn write_stdout(&self, buf: &[u8]) {
        self.0.write_stdout(buf)
    }
    fn write_stderr(&self, buf: &[u8]) {
        self.0.write_stderr(buf)
    }
    fn exit(&self) {
        self.0.exit()
    }
    fn vibrate(&self, duration_ms: u64, intensity: u8) {
        self.0.vibrate(duration_ms, intensity)
    }
}

impl Platform for MobilePlatform {
    fn font(&self) -> &Font {
        &self.font
    }

    fn screen(&self) -> &dyn Screen {
        &self.screen
    }

    fn now(&self) -> Instant {
        // 가상 시계 활성 시: 호출마다 1ms 전진 (실행 간 완전 결정적)
        let virtual_ms = self.virtual_clock_ms.load(Ordering::SeqCst);
        if virtual_ms != 0 {
            let t = self.virtual_clock_ms.fetch_add(1, Ordering::SeqCst);
            return Instant::from_epoch_millis(t);
        }

        Instant::from_epoch_millis(self.game_clock.lock().unwrap().now_ms(real_now_ms()))
    }

    fn database_repository(&self) -> &dyn wie_backend::DatabaseRepository {
        &self.database_repository
    }

    fn filesystem(&self) -> &dyn Filesystem {
        &self.filesystem
    }

    fn audio_sink(&self) -> Box<dyn wie_backend::AudioSink> {
        Box::new(self.audio_engine.sink())
    }

    fn write_stdout(&self, buf: &[u8]) {
        tracing::info!("stdout: {}", String::from_utf8_lossy(buf));
    }

    fn write_stderr(&self, buf: &[u8]) {
        tracing::warn!("stderr: {}", String::from_utf8_lossy(buf));
    }

    fn exit(&self) {
        tracing::info!("app requested exit");
        self.exit_requested.store(true, Ordering::SeqCst);
    }

    fn vibrate(&self, duration_ms: u64, intensity: u8) {
        tracing::info!("vibrate({duration_ms}ms, {intensity}%)");
        *self.vibration.lock().unwrap() = Some(VibrationRequest { duration_ms, intensity });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_clock_scales_time_without_jumping_on_speed_change() {
        let mut clock = GameClock {
            speed_percent: 100,
            real_anchor_ms: 0,
            game_anchor_ms: 0,
        };
        // 100%에선 실제 시각 그대로
        assert_eq!(clock.now_ms(1_700_000_000_000), 1_700_000_000_000);

        // 3배속: 바꾼 순간은 그대로, 그 뒤 1초가 3초로 흐른다
        clock.set_speed(1_700_000_000_000, 300);
        assert_eq!(clock.now_ms(1_700_000_000_000), 1_700_000_000_000);
        assert_eq!(clock.now_ms(1_700_000_001_000), 1_700_000_003_000);

        // 0.5배속으로 바꿔도 이어서 흐른다
        clock.set_speed(1_700_000_001_000, 50);
        assert_eq!(clock.now_ms(1_700_000_003_000), 1_700_000_004_000);

        // 범위를 벗어난 배율은 10%~1000%로 제한
        clock.set_speed(1_700_000_003_000, 0);
        assert_eq!(clock.now_ms(1_700_000_004_000), 1_700_000_004_100);
    }
}
