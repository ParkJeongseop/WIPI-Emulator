// wipi_ios C ABI(BridgingHeader.h → wipi_ios.h)의 Swift 래퍼.
// Android의 WipiNative와 대칭: start / getFrame(폴링) / keyDown / keyUp / pendingError / stop.

import Foundation

enum WipiCore {
    static let screenWidth = Int(WIPI_SCREEN_WIDTH)
    static let screenHeight = Int(WIPI_SCREEN_HEIGHT)
    static let frameByteCount = screenWidth * screenHeight * 4 // RGBA8888

    /// 앱 시작 시 1회 — 로깅/panic hook 초기화
    static func initialize() {
        wipi_init()
    }

    /// 게임 로드 및 에뮬레이터 스레드 시작. soundfontPath가 빈 문자열이면 MIDI 무음.
    static func start(gameData: Data, filename: String, dataDir: String, soundfontPath: String) -> Bool {
        gameData.withUnsafeBytes { (buf: UnsafeRawBufferPointer) in
            wipi_start(buf.bindMemory(to: UInt8.self).baseAddress, gameData.count, filename, dataDir, soundfontPath)
        }
    }

    /// 최신 프레임을 RGBA로 복사. 새 프레임이 있었으면 true.
    static func getFrame(into buffer: inout [UInt8]) -> Bool {
        buffer.withUnsafeMutableBufferPointer { wipi_get_frame($0.baseAddress, $0.count) }
    }

    static func keyDown(_ key: String) {
        wipi_key_down(key)
    }

    static func keyUp(_ key: String) {
        wipi_key_up(key)
    }

    /// 에뮬레이터 스레드의 보류 중 오류 — 사용자 안내 문구 + 진단 원문으로 변환 (없으면 nil)
    static func pendingError() -> String? {
        var buf = [CChar](repeating: 0, count: 256)
        var kind: UInt8 = 0
        guard wipi_get_error(&buf, buf.count, &kind) else { return nil }
        return describeError(kind: kind, detail: String(cString: buf))
    }

    /// kind → 로컬라이즈된 사용자 문구. 진단 원문(detail)은 둘째 줄(제보/호환성 수집용).
    static func describeError(kind: UInt8, detail: String) -> String {
        let key = kind == 0 ? "error_load_failed" : "error_runtime"
        return String(format: NSLocalizedString(key, comment: ""), detail)
    }

    /// 게임이 요청한 보류 중인 진동 (없으면 nil)
    static func pendingVibration() -> (durationMs: UInt64, intensity: UInt8)? {
        var duration: UInt64 = 0
        var intensity: UInt8 = 0
        guard wipi_poll_vibrate(&duration, &intensity) else { return nil }
        return (duration, intensity)
    }

    /// 직전 1초 동안 에뮬레이터 스레드가 게임을 실행한 비율(0~100). 100에 가까우면 기기 성능이 한계
    static func cpuLoad() -> Int {
        Int(wipi_cpu_load())
    }

    /// 게임이 종료를 요청했는지 (true면 세션 정리 후 라이브러리 복귀)
    static func pendingExit() -> Bool {
        wipi_poll_exit()
    }

    /// 에뮬 일시정지/재개 (백그라운드 auto-pause — tick 루프가 얼어붙음)
    static func setPaused(_ paused: Bool) {
        wipi_set_paused(paused)
    }

    /// 볼륨 (0.0~1.0, 0이면 음소거) — PCM(효과음)/MIDI(배경음악) 분리.
    /// 사운드폰트와 게임 내장 샘플의 음량 차이 보정용 (웹버전과 동일한 분리 조절).
    static func setVolume(pcm: Float, midi: Float) {
        wipi_set_volume(pcm, midi)
    }

    static func stop() {
        wipi_stop()
    }

    /// 게임 패키지에서 표지 아이콘 PNG를 추출 (없으면 nil)
    static func extractIcon(gameData: Data) -> Data? {
        extractBlob(gameData: gameData) { base, len, out, cap in
            wipi_game_icon(base, len, out, cap)
        }
    }

    /// 게임 패키지에서 게임명 raw 바이트(EUC-KR)를 추출 (없으면 nil)
    static func extractNameRaw(gameData: Data) -> Data? {
        extractBlob(gameData: gameData) { base, len, out, cap in
            wipi_game_name(base, len, out, cap)
        }
    }

    /// 크기 협상(out=nil) 후 버퍼를 채우는 공통 패턴
    private static func extractBlob(
        gameData: Data,
        _ call: (_ base: UnsafePointer<UInt8>?, _ len: Int, _ out: UnsafeMutablePointer<UInt8>?, _ cap: Int) -> Int
    ) -> Data? {
        gameData.withUnsafeBytes { raw -> Data? in
            let base = raw.bindMemory(to: UInt8.self).baseAddress
            let size = call(base, gameData.count, nil, 0)
            guard size > 0 else { return nil }

            var out = Data(count: size)
            let written = out.withUnsafeMutableBytes { buf in
                call(base, gameData.count, buf.bindMemory(to: UInt8.self).baseAddress, size)
            }
            return written == size ? out : nil
        }
    }
}
