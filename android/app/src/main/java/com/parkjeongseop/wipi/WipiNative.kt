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

    /**
     * 게임 로드 및 에뮬레이터 스레드 시작. 성공 시 true. soundfontPath가 빈 문자열이면 MIDI 무음.
     * screenMode: 0=자동(내장 표의 타이틀만 선언 크기), 1=선언 크기, 2=240×320.
     */
    external fun nativeStart(gameData: ByteArray, filename: String, dataDir: String, soundfontPath: String, screenMode: Int): Boolean

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

    /** 게임 속도 (퍼센트, 100 = 원래 속도). nativeStart 뒤에 호출. */
    external fun nativeSetSpeed(percent: Int)

    // 메모리 치트 — ARM 코어로 도는 타이틀(KTF·LGT)만 지원. 모두 에뮬레이터 스레드의 응답을 기다리므로
    // (일시정지 중 최대 50ms, 검색은 더 길 수 있음) 메인 스레드에서 부르지 말 것.
    // 주소·값은 u32 비트 패턴을 Int로 주고받는다.

    /** 실행 중인 타이틀이 메모리 치트를 지원하는지 */
    external fun nativeCheatSupported(): Boolean

    /** 값 검색 (width 1·2·4바이트). refine이면 직전 후보를 좁힌다. 일치 수, 미지원이면 -1. */
    external fun nativeCheatSearch(value: Int, width: Int, refine: Boolean): Long

    /** 검색 후보 앞쪽 limit개. 원소마다 상위 32비트 = 주소, 하위 32비트 = 현재 값. */
    external fun nativeCheatResults(limit: Int): LongArray?

    /** 주소의 현재 값 (0..=0xFFFFFFFF), 읽을 수 없으면 -1 */
    external fun nativeCheatRead(address: Int, width: Int): Long

    /** 주소에 값을 한 번 쓴다 */
    external fun nativeCheatWrite(address: Int, width: Int, value: Int): Boolean

    /** 값을 쓰고 nativeCheatUnfreeze까지 고정한다 (매 tick 다시 씀) */
    external fun nativeCheatFreeze(address: Int, width: Int, value: Int): Boolean

    external fun nativeCheatUnfreeze(address: Int)

    /** 직전 1초 동안 에뮬레이터 스레드가 게임을 실행한 비율(0~100). 100에 가까우면 기기 성능이 한계 */
    external fun nativeCpuLoad(): Int

    /** 게임이 종료를 요청했는지 폴링. true면 nativeStop 후 라이브러리로 복귀. */
    external fun nativePollExit(): Boolean

    /** 게임 패키지에서 표지 아이콘 PNG 추출 (없으면 null) */
    external fun nativeGameIcon(gameData: ByteArray): ByteArray?

    /** 게임이 선언한 화면 크기 ((너비 shl 16) or 높이). 선언이 없거나 240×320이면 0 — 화면 크기 설정을 보일지 판단용. */
    external fun nativeGameScreen(gameData: ByteArray): Int

    /** 게임 패키지에서 게임명 raw 바이트(EUC-KR) 추출 (없으면 null) */
    external fun nativeGameName(gameData: ByteArray): ByteArray?
}
