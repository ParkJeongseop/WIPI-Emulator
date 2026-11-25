// 에뮬레이터 설정. 진동은 코어가 넘긴 게임 원본 세기 위에 배율만 얹는다
// (RetroArch/PPSSPP 등의 rumble gain 방식). 설정값은 UserDefaults(@AppStorage)에 저장.

import SwiftUI

/// @AppStorage 키 상수 (뷰 간 공유)
enum SettingsKey {
    static let vibrationEnabled = "vibrationEnabled"
    static let vibrationScale = "vibrationScale"
    static let keypadHaptics = "keypadHaptics"
    static let soundEnabled = "soundEnabled"
    static let pcmVolume = "pcmVolume"
    static let midiVolume = "midiVolume"
    static let showPerformance = "showPerformance"
    static let keypadLayout = "keypadLayout"
    static let keypadSize = "keypadSize"
    static let gameSpeed = "gameSpeed"
}

/// 고를 수 있는 게임 속도 (퍼센트)
let gameSpeeds = [50, 100, 150, 200, 300]

/// 100 → "1×", 150 → "1.5×"
func speedLabel(_ percent: Int) -> String {
    percent % 100 == 0 ? "\(percent / 100)×" : "\(Double(percent) / 100)×"
}

struct SettingsView: View {
    @AppStorage(SettingsKey.vibrationEnabled) private var vibrationEnabled = true
    @AppStorage(SettingsKey.vibrationScale) private var vibrationScale = 1.0
    @AppStorage(SettingsKey.keypadHaptics) private var keypadHaptics = true
    @AppStorage(SettingsKey.soundEnabled) private var soundEnabled = true
    @AppStorage(SettingsKey.pcmVolume) private var pcmVolume = 1.0
    @AppStorage(SettingsKey.midiVolume) private var midiVolume = 1.0
    @AppStorage(SettingsKey.showPerformance) private var showPerformance = false
    @AppStorage(SettingsKey.keypadLayout) private var keypadLayout = KeypadLayout.phone
    @AppStorage(SettingsKey.keypadSize) private var keypadSize = KeypadSize.normal
    @AppStorage(SettingsKey.gameSpeed) private var gameSpeed = 100

    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    Toggle("settings_sound", isOn: $soundEnabled)

                    if soundEnabled {
                        VolumeRow(label: "settings_music", value: $midiVolume)
                        VolumeRow(label: "settings_effects", value: $pcmVolume)
                    }
                } header: {
                    Text("settings_sound_header")
                } footer: {
                    Text("settings_sound_footer")
                }

                Section {
                    Toggle("settings_vibration", isOn: $vibrationEnabled)

                    if vibrationEnabled {
                        VStack(alignment: .leading, spacing: 4) {
                            HStack {
                                Text("settings_vibration_strength")
                                Spacer()
                                Text("\(Int((vibrationScale * 100).rounded()))%")
                                    .foregroundStyle(.secondary)
                                    .monospacedDigit()
                            }
                            HStack {
                                Text("settings_weak").foregroundStyle(.secondary)
                                Slider(value: $vibrationScale, in: 0...1.5, step: 0.05)
                                Text("settings_strong").foregroundStyle(.secondary)
                            }
                        }
                    }
                    Toggle("settings_keypad_haptics", isOn: $keypadHaptics)
                } header: {
                    Text("settings_vibration_header")
                } footer: {
                    Text("settings_vibration_footer")
                }

                Section {
                    Picker("settings_keypad_layout", selection: $keypadLayout) {
                        ForEach(KeypadLayout.allCases) { Text($0.label).tag($0) }
                    }
                    Picker("settings_keypad_size", selection: $keypadSize) {
                        ForEach(KeypadSize.allCases) { Text($0.label).tag($0) }
                    }
                } header: {
                    Text("settings_keypad_header")
                } footer: {
                    Text("settings_keypad_layout_footer")
                }

                Section {
                    Picker("settings_game_speed", selection: $gameSpeed) {
                        ForEach(gameSpeeds, id: \.self) { Text(verbatim: speedLabel($0)).tag($0) }
                    }
                } header: {
                    Text("settings_performance_header")
                } footer: {
                    Text("settings_game_speed_footer")
                }

                Section {
                    Toggle("settings_show_performance", isOn: $showPerformance)
                } footer: {
                    Text("settings_show_performance_footer")
                }

                Section {
                    NavigationLink("settings_licenses") {
                        LicensesView()
                    }
                    HStack {
                        Text("settings_version")
                        Spacer()
                        Text(Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "-")
                            .foregroundStyle(.secondary)
                    }
                } header: {
                    Text("settings_info_header")
                }
            }
            .navigationTitle("settings_title")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("action_done") { dismiss() }
                }
            }
        }
    }
}


private struct VolumeRow: View {
    let label: LocalizedStringKey
    @Binding var value: Double

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(label)
                Spacer()
                Text("\(Int((value * 100).rounded()))%")
                    .foregroundStyle(.secondary)
                    .monospacedDigit()
            }
            HStack {
                Image(systemName: "speaker.fill").foregroundStyle(.secondary)
                Slider(value: $value, in: 0...1, step: 0.05)
                Image(systemName: "speaker.wave.3.fill").foregroundStyle(.secondary)
            }
        }
    }
}
