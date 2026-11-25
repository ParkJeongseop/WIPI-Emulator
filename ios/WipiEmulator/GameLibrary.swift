// 게임 라이브러리 — 임포트한 게임을 Documents/games/<UUID>/에 영구 저장하고 관리.
// 표지·게임명은 콘솔 에뮬과 달리 패키지(zip) 안의 big.icon/__adf__에서 뽑아 캐시한다.

import Foundation
import UIKit

struct GameEntry: Identifiable {
    let id: String // 저장 폴더명(UUID)
    let name: String
    let cover: UIImage?
    let gameData: Data
    let filename: String // 포맷 감지용 원본 파일명(.zip/.jar 확장자 유지)
    let dataDir: URL // 게임별 세이브 경로 (games/<id>/data) — 삭제 시 함께 제거됨
}

@MainActor
final class GameLibrary: ObservableObject {
    @Published private(set) var games: [GameEntry] = []

    private let root: URL

    init() {
        let docs = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
        root = docs.appendingPathComponent("games", isDirectory: true)
        try? FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        reload()
    }


    /// 저장된 게임들을 스캔해 목록을 갱신 (이름순)
    func reload() {
        let dirs = (try? FileManager.default.contentsOfDirectory(at: root, includingPropertiesForKeys: nil)) ?? []
        games = dirs.compactMap(load).sorted { $0.name.localizedCompare($1.name) == .orderedAscending }
    }

    private func load(_ dir: URL) -> GameEntry? {
        guard dir.hasDirectoryPath,
              let meta = try? JSONDecoder().decode(Meta.self, from: Data(contentsOf: dir.appendingPathComponent("meta.json"))),
              let gameData = try? Data(contentsOf: dir.appendingPathComponent(meta.filename))
        else { return nil }

        let cover = (try? Data(contentsOf: dir.appendingPathComponent("cover.png"))).flatMap(UIImage.init(data:))
        let dataDir = dir.appendingPathComponent("data", isDirectory: true)
        return GameEntry(id: dir.lastPathComponent, name: meta.name, cover: cover, gameData: gameData, filename: meta.filename, dataDir: dataDir)
    }

    /// 파일을 라이브러리에 임포트 (복사 + 표지/이름 추출·캐시). 성공 시 GameEntry 반환.
    @discardableResult
    func importGame(from url: URL) throws -> GameEntry {
        let accessing = url.startAccessingSecurityScopedResource()
        defer { if accessing { url.stopAccessingSecurityScopedResource() } }

        let gameData = try Data(contentsOf: url)
        let filename = url.lastPathComponent

        let id = UUID().uuidString
        let dir = root.appendingPathComponent(id, isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)

        try gameData.write(to: dir.appendingPathComponent(filename))

        var cover: UIImage?
        if let iconData = WipiCore.extractIcon(gameData: gameData) {
            try? iconData.write(to: dir.appendingPathComponent("cover.png"))
            cover = UIImage(data: iconData)
        }

        // 게임명: __adf__(EUC-KR) → 없으면 파일명(확장자 제거)
        let name = WipiCore.extractNameRaw(gameData: gameData).flatMap(Self.decodeEUCKR)
            ?? (filename as NSString).deletingPathExtension

        let meta = Meta(name: name, filename: filename)
        try JSONEncoder().encode(meta).write(to: dir.appendingPathComponent("meta.json"))

        let dataDir = dir.appendingPathComponent("data", isDirectory: true)
        let entry = GameEntry(id: id, name: name, cover: cover, gameData: gameData, filename: filename, dataDir: dataDir)
        reload()
        return entry
    }

    func delete(_ entry: GameEntry) {
        try? FileManager.default.removeItem(at: root.appendingPathComponent(entry.id, isDirectory: true))
        UserDefaults.standard.removeObject(forKey: CheatModel.storageKey(gameId: entry.id)) // 저장된 치트도 함께
        reload()
    }

    private struct Meta: Codable {
        let name: String
        let filename: String
    }

    /// EUC-KR(CP949) 바이트 → String. WIPI 패키지의 게임명 인코딩.
    static func decodeEUCKR(_ data: Data) -> String? {
        let cp949 = CFStringConvertEncodingToNSStringEncoding(CFStringEncoding(CFStringEncodings.dosKorean.rawValue))
        let euckr = CFStringConvertEncodingToNSStringEncoding(CFStringEncoding(CFStringEncodings.EUC_KR.rawValue))
        return String(data: data, encoding: .init(rawValue: cp949))
            ?? String(data: data, encoding: .init(rawValue: euckr))
    }
}
