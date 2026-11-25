// 설정 (SharedPreferences) + 설정/오픈소스 고지 플로우.
// 화면은 Material 3 표준 설정 패턴: Scaffold + TopAppBar(뒤로가기) + ListItem 행 +
// 컬러 카테고리 서브헤더. 변경은 즉시 적용되므로 "완료" 버튼 없음 (Android 관례).
// 기능·항목 구성(사운드/진동/정보, 고지 목록→상세)은 iOS와 동일.

package com.parkjeongseop.wipi

import android.content.Context
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.automirrored.filled.VolumeUp
import androidx.compose.material.icons.filled.Vibration
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.edit

class Settings(context: Context) {
    private val prefs = context.getSharedPreferences("settings", Context.MODE_PRIVATE)

    var soundEnabled: Boolean
        get() = prefs.getBoolean("soundEnabled", true)
        set(value) = prefs.edit { putBoolean("soundEnabled", value) }

    var pcmVolume: Float
        get() = prefs.getFloat("pcmVolume", 1.0f)
        set(value) = prefs.edit { putFloat("pcmVolume", value) }

    var midiVolume: Float
        get() = prefs.getFloat("midiVolume", 1.0f)
        set(value) = prefs.edit { putFloat("midiVolume", value) }

    var vibrationEnabled: Boolean
        get() = prefs.getBoolean("vibrationEnabled", true)
        set(value) = prefs.edit { putBoolean("vibrationEnabled", value) }

    var vibrationScale: Float
        get() = prefs.getFloat("vibrationScale", 1.0f)
        set(value) = prefs.edit { putFloat("vibrationScale", value) }

    var keypadHaptics: Boolean
        get() = prefs.getBoolean("keypadHaptics", true)
        set(value) = prefs.edit { putBoolean("keypadHaptics", value) }

    var showPerformance: Boolean
        get() = prefs.getBoolean("showPerformance", false)
        set(value) = prefs.edit { putBoolean("showPerformance", value) }

    /** 현재 설정 기준 유효 볼륨 (pcm, midi — 사운드 off면 0) */
    fun effectiveVolumes(): Pair<Float, Float> =
        if (soundEnabled) pcmVolume to midiVolume else 0f to 0f
}

// ── 설정 플로우: 설정 ⇄ 고지 목록 ⇄ 라이선스 상세 ──

private enum class SettingsPage { Settings, Licenses }

@Composable
fun SettingsFlow(settings: Settings, onVolumeChanged: () -> Unit, onClose: () -> Unit) {
    var page by remember { mutableStateOf(SettingsPage.Settings) }
    var detail by remember { mutableStateOf<License?>(null) }

    BackHandler {
        when {
            detail != null -> detail = null
            page == SettingsPage.Licenses -> page = SettingsPage.Settings
            else -> onClose()
        }
    }

    when {
        detail != null -> LicenseDetailPage(detail!!, onBack = { detail = null })
        page == SettingsPage.Licenses -> LicensesPage(
            onSelect = { detail = it },
            onBack = { page = SettingsPage.Settings },
        )
        else -> SettingsPage(
            settings = settings,
            onVolumeChanged = onVolumeChanged,
            onShowLicenses = { page = SettingsPage.Licenses },
            onBack = onClose,
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SettingsScaffold(title: String, onBack: () -> Unit, content: @Composable (Modifier) -> Unit) {
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(title) },
                navigationIcon = {
                    IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.action_back)) }
                },
            )
        },
    ) { innerPadding ->
        content(Modifier.padding(innerPadding))
    }
}

/** Material 설정 화면의 카테고리 서브헤더 */
@Composable
private fun CategoryHeader(title: String) {
    Text(
        title,
        style = MaterialTheme.typography.labelLarge,
        color = MaterialTheme.colorScheme.primary,
        modifier = Modifier.padding(start = 16.dp, top = 20.dp, bottom = 4.dp),
    )
}

