// 에뮬레이터 화면 + 키패드. 60fps 타이머로 wipi_get_frame을 폴링해 CGImage로 표시.
// Android MainActivity의 EmulatorScreen 컴포저블과 대칭.

import SwiftUI
import UIKit

struct EmulatorScreenView: View {
    let onError: (String) -> Void
    let onExit: () -> Void

    /// 치트 상태 — 게임 한 판 동안 유지 (저장된 치트는 게임 id별로 보관)
    @StateObject private var cheats: CheatModel
    @State private var showCheats = false

    init(gameId: String, onError: @escaping (String) -> Void, onExit: @escaping () -> Void) {
        self.onError = onError
        self.onExit = onExit
        _cheats = StateObject(wrappedValue: CheatModel(gameId: gameId))
    }

    @State private var frame: CGImage?
    @State private var pixelBuffer = [UInt8](repeating: 0, count: WipiCore.frameByteCount)
    @State private var haptics = Haptics()
    @State private var controllerInput = ControllerInput()
    @State private var showSettings = false
    @State private var paused = false

    @AppStorage(SettingsKey.vibrationEnabled) private var vibrationEnabled = true
    @AppStorage(SettingsKey.vibrationScale) private var vibrationScale = 1.0
    @AppStorage(SettingsKey.soundEnabled) private var soundEnabled = true
    @AppStorage(SettingsKey.pcmVolume) private var pcmVolume = 1.0
    @AppStorage(SettingsKey.midiVolume) private var midiVolume = 1.0
    @AppStorage(SettingsKey.showPerformance) private var showPerformance = false
    @AppStorage(SettingsKey.gameSpeed) private var gameSpeed = 100

    // 성능 표시: 화면에 새로 나간 프레임을 세어 1초마다 FPS로 표시
    @State private var performanceText = ""
    @State private var performanceSampledAt = Date()
    @State private var presentedFrames = 0

    @Environment(\.scenePhase) private var scenePhase

    private let timer = Timer.publish(every: 1.0 / 60.0, on: .main, in: .common).autoconnect()

    var body: some View {
        VStack(spacing: 0) {
            GeometryReader { _ in
                screen
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }

            Keypad(haptics: haptics)
                .padding(8)
        }
        .background(Color(red: 0.125, green: 0.125, blue: 0.125))
        .overlay(alignment: .topTrailing) {
            HStack(spacing: 0) {
                Button {
                    pause()
                } label: {
                    Image(systemName: "pause.fill")
                        .font(.title3)
                        .foregroundStyle(.white.opacity(0.6))
                        .padding(10)
                }
                Button {
                    showSettings = true
                } label: {
                    Image(systemName: "gearshape.fill")
                        .font(.title3)
                        .foregroundStyle(.white.opacity(0.6))
                        .padding(10)
                }
            }
        }
        .overlay(alignment: .topLeading) {
            if showPerformance && !performanceText.isEmpty {
                Text(performanceText)
                    .font(.system(size: 12, weight: .medium, design: .monospaced))
                    .foregroundStyle(.white)
                    .padding(.horizontal, 6)
                    .padding(.vertical, 3)
                    .background(Color.black.opacity(0.55), in: RoundedRectangle(cornerRadius: 4))
                    .padding(10)
                    .allowsHitTesting(false)
            }
        }
        .overlay {
            if paused {
                pauseOverlay
            }
        }
        .onReceive(timer) { _ in
            pollFrame()
        }
        .sheet(isPresented: $showSettings) {
            SettingsView()
        }
        .sheet(isPresented: $showCheats) {
            CheatView(model: cheats)
        }
        .onAppear {
            UIApplication.shared.isIdleTimerDisabled = true // 게임 중 화면 자동잠금 방지
            AudioSession.activate()
            applyVolume()
            WipiCore.setSpeed(percent: gameSpeed)
            controllerInput.start()
        }
        .onChange(of: gameSpeed) { WipiCore.setSpeed(percent: $0) }
        .onChange(of: soundEnabled) { _ in applyVolume() }
        .onChange(of: pcmVolume) { _ in applyVolume() }
        .onChange(of: midiVolume) { _ in applyVolume() }
        .onDisappear {
            UIApplication.shared.isIdleTimerDisabled = false
            AudioSession.deactivate()
            controllerInput.stop()
        }
        .onChange(of: scenePhase) { phase in
            // 백그라운드로 나가면 일시정지. 복귀해도 자동 재개하지 않고
            // 오버레이를 띄워 사용자가 "계속하기"를 눌러야 재개된다 (모바일 에뮬 표준 UX).
            switch phase {
            case .active:
                UIApplication.shared.isIdleTimerDisabled = true
                AudioSession.activate()
            case .background, .inactive:
                pause()
                AudioSession.deactivate()
            @unknown default:
                break
            }
        }
    }

