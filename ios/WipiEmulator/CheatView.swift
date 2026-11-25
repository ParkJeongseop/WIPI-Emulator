// 메모리 치트: 값 검색 → 좁히기 → 주소 저장 → 값 쓰기/고정. Android Cheats.kt와 대칭.
// 검색 상태는 코어(wipi_core::cheat)가 세션 동안 들고 있고, 여기는 화면 상태와 게임별 저장 목록을 맡는다.

import SwiftUI

/// 검색 후보 하나
struct CheatResult: Identifiable {
    let address: UInt32
    let value: UInt32

    var id: UInt32 { address }
}

/// 저장된 치트. 주소가 곧 식별자다 (코어의 고정 목록도 주소로 구분).
struct SavedCheat: Codable, Identifiable {
    var name: String
    let address: UInt32
    let width: Int
    var value: UInt32
    /// 고정 여부는 저장하지 않는다 — 주소는 실행마다 달라질 수 있어 다음 실행에선 꺼진 채로 불러온다
    var frozen = false

    var id: UInt32 { address }

    private enum CodingKeys: String, CodingKey {
        case name, address, width, value
    }
}

/// 4바이트 값은 부호 있는 수로, 1·2바이트는 부호 없는 수로 보여 준다
func cheatValueText(_ value: UInt32, width: Int) -> String {
    switch width {
    case 1: String(value & 0xFF)
    case 2: String(value & 0xFFFF)
    default: String(Int32(bitPattern: value))
    }
}

/// 음수도 받는다 (폭에 맞춰 잘림). 숫자가 아니거나 32비트를 넘으면 nil.
func parseCheatValue(_ text: String) -> UInt32? {
    guard let number = Int64(text.trimmingCharacters(in: .whitespaces)),
          (Int64(Int32.min)...Int64(UInt32.max)).contains(number)
    else { return nil }
    return UInt32(truncatingIfNeeded: number)
}

private func widthLabel(_ width: Int) -> String {
    String(format: NSLocalizedString("cheat_width_bytes", comment: ""), width)
}

/// 게임 한 판(세션) 동안의 치트 상태. 화면을 닫았다 열어도 검색 진행이 유지된다.
final class CheatModel: ObservableObject {
    /// 화면에 보여 줄 검색 후보 수
    static let resultLimit = 50
    /// 코어가 보관하는 후보 수 상한 (wipi_core::cheat::MAX_CANDIDATES)
    static let maxCandidates = 1_000_000

    /// nil = 확인 중
    @Published private(set) var supported: Bool?
    @Published private(set) var busy = false
    /// 직전 검색의 일치 수 (nil = 아직 검색 안 함)
    @Published private(set) var matchCount: Int?
    @Published private(set) var searchWidth = 4
    @Published private(set) var results: [CheatResult] = []
    @Published private(set) var saved: [SavedCheat] = []
    /// 저장된 치트 주소의 현재 값 (읽을 수 없는 주소는 없음)
    @Published private(set) var current: [UInt32: UInt32] = [:]

    private let storageKey: String

    // 코어 호출은 에뮬레이터 스레드의 응답을 기다리므로 백그라운드에서, 요청 순서대로 하나씩
    private let queue = DispatchQueue(label: "com.parkjeongseop.wipi.cheat")

    init(gameId: String) {
        storageKey = Self.storageKey(gameId: gameId)
        if let data = UserDefaults.standard.data(forKey: storageKey),
           let cheats = try? JSONDecoder().decode([SavedCheat].self, from: data) {
            saved = cheats
        }
        core({ WipiCore.cheatSupported() }) { self.supported = $0 }
    }

    static func storageKey(gameId: String) -> String {
        "cheats.\(gameId)"
    }

    private func core<T>(_ work: @escaping () -> T, then: @escaping (T) -> Void = { _ in }) {
        queue.async {
            let result = work()
            DispatchQueue.main.async { then(result) }
        }
    }

    func search(value: UInt32, width: Int, refine: Bool) {
        busy = true
        core({ (WipiCore.cheatSearch(value: value, width: width, refine: refine), WipiCore.cheatResults(limit: Self.resultLimit)) }) { count, found in
            self.busy = false
            guard let count else { return }
            self.matchCount = count
            if !refine { self.searchWidth = width }
            self.results = found
        }
    }

