// 터치 키패드 — 배치 3종(휴대폰 / 게임패드 / 게임패드 좌우 반전)과 키 크기를 설정에서 고른다.
// Android MainActivity의 Keypad/KeyGrid/KeyButton과 같은 구성.
// 키 반복은 Rust 세션이 처리하므로 여기서는 down/up만 보낸다.

import SwiftUI

/// 화면 키패드 배치 (rawValue가 저장 형식)
enum KeypadLayout: String, CaseIterable, Identifiable {
    /// 피처폰 그대로: 방향키·소프트키 3줄 + 숫자 4줄
    case phone
    /// 왼쪽 방향키 + 오른쪽 숫자키 — 줄 수가 줄어 게임 화면이 커진다
    case gamepad
    /// gamepad의 좌우 반전 (방향키 오른쪽)
    case gamepadMirrored

    var id: String { rawValue }

    var label: LocalizedStringKey {
        switch self {
        case .phone: "keypad_layout_phone"
        case .gamepad: "keypad_layout_gamepad"
        case .gamepadMirrored: "keypad_layout_gamepad_mirrored"
        }
    }
}

enum KeypadSize: String, CaseIterable, Identifiable {
    case small, normal, large

    var id: String { rawValue }

    var label: LocalizedStringKey {
        switch self {
        case .small: "keypad_size_small"
        case .normal: "keypad_size_normal"
        case .large: "keypad_size_large"
        }
    }

    var keyHeight: CGFloat {
        switch self {
        case .small: 33
        case .normal: 40
        case .large: 50
        }
    }
}

private typealias Key = (key: String, label: String)

private let directionRows: [[Key?]] = [
    [("SOFT_L", "◁"), ("UP", "▲"), ("SOFT_R", "▷")],
    [("LEFT", "◀"), ("OK", "OK"), ("RIGHT", "▶")],
    [("CALL", "📞"), ("DOWN", "▼"), ("CLR", "CLR")],
]

private let numberRows: [[Key?]] = [
    [("1", "1"), ("2", "2"), ("3", "3")],
    [("4", "4"), ("5", "5"), ("6", "6")],
    [("7", "7"), ("8", "8"), ("9", "9")],
    [("*", "*"), ("0", "0"), ("#", "#")],
]

// 게임패드 배치: 기능 키 한 줄 + (방향키 | 숫자키). nil은 빈 칸.
private let gamepadTopRow: [[Key?]] = [[("SOFT_L", "◁"), ("CALL", "📞"), ("CLR", "CLR"), ("SOFT_R", "▷")]]

private let dpadRows: [[Key?]] = [
    [nil, ("UP", "▲"), nil],
    [("LEFT", "◀"), ("OK", "OK"), ("RIGHT", "▶")],
    [nil, ("DOWN", "▼"), nil],
]

private let keySpacing: CGFloat = 4

struct Keypad: View {
    let haptics: Haptics

    @AppStorage(SettingsKey.keypadLayout) private var layout = KeypadLayout.phone
    @AppStorage(SettingsKey.keypadSize) private var size = KeypadSize.normal

    var body: some View {
        VStack(spacing: keySpacing) {
            if layout == .phone {
                KeyGrid(rows: directionRows + numberRows, haptics: haptics)
                    .frame(height: rowsHeight(7))
            } else {
                KeyGrid(rows: gamepadTopRow, haptics: haptics)
                    .frame(height: size.keyHeight)
                // 방향키 3줄을 숫자키 4줄 높이에 맞춘다 — 그만큼 방향키가 커진다
                HStack(spacing: 12) {
                    KeyGrid(rows: layout == .gamepadMirrored ? numberRows : dpadRows, haptics: haptics)
                    KeyGrid(rows: layout == .gamepadMirrored ? dpadRows : numberRows, haptics: haptics)
                }
                .frame(height: rowsHeight(4))
            }
        }
    }

    private func rowsHeight(_ rows: Int) -> CGFloat {
        size.keyHeight * CGFloat(rows) + keySpacing * CGFloat(rows - 1)
    }
}

/// 키를 격자로 채운다 — 줄과 칸이 주어진 크기를 똑같이 나눠 갖는다
private struct KeyGrid: View {
    let rows: [[Key?]]
    let haptics: Haptics

    var body: some View {
        VStack(spacing: keySpacing) {
            ForEach(rows.indices, id: \.self) { rowIndex in
                HStack(spacing: keySpacing) {
                    ForEach(rows[rowIndex].indices, id: \.self) { column in
                        if let entry = rows[rowIndex][column] {
                            KeyButton(key: entry.key, label: entry.label, haptics: haptics)
                        } else {
                            Color.clear
                        }
                    }
                }
            }
        }
    }
}

struct KeyButton: View {
    let key: String
    let label: String
    let haptics: Haptics

    @State private var pressed = false
    @AppStorage(SettingsKey.keypadHaptics) private var keypadHaptics = true

    var body: some View {
        Text(label)
            .font(.system(size: 16))
            .foregroundColor(.white)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(Color(red: 0.25, green: 0.25, blue: 0.25).opacity(pressed ? 0.6 : 1.0))
            .cornerRadius(8)
            .gesture(
                DragGesture(minimumDistance: 0)
                    .onChanged { _ in
                        if !pressed {
                            pressed = true
                            if keypadHaptics { haptics.tap() }
                            WipiCore.keyDown(key)
                        }
                    }
                    .onEnded { _ in
                        pressed = false
                        WipiCore.keyUp(key)
                    }
            )
    }
}