    /// 일시정지 메뉴 — 탭하면 재개, 종료 버튼으로 라이브러리 복귀 (Delta 등 에뮬 표준 패턴)
    private var pauseOverlay: some View {
        ZStack {
            Color.black.opacity(0.6)
            VStack(spacing: 12) {
                Image(systemName: "pause.circle.fill")
                    .font(.system(size: 56))
                Text("emulator_paused")
                    .font(.headline)
                Text("emulator_resume_hint")
                    .font(.subheadline)
                    .opacity(0.7)

                HStack(spacing: 12) {
                    Button {
                        showCheats = true
                    } label: {
                        Label("cheat_title", systemImage: "wand.and.stars")
                            .padding(.horizontal, 20)
                            .padding(.vertical, 10)
                            .background(Color.white.opacity(0.15), in: Capsule())
                    }
                    Button {
                        onExit()
                    } label: {
                        Label("emulator_exit", systemImage: "rectangle.portrait.and.arrow.right")
                            .padding(.horizontal, 20)
                            .padding(.vertical, 10)
                            .background(Color.white.opacity(0.15), in: Capsule())
                    }
                }
                .padding(.top, 24)
            }
            .foregroundStyle(.white)
        }
        .contentShape(Rectangle())
        .onTapGesture { resume() }
    }

    private func pause() {
        guard !paused else { return }
        paused = true
        WipiCore.setPaused(true)
    }

    private func resume() {
        paused = false
        WipiCore.setPaused(false)
    }

    private func applyVolume() {
        WipiCore.setVolume(
            pcm: soundEnabled ? Float(pcmVolume) : 0,
            midi: soundEnabled ? Float(midiVolume) : 0
        )
    }

    @ViewBuilder
    private var screen: some View {
        if let frame {
            Image(decorative: frame, scale: 1)
                .resizable()
                .interpolation(.none)
                .aspectRatio(
                    CGFloat(WipiCore.screenWidth) / CGFloat(WipiCore.screenHeight),
                    contentMode: .fit
                )
        } else {
            Text("emulator_loading")
                .foregroundColor(.white)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }

    private func pollFrame() {
        if WipiCore.getFrame(into: &pixelBuffer) {
            frame = Self.makeImage(from: pixelBuffer)
            presentedFrames += 1
        }
        if showPerformance {
            samplePerformance()
        }
        // enabled가 아니어도 요청은 소비해야 큐가 쌓이지 않는다
        if let vibration = WipiCore.pendingVibration(), vibrationEnabled {
            haptics.play(durationMs: vibration.durationMs, intensity: vibration.intensity, scale: Float(vibrationScale))
        }
        if let message = WipiCore.pendingError() {
            onError(message)
        }
        // 게임이 종료를 요청하면 라이브러리로 복귀
        if WipiCore.pendingExit() {
            onExit()
        }
    }

    /// 1초마다 FPS(화면에 새로 나간 프레임 수)와 에뮬레이터 부하를 갱신
    private func samplePerformance() {
        let now = Date()
        let elapsed = now.timeIntervalSince(performanceSampledAt)
        guard elapsed >= 1 else { return }

        performanceText = "\(Int((Double(presentedFrames) / elapsed).rounded())) FPS  CPU \(WipiCore.cpuLoad())%"
        presentedFrames = 0
        performanceSampledAt = now
    }

    /// RGBA8888 바이트 → CGImage (알파 무시)
    private static func makeImage(from pixels: [UInt8]) -> CGImage? {
        let data = Data(pixels)
        guard let provider = CGDataProvider(data: data as CFData) else { return nil }

        return CGImage(
            width: WipiCore.screenWidth,
            height: WipiCore.screenHeight,
            bitsPerComponent: 8,
            bitsPerPixel: 32,
            bytesPerRow: WipiCore.screenWidth * 4,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.noneSkipLast.rawValue),
            provider: provider,
            decode: nil,
            shouldInterpolate: false,
            intent: .defaultIntent
        )
    }
}