    /// 후보와 저장된 치트의 현재 값을 다시 읽는다 (화면을 열 때 — 그 사이 게임이 값을 바꿨을 수 있다)
    func refresh() {
        let saved = saved
        let hasSearch = matchCount != nil
        core({
            let found = hasSearch ? WipiCore.cheatResults(limit: Self.resultLimit) : []
            var current: [UInt32: UInt32] = [:]
            for cheat in saved {
                current[cheat.address] = WipiCore.cheatRead(address: cheat.address, width: cheat.width)
            }
            return (found, current)
        }) { found, current in
            if hasSearch { self.results = found }
            self.current = current
        }
    }

    func isSaved(_ address: UInt32) -> Bool {
        saved.contains { $0.address == address }
    }

    func add(_ result: CheatResult) {
        guard !isSaved(result.address) else { return }
        saved.append(SavedCheat(name: "", address: result.address, width: searchWidth, value: result.value))
        current[result.address] = result.value
        persist()
    }

    @discardableResult
    private func update(_ address: UInt32, _ change: (inout SavedCheat) -> Void) -> SavedCheat? {
        guard let index = saved.firstIndex(where: { $0.address == address }) else { return nil }
        change(&saved[index])
        persist()
        return saved[index]
    }

    func rename(_ cheat: SavedCheat, name: String) {
        update(cheat.address) { $0.name = name }
    }

    /// 값을 바꾼다. 고정 중이면 새 값으로 바로 고정한다.
    func setValue(_ cheat: SavedCheat, value: UInt32) {
        guard let updated = update(cheat.address, { $0.value = value }), updated.frozen else { return }
        core({ WipiCore.cheatFreeze(address: updated.address, width: updated.width, value: updated.value) })
    }

    /// 값을 한 번 쓴다
    func write(_ cheat: SavedCheat) {
        guard let cheat = saved.first(where: { $0.address == cheat.address }) else { return }
        core({ WipiCore.cheatWrite(address: cheat.address, width: cheat.width, value: cheat.value) }) { ok in
            if ok { self.current[cheat.address] = cheat.value }
        }
    }

    func setFrozen(_ cheat: SavedCheat, _ frozen: Bool) {
        guard let updated = update(cheat.address, { $0.frozen = frozen }) else { return }
        if frozen {
            core({ WipiCore.cheatFreeze(address: updated.address, width: updated.width, value: updated.value) }) { ok in
                if ok {
                    self.current[updated.address] = updated.value
                } else {
                    // 이번 실행에선 쓸 수 없는 주소 (게임을 다시 시작해 메모리 배치가 달라진 경우)
                    self.update(updated.address) { $0.frozen = false }
                }
            }
        } else {
            core({ WipiCore.cheatUnfreeze(address: updated.address) })
        }
    }

    func remove(at offsets: IndexSet) {
        let removed = offsets.map { saved[$0].address }
        saved.remove(atOffsets: offsets)
        persist()
        core({ removed.forEach(WipiCore.cheatUnfreeze) })
    }

    private func persist() {
        UserDefaults.standard.set(try? JSONEncoder().encode(saved), forKey: storageKey)
    }
}

struct CheatView: View {
    @ObservedObject var model: CheatModel

