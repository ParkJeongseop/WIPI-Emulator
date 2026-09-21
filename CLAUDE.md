# wie-mobile — 한국 피처폰 에뮬레이터 모바일 포팅

[dlunch/wie](https://github.com/dlunch/wie)(WIPI/SKVM/J2ME 에뮬레이터, Rust)를 Android/iOS 네이티브 앱으로 포팅하는 프로젝트.

## 핵심 결정사항 (변경 시 근거 필요)

- **네이티브 방식**: Rust 코어를 타깃별 네이티브로 빌드해 링크. RN/Flutter/Tauri 아님.
  - iOS는 JIT 금지라 RustJava(순수 Rust 인터프리터) 내장이 유일하게 깔끔한 경로.
- **wie 소비 전략**(2026-07-21 갱신): 본인 포크 **ParkJeongseop/wie** + rev 고정으로 소비.
  - 포크 main은 upstream(dlunch/wie) 추적, 기능은 브랜치로 작업 → upstream PR + 앱은 포크 rev 갱신으로 먼저 적용.
  - upstream 머지 후엔 해당 upstream rev로 복귀 (포크는 "미머지 델타"만 담는 얇은 층 유지 — 하드포크 금지).
  - 현재 rev: 325b6171 (sync/upstream-2026-09-21 — upstream v0.1.4 동기화+우리 델타). 로컬 ../wie 클론은 코어 디버깅 시 path 전환용.
- **FFI는 wie_app의 WieWeb 4-메서드 모델**을 따름: start / getFrame(폴링) / keyDown / keyUp.
  콜백 없음 — UI가 60fps로 프레임을 폴링. 키코드는 문자열("UP","OK","1","*","SOFT_L"...).
- **라이선스**: wie·RustJava·smaf·wipi 전부 MIT (저작권자 Inseok Lee). 고지 의무만 있음.
  사운드폰트 GeneralUser-GS는 상용배포 가능 라이선스. 게임 파일은 절대 동봉 금지(사용자 임포트만).

## 저장소 구조

```
wie-mobile/
├─ rust/                    # Rust 워크스페이스
│  ├─ Cargo.toml            # wie 크레이트를 path 의존성으로 (../wie — 로컬 클론 필요!)
│  │                        #   [profile.release-ios] = LTO off (Xcode ld와 LLVM 버전 충돌 회피)
│  ├─ Cargo.lock            # ★ wie/Cargo.lock 복사본 — RustJava 등 git rev 고정용. 재생성 금지
│  ├─ .cargo/config.toml    # Android NDK 링커 설정 (android26-clang — AAudio 때문에 26 필수, Windows 경로)
│  ├─ wipi_core/      # ★ 공통 코어 (Android/iOS 공유)
│  │  ├─ src/lib.rs         # create_emulator() — 포맷 자동감지 (wie_cli start()와 동일 분기)
│  │  ├─ src/session.rs     # 에뮬레이터 전용 스레드 + 키repeat(100ms) + 프레임/오류 폴링 API
│  │  └─ src/platform/      # MobilePlatform: screen(프레임캡처)/filesystem/database(wie_cli 이식)/audio
│  ├─ wipi_android/          # JNI 브리지 (Java_com_parkjeongseop_wipi_WipiNative_*) — session의 얇은 래퍼
│  │  └─ src/bin/headless.rs # UI 없이 검증하는 테스트 하니스 (프레임을 BMP로 덤프, macOS에서도 실행 가능)
│  └─ wipi_ios/              # C ABI 브리지 (wipi_init/start/get_frame/key_down/key_up/get_error/stop)
│     └─ include/wipi_ios.h  # 수기 관리 헤더 (Swift 브리징 헤더가 include)
├─ android/                 # Gradle 프로젝트 (AGP 8.10.1, Kotlin 2.1.21, Compose)
│  └─ app/src/main/
│     ├─ java/com/parkjeongseop/wipi/  # MainActivity(Compose UI+키패드+SAF), WipiNative(JNI 선언)
│     ├─ jniLibs/{arm64-v8a,x86_64}/libwipi_android.so  # cargo 빌드 후 수동 복사
│     └─ assets/GeneralUser-GS.sf2  # MIDI 사운드폰트 (첫 실행 시 filesDir로 복사)
└─ ios/                     # Xcode 프로젝트 (xcodegen — project.yml에서 생성, .xcodeproj는 산출물)
   └─ WipiEmulator/               # SwiftUI: ContentView(라이브러리↔에뮬 전환), EmulatorScreenView(60fps 폴링→CGImage),
                            # Keypad(Android와 동일 배치), WipiCore(C ABI 래퍼), Resources/GeneralUser-GS.sf2
```

전제: 형제 디렉토리에 `../wie` 클론 필요 (path 의존성). `../wie_app`, `../RustJava`, `../smaf`도 참고용으로 클론되어 있음.

## Android 빌드 (Mac 기준 — 2026-07-21 검증됨. Windows는 .cargo/config.toml 주석 참고)

```sh
# Rust (rust/ 에서) — .cargo/config.toml이 Mac NDK(darwin-x86_64) 경로로 설정됨
cargo build --release --target aarch64-linux-android   # 실기기
cargo build --release --target x86_64-linux-android    # AVD
# → .so를 android/app/src/main/jniLibs/<abi>/에 복사

# APK (android/ 에서) — AGP 8.10.1은 Gradle 8.x 필요 → brew의 gradle@8 사용 (gradle 9 비호환)
export JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"
/opt/homebrew/opt/gradle@8/bin/gradle assembleDebug
# → app/build/outputs/apk/debug/app-debug.apk

# 설치·테스트 — autoload 훅은 제거됨(2026-07-21). 라이브러리에 게임을 주입하고 탭으로 실행:
adb install -r app/build/outputs/apk/debug/app-debug.apk
adb push <게임.zip> /data/local/tmp/game.zip
adb shell "run-as com.parkjeongseop.wipi sh -c 'mkdir -p files/games/test && cp /data/local/tmp/game.zip files/games/test/game.zip && echo {\\\"name\\\":\\\"테스트\\\",\\\"filename\\\":\\\"game.zip\\\"} > files/games/test/meta.json'"
adb shell am start -n com.parkjeongseop.wipi/.MainActivity
adb shell input tap <표지좌표>   # 라이브러리에서 게임 탭
```
주의: local.properties의 sdk.dir도 OS별로 다름 (Mac: ~/Library/Android/sdk).

## iOS 빌드 (Mac)

```sh
# Rust (rust/ 에서) — release-ios 프로필 필수 (LTO off)
cargo build --profile release-ios --target aarch64-apple-ios-sim -p wipi_ios   # 시뮬레이터
cargo build --profile release-ios --target aarch64-apple-ios -p wipi_ios      # 실기기

# Xcode 프로젝트 생성 + 빌드 (ios/ 에서)
xcodegen generate
xcodebuild -project WipiEmulator.xcodeproj -scheme WipiEmulator -sdk iphonesimulator \
  -destination 'platform=iOS Simulator,name=iPhone 17' -derivedDataPath build build

# 시뮬레이터 테스트 — autoload 훅은 제거됨(2026-07-21). 라이브러리에 게임 주입 후 탭으로 실행:
xcrun simctl boot "iPhone 17"; xcrun simctl install "iPhone 17" build/Build/Products/Debug-iphonesimulator/WipiEmulator.app
DATA=$(xcrun simctl get_app_container "iPhone 17" com.parkjeongseop.wipi data)
mkdir -p "$DATA/Documents/games/test" && cp <게임.zip> "$DATA/Documents/games/test/game.zip"
printf '{"name":"테스트","filename":"game.zip"}' > "$DATA/Documents/games/test/meta.json"
xcrun simctl launch --console-pty "iPhone 17" com.parkjeongseop.wipi   # --console-pty로 tracing 로그(stderr) 확인
# 탭은 cliclick(brew) 사용 — 시뮬레이터 창 좌표로 클릭
```

## 현재 상태 (2026-07-18)

완료: 화면(240×320 프레임 폴링→Bitmap), 터치 키패드, 파일 선택(SAF), 게임 저장(파일시스템/DB),
오디오(PCM=rodio/AAudio, MIDI=rustysynth+사운드폰트). 리듬스타1(KTF)로 플레이 검증됨.

해결된 이슈: AVD에서 BGM이 비프음으로 깨지던 문제 — 코드 아님. 스냅샷 복원 + 반복 재설치로
에뮬레이터 오디오 HAL이 열화된 것. **AVD에서 소리가 이상하면 콜드 부팅(`-no-snapshot-load`)부터 시도.**
물리 키보드 지원(hardwareKeyMap)과 진단 로그 제거 모두 최종 상태로 복원됨.

## 함정 모음 (시간 아끼는 지식)

- **Cargo.lock**: wie/Cargo.lock을 복사해 시드로 사용(버전 정렬). RustJava는 crates.io +
  [patch.crates-io]로 포크 rev 고정(f414a98, RustJava 0.2.0 기반) — **wie와 앱의 patch 섹션을 항상 일치**시킬 것.
- **logcat TRACE 로그 = 에뮬레이션 수십 배 감속**. nativeInit에서 EnvFilter "info" 고정.
- **cpal(AAudio)**: ①링커가 android26+ 이어야 함(-laaudio) ②ndk-context 초기화 필수 —
  일반 JNI 앱은 자동 초기화가 없어서 nativeInit(context)에서 initialize_android_context() 호출.
  빠뜨리면 오디오 스레드가 조용히 panic (증상: 소리 없음, "audio output opened" 로그 부재).
- **0ms 탭은 게임이 못 잡음** (adb input tap). 실터치/`input swipe x y x y 300`은 OK.
- **테스트 자동화**: autoload 훅은 제거됨(2026-07-21) — 라이브러리(files/games/)에 run-as로 게임 주입 후 탭.
  게임 데이터 초기화: `run-as com.parkjeongseop.wipi rm -rf files/games/<id>/data`.
- 게임은 시간 기반 로딩 — 화면 검다고 바로 판단 금지, 몇 초 기다릴 것.
- **headless 세이브 오염**: headless는 출력 prefix의 부모 디렉토리에 `wie_data/`(세이브)를 만든다.
  여러 실행이 같은 디렉토리에 prefix만 달리해 쓰면 세이브를 공유해 **실행마다 다른 장면**이 나온다
  (스토리 진행 게임에서 로그와 프레임이 안 맞는 원인). 결정적 재현이 필요하면 실행마다 전용 디렉토리를 새로 만들 것.
- AVD에서 물리 키보드가 앱에 안 들어오면 config.ini의 `hw.keyboard = yes` 확인 (avdmanager 기본값 no).
- (Windows 한정) 실기기 SM-G977N은 USB adb가 지속 전송 ~5초마다 끊김(원인 미상, 케이블 교체 필요).
  cargo-ndk는 windows-gnu 툴체인에서 설치 불가 → .cargo/config.toml에 링커 직접 지정으로 대체.

## iOS 포팅 현황 (2026-07-18)

완료 (실기기 검증됨):
- **wipi_core 추출**: platform(screen/filesystem/database/audio) + session(스레드/키repeat/폴링)을
  공통 크레이트로. wipi_android는 JNI 변환만, wipi_ios는 C ABI 변환만 하는 얇은 래퍼.
- **wipi_ios C ABI**: wipi_init/start/get_frame(RGBA)/key_down/key_up/get_error/poll_vibrate/stop — include/wipi_ios.h.
- **SwiftUI 앱**: ios/ (xcodegen). 런처(fileImporter)+에뮬레이터 화면(60fps Timer 폴링→CGImage)+키패드
  +진동(Core Haptics)+설정(SettingsView). UIFileSharingEnabled로 게임 임포트. (autoload 훅은 2026-07-21 제거)
- **설정**: 진동 On/Off+세기 슬라이더(0~150%), 사운드 On/Off+볼륨 슬라이더(@AppStorage). RetroArch 방식 —
  게임 원본 값에 사용자 배율만 곱함(코어 불변). 라이브러리/에뮬화면(우상단 톱니) 양쪽에서 sheet로 접근.
  볼륨은 **PCM(효과음)/MIDI(배경음악) 분리 조절**(2026-07-21, 웹버전 volume-pcm/volume-midi와 동등) —
  사운드폰트 음량과 게임 내장 샘플 음량이 게임마다 달라 밸런스 보정 필요. AudioCommand::SetVolume{pcm,midi}
  →각 Player.set_volume. FFI: wipi_set_volume(pcm,midi) / nativeSetVolume(pcm,midi). 설정 키 pcmVolume/midiVolume.
- **키패드 햅틱**(2026-07-21): 화면 키패드 키 다운 시 가벼운 탭. 설정 토글 keypadHaptics(기본 ON, 진동 섹션).
  게임 진동과 독립(호스트 UI 피드백), 하드웨어 키보드/게임패드에는 미적용(물리 피드백 존재).
  Android: performHapticFeedback(VIRTUAL_KEY). iOS: **CHHapticEngine 트랜지언트로 재생** —
  ★함정: UIImpactFeedbackGenerator는 CHHapticEngine(게임 진동용)이 가동 중이면 무시됨.
  키패드는 게임 화면에만 있어 항상 충돌 → 같은 엔진의 캐시된 tapPlayer로 통일 (Haptics.tap()).
- **게임 컨트롤러/키보드**(ControllerInput.swift): GCController(dpad·스틱→방향, A→OK, B→CLR, X/Y→*/#,
  숄더→소프트키) + GCKeyboard(Android hardwareKeyMap과 동일: 123/QWE/ASD/ZXC→키패드, Enter·Space→OK,
  Backspace→CLR, Shift→소프트키, F1/F2→CALL/HANGUP). 게임 화면에서만 활성(onAppear/onDisappear).
- **브랜딩**: 앱명 "WIPI 에뮬레이터"(홈 화면 CFBundleDisplayName "WIPI 에뮬"), 번들 ID **com.parkjeongseop.wipi**
  (구 dev.wie.app에서 변경 — "wie" 회피. 출시 후엔 번들 ID 영구라 출시 전에 확정함. 기기의 구 앱은 제거됨).
  WIPI는 TTA 기술표준명이라 사용 무방(J2ME Loader 선례). "wie"는 원작자 프로젝트명이므로 브랜드에 사용하지 않음
  (고지 화면 크레딧은 별도). Android도 namespace/JNI 심볼까지 com.parkjeongseop.wipi로 통일함(2026-07-21).
  아이콘: 픽셀 폴더폰(주황/청록, 화면에 하트+음표) — ios/make_icon.py로 생성한 오리지널 아트(재생성 가능),
  Assets.xcassets/AppIcon 1024 단일 사이즈. 부제(앱스토어): "한국 피처폰 게임 에뮬레이터" 예정.
- **게임 라이브러리**: 표지 그리드가 메인 화면(LibraryView/GameLibrary). 콘솔 에뮬과 달리 WIPI 패키지는
  표지·이름이 zip 안에 동봉됨 — big.icon(PNG)/__adf__의 Name을 코어에서 추출(wipi_game_icon/wipi_game_name).
  게임명은 EUC-KR(CP949)이라 Swift에서 디코딩. 임포트 시 Documents/games/<UUID>/에 복사 + 표지/이름 캐시,
  게임별 세이브(data/)라 삭제 시 함께 제거. + 버튼 임포트, 롱프레스 삭제, 탭 실행. 저해상도 아이콘은 nearest 확대.
  (참고: extract_metadata 스모크 테스트는 WIE_TEST_ROM 환경변수로 실제 게임 지정 시 실행 — wipi_core/src/lib.rs)
- **게임 종료(exit) 연결**: 코어의 platform().exit()(게임 종료 API) → MobilePlatform이 플래그 세팅 →
  session::take_exit_requested → wipi_poll_exit(폴링) → EmulatorScreenView onExit → 에뮬 정지 후 라이브러리 복귀.
  진동과 동일 패턴. **검증**: 즉시 exit하는 helloworld를 실행하면 잠깐 로딩 후 라이브러리로 자동 복귀 확인.
- **호스트 위생 (티어1)**: ①화면 자동잠금 방지(EmulatorScreenView onAppear에서 isIdleTimerDisabled=true, 종료 시 false)
  ②오디오 세션(AudioSession.swift, .playback — 무음스위치 무관 재생. configure는 앱 시작, activate/deactivate는 게임 화면 진입/종료)
  ③백그라운드 전환(scenePhase onChange — 백그라운드 시 세션 deactivate, 복귀 시 activate). 실제 체감(화면/스피커/홈복귀)은 실기기 확인 필요.
- **일시정지 메뉴**: scenePhase background → wipi_set_paused(true) → session tick 루프 정지(게임 세계 얼림,
  에뮬 표준 방식). **복귀해도 자동 재개하지 않고** 오버레이 유지 — 탭하면 재개, **[게임 종료] 버튼으로 라이브러리 복귀**
  (2026-07-21 추가 — Delta/PPSSPP처럼 일시정지 메뉴가 종료 허브. iOS는 이게 유일한 인앱 종료 수단).
  수동 일시정지 버튼(⏸) 우상단. Android back: 즉시 종료가 아니라 **일시정지 열기, 일시정지 중 back 한 번 더 = 종료**
  (실수 종료 방지, Android 에뮬 관례). 검증: 시뮬에서 백그라운드→복귀 시 오버레이+프레임 정지 확인.
  한계: 게임에게 알리는 정식 WIPI 생명주기(Jlet pauseApp 등)는 코어 Event enum에 Pause가 없어 불가 — wie 포크 필요.
  또한 platform.now()가 실제 epoch라 재개 시 게임이 보는 시간은 점프함(멈춘 동안의 타이머가 일괄 발화).
- **진동**: 코어의 Platform::vibrate → MobilePlatform이 요청 저장 → session::take_vibration →
  wipi_poll_vibrate(폴링, 콜백 없는 설계 유지) → Swift Haptics(CHHapticEngine). **WIPI API 충실**:
  진동 세기는 API에 level 0~10으로 존재(MC_mdaVibrator/Vibrator.on/Vibration.start + getLevelNum).
  duration을 지정 시간만큼 연속 진동(상한 5초), intensity(0~100)를 0.5+0.5x로 반영(단계 구분 유지, 최소 0.55 보정),
  duration/intensity가 0이면 skip(진동 끄기 신호 — SKVM stop()이 vibrate(0,0) 호출). Android는 아직 로그 스텁(대칭 미구현).
- **검증**: **실기기(iPhone 16, iOS 26.5.2)에서 리듬스타1(KTF) 실행 확인** — 타이틀 렌더링, 사운드폰트/CoreAudio,
  WIPI C API 활발히 호출(MC_mdaClipSetVolume 등), 50초+ error/panic 0건. 시뮬레이터에선 helloworld_ktf/lgt 검증.

iOS 함정 모음:
- **release-ios 프로필(LTO off) 필수**: lto=true 산출물은 rustc LLVM과 Xcode ld의 버전 차이로
  비트코드 호환 문제 위험 (Apple nm도 오브젝트를 못 읽음). 링크는 -lwipi_ios + AudioToolbox/CoreAudio/CoreHaptics.
- 예상대로 cpal(CoreAudio)은 iOS에서 초기화 없이 그대로 동작 (ndk-context 불필요).
- 시뮬레이터 로그는 `simctl launch --console-pty`, 실기기는 `devicectl device process launch --console`로 stderr 확인.
- **C ABI에 심볼 추가 시**: 디바이스+시뮬레이터 .a 둘 다 재빌드해야 링크 심볼이 일치 (안 하면 한쪽 빌드 깨짐).

실기기 배포 절차 (자동 서명, Personal Team 7WYMPDQCGP):
```sh
cargo build --profile release-ios --target aarch64-apple-ios -p wipi_ios   # rust/ 에서
xcodegen generate                                                          # ios/ 에서
xcodebuild -project WipiEmulator.xcodeproj -scheme WipiEmulator -sdk iphoneos \
  -destination 'platform=iOS,id=<UDID>' -derivedDataPath build-device -allowProvisioningUpdates build
xcrun devicectl device install app --device <UDID> build-device/Build/Products/Debug-iphoneos/WipiEmulator.app
# 게임 주입: 라이브러리 폴더(Documents/games/<id>/)로 복사 (autoload 훅은 제거됨)
xcrun devicectl device copy to --device <UDID> --domain-type appDataContainer \
  --domain-identifier com.parkjeongseop.wipi --user mobile --source <로컬 games 폴더> --destination Documents/games
xcrun devicectl device process launch --device <UDID> --console com.parkjeongseop.wipi
```
- 기기 UDID: `xcrun devicectl list devices` (available/paired) 또는 `device info details`의 udid.
- 실기기 스크린샷 CLI 없음(idevicescreenshot 미설치, devicectl에 screenshot 없음) — 필요시 libimobiledevice 설치.

- **오픈소스 고지**(LicensesView.swift): 설정→정보→오픈소스 라이선스. wie/RustJava/smaf(MIT, Inseok Lee),
  rodio(MIT)/cpal(Apache-2.0), rustysynth(MIT), GeneralUser GS(자체 무료 라이선스) — 원문 포함, dlunch 크레딧.
  라이선스 원문은 각 저장소/cargo 캐시에서 확인한 실제 저작권 표기 사용.

남은 것 (출시 전):
1. 진동 실체감 최종 확인 — 리듬스타 곡 플레이 진입해야 vibrate 호출됨(타이틀/메뉴에선 0건)
2. 티어1(화면잠금/오디오세션/백그라운드/exit) 실기기 체감 확인 — 코드는 완료, 시뮬 컴파일/실행 검증됨
3. **백그라운드 오디오 재개 한계**: 앱 백그라운드 시 세션 deactivate하지만, 복귀 후 cpal(AudioUnit) 스트림이
   자동 재개 안 될 수 있음. 소리 안 나오면 오디오 스레드/스트림 재시작 필요(Rust 쪽 과제).
4. 앱스토어 제출: App Store Connect 앱 생성(이름 중복 확인), 스크린샷, 부제 "한국 피처폰 게임 에뮬레이터"
5. (선택) XCFramework 묶음, CADisplayLink 전환

## 데모 게임 (2026-07-21, 심사·첫경험용)

### WIPI C(zip) 버전 — 내장 데모 (2026-07-21 추가)

"jar는 순수 J2ME 아니냐" 지적으로 **정통 WIPI C(Clet) 데모**를 추가 제작 (demo/wipi_c/):
- **dlunch/wipi SDK**(Rust로 WIPI 앱 작성, MIT)로 heart_catch.rs 작성 → nightly + -Zbuild-std +
  **thumbv4t-none-eabi**(ARM) 빌드 → wipi_archiver로 KTF zip 포장 → big.icon/Name(EUC-KR) 후처리.
  빌드 재현: demo/wipi_c/build.sh (SDK 클론→주입→빌드→포장).
- **배포 방식 변경(2026-07-21 사용자 결정): 내장하지 않음 — 둘 다 임포트 전용.** 시드 로직/번들 리소스 제거.
  demo_wipi.zip(KTF C)과 demo.jar(J2ME) 모두 demo/에만 존재, 심사 노트에 두 링크+임포트 절차 기재 예정
  → 심사관이 zip/jar 두 형식과 WIPI-C/J2ME 두 경로, 그리고 임포트 기능 자체를 검증.
  라이브러리 빈 상태가 기본 첫 화면(안내 문구 존재). 기기에 남아 있던 시드 엔트리: 갤럭시는 제거,
  iPhone은 일반 항목으로 남음(롱프레스 삭제 가능).
- ★학습: KTF·LGT 포장 zip은 **ARM 바이너리 필수**(client.bin/binary.mod 하드 요구) — 순수 자바 WIPI(Jlet)
  데모는 현 wie 구조상 불가 (org.kwis Main 부트스트랩이 KTF ARM 경로 안에만 있음).
- ★SDK 버그 발견(기여 후보): wipi SDK color_to_pixel이 fb.bpp 무시하고 ARGB8888 고정 패킹 —
  wie는 16bpp(RGB565)라 색 깨짐. 데모는 RGB565 값을 하위 16비트에 인코딩해 우회 (heart_catch.rs rgb() 참고).
- SDK의 App 트레이트에 on_pause/on_resume 존재 — WIPI 생명주기 통지 구현 시 참고.

### J2ME(jar) 버전 — 심사노트 임포트용

demo/ — 자체 제작 J2ME MIDlet "하트 캐치" (MIT, 우리 소유): 좌우/4·6키로 바구니 이동해 하트 받기,
점수·한글 안내 표시. build.sh(javac --release 8 + 스텁 컴파일, RustJava classfile 45~70 지원 확인) → demo.jar.
~~앱에 내장~~(2026-07-21 철회 — 임포트 전용으로 전환, 위 WIPI C 섹션 참고).
**jar 안에 big.icon(128px 픽셀 하트, make_cover.py)+__adf__(EUC-KR 이름) 동봉** — KTF/LGT/SKT jar 감지에
영향 없음(각각 client.bin/binary.mod/매직바이트 검사 확인). 따라서 demo.jar 자체가 **심사관 임포트용 파일**도 겸함:
+ 버튼으로 임포트하면 표지·이름 추출까지 전체 플로우 시연 (심사 노트에 저장소 raw URL 첨부 예정).
검증: headless 프레임(한글 포함), 추출 스모크(icon 326B/name 15B), 시뮬 새설치 시드에 하트 표지 표시.
★발견 1: RustJava에 java.lang.StringBuilder 미구현 — 최신 javac의 문자열 연결(+)이 여기로 컴파일됨.
데모는 StringBuffer 명시 사용으로 우회. **RustJava 기여 후보 1호** (StringBuilder 구현).
★발견 2: 고정핀 RustJava(62cf0c6a)엔 Random.nextInt(int)도 없음 (upstream엔 이후 추가됨) —
게임 스레드가 NoSuchMethodError로 조용히 죽어 "화면 멈춤"으로 나타남. 데모는 자체 LCG로 우회.
교훈: **자바 스레드의 uncaught exception은 앱 에러로 안 뜨고 조용한 프리즈가 됨** (개선 후보 —
uncaught 시 session error로 승격). wie rev 범프 시 nextInt는 자연 해결.

- **버전 통일**(2026-07-21): iOS는 project.yml에 버전 미지정 시 Xcode 기본 1.0이 박힘 → Info.plist properties에
  CFBundleShortVersionString/Version 명시로 Android versionName(0.1.0)과 통일. (설정 화면 버전 표시 불일치로 발견.)
- **스토어 스크린샷**: store/ios(1320×2868, 6.9") · store/android(1080×2400) 각 4컷(라이브러리/게임플레이/일시정지/설정).
  콘텐츠는 자체 데모(하트 캐치)만 — 상용 게임 화면 배제. Android는 AVD(store, android-35 arm64)로 촬영,
  데모 status bar(sysui demo: 9:41/배터리100/알림숨김). iOS는 status_bar override + cliclick(게임화면 톱니 y≈100).
  README 표시용 대표 4컷은 docs/images/.

## i18n (2026-07-21, 영어 기본 + 한국어)

개발/기본 언어 영어, 기기 로케일이 한국어면 한국어. 49개 문자열 마스터 → 4개 리소스 생성(scratchpad/i18n.json 참고 가능):
- **iOS**: WipiEmulator/{en,ko}.lproj/Localizable.strings + InfoPlist.strings(앱명). project.yml developmentLanguage: en.
  SwiftUI Text("key")/Toggle("key")는 LocalizedStringKey 자동 조회. String 필요한 곳(에러/포맷)은
  NSLocalizedString+String(format:) (%@ 인자). LibraryView/SettingsView/LicensesView/ContentView/WipiCore/
  EmulatorScreenView 전부 키로 교체. VolumeRow.label·LicenseEntry.roleKey는 LocalizedStringKey 타입.
- **Android**: res/values/strings.xml(en 기본) + res/values-ko/strings.xml. build.gradle localeFilters=[en,ko].
  Compose는 stringResource(R.string.key), Composable 밖(에러 문구)은 getString(R.string.key, arg).
  라이선스 role은 License.roleRes: Int. 매니페스트 android:label=@string/app_name.
- **앱명**: 영어 "WIPI Emu" / 한국어 "WIPI 에뮬" (홈 화면 표시명).
- 키 이름은 양 플랫폼 공유(snake_case): settings_*, error_*, action_*, emulator_*, library_*, license_role_* 등.
- **검증**: iOS 시뮬 -AppleLanguages "(en)"/"(ko)"로 설정 화면 영어/한국어 전환 확인. Android AVD 로케일 전환 검증.

## Android 대칭 구현 완료 (2026-07-21)

iOS와 동일 기능 세트가 Android에도 구현됨 (갤럭시 실기기 R3CM409ZZPZ 검증):
- **JNI 추가**: nativePollVibrate(LongArray[2]) / nativeSetPaused / nativeSetVolume / nativePollExit /
  nativeGameIcon / nativeGameName (jni_bridge.rs — wipi_core::session의 얇은 래퍼)
- **UI**(Compose): GameLibrary.kt(임포트/표지·이름 캐시/게임별 세이브 data/), Settings.kt(사운드·진동 설정
  +오픈소스 고지, SharedPreferences), Haptics.kt(Vibrator, 세기조절 불가 기기는 DEFAULT_AMPLITUDE 폴백),
  MainActivity.kt(화면 전환 Library/Emulator/Settings/Licenses + 일시정지 오버레이 + FLAG_KEEP_SCREEN_ON
  + 폴링 통합: 프레임/진동/exit/오류). 게임명 디코딩은 Charset "EUC-KR".
- **브랜딩**: label "WIPI 에뮬", 픽셀 폴더폰 아이콘(mipmap 5종, sips로 icon_1024.png 축소),
  applicationId/namespace com.parkjeongseop.wipi (2026-07-21 wie 명칭 제거 때 JNI 심볼까지 통일). VIBRATE 권한 추가.
- **검증**: 라이브러리(표지+이름) → 탭 실행 → 홈 → 복귀 시 일시정지 오버레이 → 탭 재개까지 실기기 확인.
  설정/고지 화면은 컴파일·코드 검증(시각 확인은 추후). run-as로 files/games/에 직접 게임 주입 가능(테스트).
- 주의: buildConfig=true 필요(설정의 버전 표시). am start 컴포넌트는 com.parkjeongseop.wipi/.MainActivity.

### wie 명칭 제거 (2026-07-21, 공개 레포 준비)

우리 산출물에서 원작자 프로젝트명 "wie"를 제거 (upstream 의존성 이름 wie_*는 당연히 유지):
크레이트 wie_mobile_core/wie_android/wie_ios → **wipi_core/wipi_android/wipi_ios**, C ABI wie_* → **wipi_***,
헤더 wipi_ios.h, JNI Java_dev_wie_app_WieNative_* → **Java_com_parkjeongseop_wipi_WipiNative_***,
Kotlin 패키지 dev.wie.app → com.parkjeongseop.wipi (WieNative→WipiNative, WieTheme→WipiTheme),
iOS 타깃/폴더 WieApp → **WipiEmulator** (WieCore→WipiCore, 스킴/앱 경로 변경 주의).
반대급부로 README에 dlunch/wie 크레딧을 눈에 띄게 유지. GitHub: ParkJeongseop/WIPI-Emulator.

### 디자인 원칙 (2026-07-21 확정)

- **기능·플로우는 iOS가 명세** (어떤 기능이 있고 어떻게 이동하나 — 항목 구성까지 동일하게)
- **look & feel은 각 플랫폼 관례** — iOS는 SwiftUI Form/sheet/SF Symbols, Android는 Material 3
  (Scaffold+TopAppBar+ListItem+카테고리 서브헤더, "완료" 없이 즉시 적용, Material You 동적색상+다크 —
  Theme.kt WipiTheme). 설정 화면을 iOS Form 이식으로 만들었다가 "구리다" 피드백 받고 Material 표준으로 재작성함.
  라이브러리도 TopAppBar(좌정렬 타이틀+우측 액션) 표준 구조.

### iOS 화면 명세 정렬 (2026-07-21, "기능 대칭"을 넘어 "화면 대칭"으로)

원칙: 기능만 맞추고 화면 구성을 재량 처리했던 것을 전수 비교(A버그 2/B동작 7/C시각 6+)로 찾아 정렬:
- 버그 수정: ①emulatorRunning 죽은화면 복귀 버그 — 설정을 화면 전환이 아닌 **오버레이**(iOS sheet 대응)로 바꿔
  구조적으로 해결(에뮬이 뒤에 살아있고 폴링 계속) ②시스템 back → 앱 종료가 아니라 라이브러리 복귀(BackHandler)
- 동작 정렬: ON_PAUSE에서 일시정지(iOS .inactive 대응), 임포트 zip/jar 필터(OpenDocument), 임포트 실패 알럿,
  하단 빨간 에러 배너(탭 닫기), autoload 실패 표시, 게임패드 버튼(BUTTON_A/B/X/Y/L1/R1→OK/CLR/*/#/소프트키)
  +왼스틱(onGenericMotionEvent, 임계 0.5 — iOS GCController와 동일 매핑)
- 화면 정렬: **키패드 겹침 버그 수정**(fillMaxSize+aspectRatio 오버플로 → ContentScale.Fit로 영역 내 레터박스),
  키 눌림 피드백(alpha 0.6), Material 아이콘(material-icons-extended — 이모지 제거), 라이브러리 헤더(톱니/+/큰타이틀),
  빈 상태(아이콘+2줄), 삭제 확인 다이얼로그, 설정 Form 구조(섹션/스피커 아이콘/우상단 완료/버전 행),
  고지 목록→상세 구조. iOS 쪽도 인앱 타이틀 "WIE"→"WIPI 에뮬" 수정(리브랜딩 누락이었음).
- 실기기 검증: 라이브러리 헤더/키패드 분리/back 복귀/설정 Form/고지 상세 스크린샷 확인.
- **세션 자가복구**(session.rs): "게임 탭 → 지원하지 않는 형식" 오보 증상 — 실체는 이전 세션 미정리로
  session::start가 "already running"으로 동기 실패한 것(폴백 메시지가 오해 유발). 수정: 단일 세션 앱이므로
  start가 기존 세션을 만나면 오류 대신 **정지·join 후 새로 시작**(warn 로그 남김). 실행→종료→재실행 3회 검증.
- **에러 표준**(2026-07-21): 코어 take_error가 `SessionError{kind, message}` 반환 — kind는
  LoadFailed(0, create_emulator 실패=형식/손상) | Runtime(1, tick 실패=호환성), message는 영어 진단 원문.
  FFI: iOS `wipi_get_error(buf,cap,out_kind)`, Android `nativeGetError(outKind: IntArray)`.
  호스트가 kind로 한국어 카피 선택(양 플랫폼 동일 문구, iOS WipiCore.describeError / Android pendingError()) +
  둘째 줄에 진단 원문(호환성 제보용). 시작 동기 실패는 "게임을 시작할 수 없어요. 다시 시도해 주세요."
  (형식 언급 제거 — 동기 실패는 형식과 무관). 검증: 가짜 zip → LoadFailed 문구+EOCD 원문 배너, 리듬스타 회귀 OK.
