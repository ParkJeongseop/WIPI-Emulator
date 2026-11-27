// 라이브러리 화면 ↔ 에뮬레이터 화면 전환.

import SwiftUI

struct ContentView: View {
    @StateObject private var library = GameLibrary()
    @State private var running = false
    @State private var runningGameId = ""
    @State private var errorMessage: String?

    var body: some View {
        Group {
            if running {
                EmulatorScreenView(
                    gameId: runningGameId,
                    onError: { message in
                        errorMessage = message
                        running = false
                        WipiCore.stop()
                    },
                    onExit: {
                        running = false
                        WipiCore.stop()
                    }
                )
            } else {
                LibraryView(
                    library: library,
                    onPlay: play,
                    runError: errorMessage,
                    onDismissError: { errorMessage = nil }
                )
            }
        }
    }

    private func play(_ entry: GameEntry) {
        try? FileManager.default.createDirectory(at: entry.dataDir, withIntermediateDirectories: true)
        runningGameId = entry.id
        start(gameData: entry.gameData, filename: entry.filename, dataDir: entry.dataDir, screenMode: entry.screenMode)
    }

    private func start(gameData: Data, filename: String, dataDir: URL, screenMode: Int) {
        let soundfontPath = Bundle.main.path(forResource: "GeneralUser-GS", ofType: "sf2") ?? ""

        let ok = WipiCore.start(
            gameData: gameData,
            filename: filename,
            dataDir: dataDir.path,
            soundfontPath: soundfontPath,
            screenMode: screenMode
        )

        if ok {
            errorMessage = nil
            running = true
        } else {
            // 동기 실패는 형식 문제가 아니라 시작 자체 실패 (형식 오류는 로드 후 kind로 안내됨)
            errorMessage = WipiCore.pendingError() ?? String(localized: "error_start_failed")
        }
    }
}