@Composable
private fun SettingsPage(settings: Settings, onVolumeChanged: () -> Unit, onShowLicenses: () -> Unit, onBack: () -> Unit) {
    var soundEnabled by remember { mutableStateOf(settings.soundEnabled) }
    var pcmVolume by remember { mutableStateOf(settings.pcmVolume) }
    var midiVolume by remember { mutableStateOf(settings.midiVolume) }
    var vibrationEnabled by remember { mutableStateOf(settings.vibrationEnabled) }
    var vibrationScale by remember { mutableStateOf(settings.vibrationScale) }
    var keypadHaptics by remember { mutableStateOf(settings.keypadHaptics) }
    var showPerformance by remember { mutableStateOf(settings.showPerformance) }

    SettingsScaffold(title = stringResource(R.string.settings_title), onBack = onBack) { modifier ->
        Column(modifier = modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
            CategoryHeader(stringResource(R.string.settings_sound_header))
            ListItem(
                headlineContent = { Text(stringResource(R.string.settings_sound)) },
                supportingContent = { Text(stringResource(R.string.settings_sound_desc)) },
                leadingContent = { Icon(Icons.AutoMirrored.Filled.VolumeUp, null) },
                trailingContent = {
                    Switch(checked = soundEnabled, onCheckedChange = {
                        soundEnabled = it
                        settings.soundEnabled = it
                        onVolumeChanged()
                    })
                },
            )
            if (soundEnabled) {
                ListItem(
                    headlineContent = { Text(stringResource(R.string.settings_music)) },
                    supportingContent = {
                        Slider(value = midiVolume, valueRange = 0f..1f, onValueChange = {
                            midiVolume = it
                            settings.midiVolume = it
                            onVolumeChanged()
                        })
                    },
                    trailingContent = { Text("${(midiVolume * 100).toInt()}%") },
                )
                ListItem(
                    headlineContent = { Text(stringResource(R.string.settings_effects)) },
                    supportingContent = {
                        Column {
                            Slider(value = pcmVolume, valueRange = 0f..1f, onValueChange = {
                                pcmVolume = it
                                settings.pcmVolume = it
                                onVolumeChanged()
                            })
                            Text(
                                stringResource(R.string.settings_sound_footer),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    },
                    trailingContent = { Text("${(pcmVolume * 100).toInt()}%") },
                )
            }

            CategoryHeader(stringResource(R.string.settings_vibration_header))
            ListItem(
                headlineContent = { Text(stringResource(R.string.settings_vibration)) },
                supportingContent = { Text(stringResource(R.string.settings_vibration_desc)) },
                leadingContent = { Icon(Icons.Filled.Vibration, null) },
                trailingContent = {
                    Switch(checked = vibrationEnabled, onCheckedChange = {
                        vibrationEnabled = it
                        settings.vibrationEnabled = it
                    })
                },
            )
            if (vibrationEnabled) {
                ListItem(
                    headlineContent = { Text(stringResource(R.string.settings_vibration_strength)) },
                    supportingContent = {
                        Column {
                            Slider(value = vibrationScale, valueRange = 0f..1.5f, onValueChange = {
                                vibrationScale = it
                                settings.vibrationScale = it
                            })
                            Text(
                                stringResource(R.string.settings_vibration_footer),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    },
                    trailingContent = { Text("${(vibrationScale * 100).toInt()}%") },
                )
            }

            ListItem(
                headlineContent = { Text(stringResource(R.string.settings_keypad_haptics)) },
                supportingContent = { Text(stringResource(R.string.settings_keypad_haptics_desc)) },
                trailingContent = {
                    Switch(checked = keypadHaptics, onCheckedChange = {
                        keypadHaptics = it
                        settings.keypadHaptics = it
                    })
                },
            )

            CategoryHeader(stringResource(R.string.settings_performance_header))
            ListItem(
                headlineContent = { Text(stringResource(R.string.settings_show_performance)) },
                supportingContent = { Text(stringResource(R.string.settings_show_performance_footer)) },
                trailingContent = {
                    Switch(checked = showPerformance, onCheckedChange = {
                        showPerformance = it
                        settings.showPerformance = it
                    })
                },
            )

            CategoryHeader(stringResource(R.string.settings_info_header))
            ListItem(
                headlineContent = { Text(stringResource(R.string.settings_licenses)) },
                trailingContent = { Icon(Icons.AutoMirrored.Filled.KeyboardArrowRight, null) },
                modifier = Modifier.clickable(onClick = onShowLicenses),
            )
            ListItem(
                headlineContent = { Text(stringResource(R.string.settings_version)) },
                supportingContent = { Text(BuildConfig.VERSION_NAME) },
            )
        }
    }
}

// ── 오픈소스 고지 (iOS와 동일 내용, 목록 → 상세) ──

private fun mit(copyright: String) = """
$copyright

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
""".trimIndent()

data class License(val name: String, val roleRes: Int, val text: String)

private val licenses = listOf(
    License("wie", R.string.license_role_wie, mit("Copyright 2020 Inseok Lee")),
    License("RustJava", R.string.license_role_rustjava, mit("Copyright 2020 Inseok Lee")),
    License("smaf", R.string.license_role_smaf, mit("Copyright 2020 Inseok Lee")),
    License(
        "rodio / cpal", R.string.license_role_rodio,
        mit("Copyright (c) The Rodio Project Contributors") +
            "\n\n---\n\ncpal is licensed under the Apache License, Version 2.0.\nhttp://www.apache.org/licenses/LICENSE-2.0"
    ),
    License("rustysynth", R.string.license_role_rustysynth, mit("Copyright (c) 2021 Nobuaki Tanaka")),
    License(
        "GeneralUser GS", R.string.license_role_soundfont,
        "GeneralUser GS by S. Christian Collins\n(schristiancollins.com/generaluser.php)\n\n" +
            "Licensed under the GeneralUser GS License v2.0: free to use, modify and distribute, " +
            "including in commercial software, with attribution appreciated. No warranty is provided."
    ),
)

@Composable
private fun LicensesPage(onSelect: (License) -> Unit, onBack: () -> Unit) {
    SettingsScaffold(title = stringResource(R.string.licenses_title), onBack = onBack) { modifier ->
        LazyColumn(modifier = modifier.fillMaxSize()) {
            item {
                Text(
                    stringResource(R.string.licenses_credit),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                )
            }
            items(licenses) { license ->
                ListItem(
                    headlineContent = { Text(license.name) },
                    supportingContent = { Text(stringResource(license.roleRes)) },
                    trailingContent = { Icon(Icons.AutoMirrored.Filled.KeyboardArrowRight, null) },
                    modifier = Modifier.clickable { onSelect(license) },
                )
                HorizontalDivider()
            }
            item {
                Text(
                    stringResource(R.string.licenses_footer),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(16.dp),
                )
            }
        }
    }
}

@Composable
private fun LicenseDetailPage(license: License, onBack: () -> Unit) {
    SettingsScaffold(title = license.name, onBack = onBack) { modifier ->
        Text(
            license.text,
            fontSize = 11.sp,
            fontFamily = FontFamily.Monospace,
            modifier = modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(16.dp),
        )
    }
}