    @State private var valueText = ""
    @State private var width = 4

    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            Group {
                switch model.supported {
                case nil:
                    ProgressView()
                case false?:
                    Text("cheat_unsupported")
                        .foregroundStyle(.secondary)
                        .padding()
                        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
                case true?:
                    form
                }
            }
            .navigationTitle("cheat_title")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("action_done") { dismiss() }
                }
            }
        }
        .onAppear { model.refresh() }
    }

    private var form: some View {
        Form {
            Section {
                TextField("cheat_value", text: $valueText)
                    .keyboardType(.numbersAndPunctuation)
                Picker("cheat_value", selection: $width) {
                    ForEach([4, 2, 1], id: \.self) { Text(verbatim: widthLabel($0)).tag($0) }
                }
                .pickerStyle(.segmented)
                HStack {
                    Button("cheat_new_search") { search(refine: false) }
                        .buttonStyle(.borderedProminent)
                        .disabled(parseCheatValue(valueText) == nil || model.busy)
                    Button("cheat_refine") { search(refine: true) }
                        .buttonStyle(.bordered)
                        .disabled(parseCheatValue(valueText) == nil || model.busy || (model.matchCount ?? 0) == 0)
                }
                status
            } header: {
                Text("cheat_search_header")
            } footer: {
                Text("cheat_howto")
            }

            if !model.results.isEmpty {
                Section {
                    ForEach(model.results) { result in
                        HStack {
                            Text(verbatim: String(format: "0x%08X", result.address))
                            Spacer()
                            Text(verbatim: cheatValueText(result.value, width: model.searchWidth))
                            Button {
                                model.add(result)
                            } label: {
                                Image(systemName: model.isSaved(result.address) ? "checkmark" : "plus.circle")
                                    .font(.title3)
                                    .frame(width: 44, height: 32)
                                    .contentShape(Rectangle())
                            }
                            .buttonStyle(.borderless)
                            .disabled(model.isSaved(result.address))
                            .accessibilityLabel(Text("cheat_add"))
                        }
                        .font(.body.monospaced())
                    }
                }
            }

            Section {
                ForEach(model.saved) { cheat in
                    SavedCheatRow(model: model, cheat: cheat)
                }
                .onDelete { model.remove(at: $0) }
            } header: {
                Text("cheat_saved_header")
            } footer: {
                Text(LocalizedStringKey(model.saved.isEmpty ? "cheat_saved_empty" : "cheat_saved_footer"))
            }
        }
    }

    @ViewBuilder
    private var status: some View {
        if model.busy {
            Text("cheat_searching").foregroundStyle(.secondary)
        } else if let count = model.matchCount {
            if count == 0 {
                Text("cheat_no_match").font(.footnote).foregroundStyle(.secondary)
            } else {
                VStack(alignment: .leading, spacing: 2) {
                    Text(verbatim: String(format: NSLocalizedString("cheat_match_count", comment: ""), count))
                        .font(.headline)
                    if count > CheatModel.maxCandidates {
                        Text("cheat_too_many").font(.footnote).foregroundStyle(.secondary)
                    } else if count > model.results.count {
                        Text(verbatim: String(format: NSLocalizedString("cheat_showing_first", comment: ""), model.results.count))
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                    }
                }
            }
        }
    }

    private func search(refine: Bool) {
        guard let value = parseCheatValue(valueText) else { return }
        model.search(value: value, width: width, refine: refine)
    }
}

private struct SavedCheatRow: View {
    @ObservedObject var model: CheatModel
    let cheat: SavedCheat

    // 입력 중인 글자는 여기서 들고, 숫자로 읽힐 때만 모델에 반영한다
    @State private var valueText: String

    init(model: CheatModel, cheat: SavedCheat) {
        self.model = model
        self.cheat = cheat
        _valueText = State(initialValue: cheatValueText(cheat.value, width: cheat.width))
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            TextField("cheat_name", text: Binding(get: { cheat.name }, set: { model.rename(cheat, name: $0) }))
            HStack {
                TextField("cheat_value", text: $valueText)
                    .keyboardType(.numbersAndPunctuation)
                    .textFieldStyle(.roundedBorder)
                    .onChange(of: valueText) { text in
                        if let value = parseCheatValue(text) { model.setValue(cheat, value: value) }
                    }
                Button("cheat_write") { model.write(cheat) }
                    .buttonStyle(.bordered)
                Toggle("cheat_freeze", isOn: Binding(get: { cheat.frozen }, set: { model.setFrozen(cheat, $0) }))
                    .fixedSize()
            }
            Text(verbatim: detail)
                .font(.caption.monospaced())
                .foregroundStyle(.secondary)
        }
        .padding(.vertical, 4)
    }

    /// "0x40001234 · 4바이트 · 현재 1200"
    private var detail: String {
        var parts = [String(format: "0x%08X", cheat.address), widthLabel(cheat.width)]
        if let value = model.current[cheat.address] {
            parts.append(String(format: NSLocalizedString("cheat_current", comment: ""), cheatValueText(value, width: cheat.width)))
        }
        return parts.joined(separator: " · ")
    }
}
