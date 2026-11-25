package com.parkjeongseop.wipi

import android.content.Context

object WipiNative {
    init {
        System.loadLibrary("wipi_android")
    }

    private var initialized = false

    /** 앱 시작 시 1회 호출 — logcat 로깅 및 오디오(ndk-context) 초기화 */
    fun init(context: Context) {
        if (!initialized) {
            nativeInit(context.applicationContext)
            initialized = true
        }
    }

    private external fun nativeInit(context: Context)

    /** 게임 로드 및 에뮬레이터 스레드 시작. 성공 시 true. soundfontPath가 빈 문자열이면 MIDI 무음. */
    external fun nativeStart(gameData: ByteArray, filename: String, dataDir: String, soundfontPath: String): Boolean

    /** 최신 프레임을 ARGB int 배열로 복사. 새 프레임이 있었으면 true. */
    external fun nativeGetFrame(out: IntArray): Boolean

    external fun nativeKeyDown(key: String)

    external fun nativeKeyUp(key: String)

    /** 보류 중 오류 (없으면 null). outKind[0]: 0=로드 실패, 1=실행 중 오류. 반환값은 영어 진단 원문. */
    external fun nativeGetError(outKind: IntArray): String?

    external fun nativeStop()

    /** 게임의 보류 중 진동 요청. 있으면 true, out(길이 2)에 [durationMs, intensity 0~100]. */
    external fun nativePollVibrate(out: LongArray): Boolean

    /** 에뮬 일시정지/재개 (백그라운드 auto-pause — tick 루프가 얼어붙음) */
    external fun nativeSetPaused(paused: Boolean)

    /** 볼륨 (0.0~1.0, 0이면 음소거) — PCM(효과음)/MIDI(배경음악) 분리, 호스트 사운드 설정 */
    external fun nativeSetVolume(pcm: Float, midi: Float)

    /** 직전 1초 동안 에뮬레이터 스레드가 게임을 실행한 비율(0~100). 100에 가까우면 기기 성능이 한계 */
    external fun nativeCpuLoad(): Int

    /** 게임이 종료를 요청했는지 폴링. true면 nativeStop 후 라이브러리로 복귀. */
    external fun nativePollExit(): Boolean

    /** 게임 패키지에서 표지 아이콘 PNG 추출 (없으면 null) */
    external fun nativeGameIcon(gameData: ByteArray): ByteArray?

    /** 게임 패키지에서 게임명 raw 바이트(EUC-KR) 추출 (없으면 null) */
    external fun nativeGameName(gameData: ByteArray): ByteArray?
}
