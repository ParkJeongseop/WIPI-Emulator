package com.parkjeongseop.wipi

import android.graphics.Bitmap
import android.net.Uri
import android.os.Bundle
import android.view.InputDevice
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.systemBarsPadding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Pause
import androidx.compose.material.icons.filled.PauseCircle
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.SportsEsports
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalLifecycleOwner
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import java.io.File
import kotlin.math.roundToInt

const val SCREEN_WIDTH = 240
const val SCREEN_HEIGHT = 320
const val SOUNDFONT_NAME = "GeneralUser-GS.sf2"

private enum class Screen { Library, Emulator }

class MainActivity : ComponentActivity() {
    private lateinit var library: GameLibrary
    private lateinit var settings: Settings
    private lateinit var haptics: Haptics

    /** 에뮬 화면에서만 하드웨어 키를 게임으로 전달 */
    private var emulatorActive = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        WipiNative.init(this)
        library = GameLibrary(this)
        settings = Settings(this)
        haptics = Haptics(this)

        setContent {
            WipiTheme {
                WipiApp()
            }
        }
    }

    override fun onDestroy() {
        super.onDestroy()
        WipiNative.nativeStop()
    }

    @Composable
    private fun WipiApp() {
        var screen by remember { mutableStateOf(Screen.Library) }
        var games by remember { mutableStateOf(library.list()) }
        var errorMessage by remember { mutableStateOf<String?>(null) }
        var showSettings by remember { mutableStateOf(false) } // iOS sheet 대응 오버레이

        emulatorActive = screen == Screen.Emulator

        Box(modifier = Modifier.fillMaxSize()) {
            when (screen) {
                Screen.Library -> LibraryScreen(
                    games = games,
                    error = errorMessage,
                    onDismissError = { errorMessage = null },
                    onPlay = { entry ->
                        entry.dataDir.mkdirs()
                        if (start(entry.gameFile.readBytes(), entry.filename, entry.dataDir)) {
                            errorMessage = null
                            screen = Screen.Emulator
                        } else {
                            // 동기 실패는 형식 문제가 아니라 시작 자체 실패 (형식 오류는 로드 후 kind로 안내됨)
                            errorMessage = pendingError() ?: getString(R.string.error_start_failed)
                        }
                    },
                    onImport = { uri ->
                        val entry = library.importGame(uri)
                        games = library.list()
                        entry != null
                    },
                    onDelete = { entry ->
                        library.delete(entry)
                        games = library.list()
                    },
                    onSettings = { showSettings = true },
                )

                Screen.Emulator -> EmulatorScreen(
                    onError = { message ->
                        errorMessage = message
                        WipiNative.nativeStop()
                        screen = Screen.Library
                    },
                    onExit = {
                        WipiNative.nativeStop()
                        screen = Screen.Library
                    },
                    onSettings = { showSettings = true },
                )
            }

            // 설정은 오버레이(iOS sheet 대응) — 에뮬 화면이 뒤에 살아있고 폴링도 계속된다
            if (showSettings) {
                SettingsFlow(
                    settings = settings,
                    onVolumeChanged = {
                    val (pcm, midi) = settings.effectiveVolumes()
                    WipiNative.nativeSetVolume(pcm, midi)
                },
                    onClose = { showSettings = false },
                )
            }
        }
    }

    /** 보류 중 오류를 사용자 안내 문구 + 진단 원문으로 변환 (없으면 null). iOS WipiCore.describeError와 동일 카피. */
    private fun pendingError(): String? {
        val kind = IntArray(1)
        val detail = WipiNative.nativeGetError(kind) ?: return null
        return when (kind[0]) {
            0 -> getString(R.string.error_load_failed, detail)
            else -> getString(R.string.error_runtime, detail)
        }
    }

    /** 게임 로드 및 에뮬레이터 시작 + 사운드 설정 적용 */
    private fun start(bytes: ByteArray, filename: String, dataDir: File): Boolean {
        val ok = WipiNative.nativeStart(bytes, filename, dataDir.absolutePath, soundfontPath())
        if (ok) {
            val (pcm, midi) = settings.effectiveVolumes()
            WipiNative.nativeSetVolume(pcm, midi)
        }
        return ok
    }

    /** assets의 사운드폰트를 filesDir로 복사하고 경로 반환 (실패 시 빈 문자열 → MIDI 무음) */
    private fun soundfontPath(): String = try {
        val file = File(filesDir, SOUNDFONT_NAME)
        if (!file.exists()) {
            assets.open(SOUNDFONT_NAME).use { input ->
                file.outputStream().use { output -> input.copyTo(output) }
            }
        }
        file.absolutePath
    } catch (e: Exception) {
        ""
    }

    // ── 하드웨어 입력: 키보드(wie_cli 배치) + 게임패드 버튼 ──

    private val hardwareKeyMap = mapOf(
        KeyEvent.KEYCODE_DPAD_UP to "UP",
        KeyEvent.KEYCODE_DPAD_DOWN to "DOWN",
        KeyEvent.KEYCODE_DPAD_LEFT to "LEFT",
        KeyEvent.KEYCODE_DPAD_RIGHT to "RIGHT",
        KeyEvent.KEYCODE_DPAD_CENTER to "OK",
        KeyEvent.KEYCODE_ENTER to "OK",
        KeyEvent.KEYCODE_SPACE to "OK",
        KeyEvent.KEYCODE_1 to "1",
        KeyEvent.KEYCODE_2 to "2",
        KeyEvent.KEYCODE_3 to "3",
        KeyEvent.KEYCODE_Q to "4",
        KeyEvent.KEYCODE_W to "5",
        KeyEvent.KEYCODE_E to "6",
        KeyEvent.KEYCODE_A to "7",
        KeyEvent.KEYCODE_S to "8",
        KeyEvent.KEYCODE_D to "9",
        KeyEvent.KEYCODE_Z to "*",
        KeyEvent.KEYCODE_X to "0",
        KeyEvent.KEYCODE_C to "#",
        KeyEvent.KEYCODE_4 to "4",
        KeyEvent.KEYCODE_5 to "5",
        KeyEvent.KEYCODE_6 to "6",
        KeyEvent.KEYCODE_7 to "7",
        KeyEvent.KEYCODE_8 to "8",
        KeyEvent.KEYCODE_9 to "9",
        KeyEvent.KEYCODE_0 to "0",
        KeyEvent.KEYCODE_STAR to "*",
        KeyEvent.KEYCODE_POUND to "#",
        KeyEvent.KEYCODE_DEL to "CLR",
        KeyEvent.KEYCODE_SHIFT_LEFT to "SOFT_L",
        KeyEvent.KEYCODE_SHIFT_RIGHT to "SOFT_R",
        KeyEvent.KEYCODE_F1 to "CALL",
        KeyEvent.KEYCODE_F2 to "HANGUP",
        // 게임패드 버튼 (iOS GCController 매핑과 동일: A→OK, B→CLR, X→*, Y→#, 숄더→소프트키)
        KeyEvent.KEYCODE_BUTTON_A to "OK",
        KeyEvent.KEYCODE_BUTTON_B to "CLR",
        KeyEvent.KEYCODE_BUTTON_X to "*",
        KeyEvent.KEYCODE_BUTTON_Y to "#",
        KeyEvent.KEYCODE_BUTTON_L1 to "SOFT_L",
        KeyEvent.KEYCODE_BUTTON_R1 to "SOFT_R",
    )

    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean {
        if (!emulatorActive) return super.onKeyDown(keyCode, event)
        val key = hardwareKeyMap[keyCode] ?: return super.onKeyDown(keyCode, event)
        if (event.repeatCount == 0) WipiNative.nativeKeyDown(key) // 키 반복은 Rust 쪽에서 처리
        return true
    }

    override fun onKeyUp(keyCode: Int, event: KeyEvent): Boolean {
        if (!emulatorActive) return super.onKeyUp(keyCode, event)
        val key = hardwareKeyMap[keyCode] ?: return super.onKeyUp(keyCode, event)
        WipiNative.nativeKeyUp(key)
        return true
    }

    /** 게임패드 왼쪽 스틱 → 방향키 (iOS GCController leftThumbstick 매핑과 동일, 임계값 0.5) */
    private val stickPressed = mutableSetOf<String>()

    override fun onGenericMotionEvent(event: MotionEvent): Boolean {
        if (!emulatorActive || event.source and InputDevice.SOURCE_JOYSTICK == 0) {
            return super.onGenericMotionEvent(event)
        }

        val x = event.getAxisValue(MotionEvent.AXIS_X)
        val y = event.getAxisValue(MotionEvent.AXIS_Y)

        fun update(key: String, active: Boolean) {
            if (active && key !in stickPressed) {
                stickPressed += key
                WipiNative.nativeKeyDown(key)
            } else if (!active && key in stickPressed) {
                stickPressed -= key
                WipiNative.nativeKeyUp(key)
            }
        }
        update("LEFT", x < -0.5f)
        update("RIGHT", x > 0.5f)
        update("UP", y < -0.5f)
        update("DOWN", y > 0.5f)
        return true
    }

    // ── 에뮬레이터 화면 (iOS EmulatorScreenView가 명세) ──

    @Composable
    private fun EmulatorScreen(onError: (String) -> Unit, onExit: () -> Unit, onSettings: () -> Unit) {
        val bitmap = remember { Bitmap.createBitmap(SCREEN_WIDTH, SCREEN_HEIGHT, Bitmap.Config.ARGB_8888) }
        val pixels = remember { IntArray(SCREEN_WIDTH * SCREEN_HEIGHT) }
        val vibrateOut = remember { LongArray(2) }
        var frame by remember { mutableStateOf<ImageBitmap?>(null) }
        var paused by remember { mutableStateOf(false) }
        // 성능 표시: 화면에 새로 나간 프레임을 세어 1초마다 FPS로 표시 (설정이 꺼져 있으면 null)
        var performanceText by remember { mutableStateOf<String?>(null) }

        // 시스템 뒤로가기: 실수 종료 방지를 위해 먼저 일시정지 메뉴, 일시정지 중 다시 누르면 종료 (에뮬 표준 패턴)
        BackHandler {
            if (paused) {
                onExit()
            } else {
                paused = true
                WipiNative.nativeSetPaused(true)
            }
        }

        // 게임 중 화면 자동꺼짐 방지
        DisposableEffect(Unit) {
            window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
            onDispose { window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON) }
        }

        // 포커스를 잃으면(ON_PAUSE — iOS .inactive 대응) 일시정지.
        // 복귀해도 자동 재개하지 않고 오버레이 유지 (탭해야 재개)
        val lifecycleOwner = LocalLifecycleOwner.current
        DisposableEffect(lifecycleOwner) {
            val observer = LifecycleEventObserver { _, event ->
                if (event == Lifecycle.Event.ON_PAUSE && !paused) {
                    paused = true
                    WipiNative.nativeSetPaused(true)
                }
            }
            lifecycleOwner.lifecycle.addObserver(observer)
            onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
        }

        LaunchedEffect(Unit) {
            var sampledAt = System.nanoTime()
            var presentedFrames = 0
            while (isActive) {
                if (WipiNative.nativeGetFrame(pixels)) {
                    bitmap.setPixels(pixels, 0, SCREEN_WIDTH, 0, 0, SCREEN_WIDTH, SCREEN_HEIGHT)
                    frame = bitmap.asImageBitmap().also { it.prepareToDraw() }
                    presentedFrames++
                }
                if (settings.showPerformance) {
                    val now = System.nanoTime()
                    if (now - sampledAt >= 1_000_000_000L) {
                        val fps = presentedFrames * 1_000_000_000.0 / (now - sampledAt)
                        performanceText = "${fps.roundToInt()} FPS  CPU ${WipiNative.nativeCpuLoad()}%"
                        sampledAt = now
                        presentedFrames = 0
                    }
                } else if (performanceText != null) {
                    performanceText = null
                }
                // enabled가 아니어도 요청은 소비해야 큐가 쌓이지 않는다
                if (WipiNative.nativePollVibrate(vibrateOut) && settings.vibrationEnabled) {
                    haptics.play(vibrateOut[0], vibrateOut[1].toInt(), settings.vibrationScale)
                }
                if (WipiNative.nativePollExit()) {
                    onExit()
                    break
                }
                pendingError()?.let {
                    onError(it)
                    return@LaunchedEffect
                }
                delay(16)
            }
        }

        // targetSdk 35+는 edge-to-edge가 강제라, 시스템 바 영역을 비워 두지 않으면 게임 화면 윗줄이
        // 상태바 밑으로, 키패드 아랫줄이 제스처 바 밑으로 들어간다.
        Box(modifier = Modifier.fillMaxSize().background(Color(0xFF202020)).systemBarsPadding()) {
            Column(modifier = Modifier.fillMaxSize()) {
                // 화면 영역 — ContentScale.Fit이 영역 안에서 레터박스 (키패드 침범 없음)
                Box(
                    modifier = Modifier.fillMaxWidth().weight(1f),
                    contentAlignment = Alignment.Center,
                ) {
                    frame?.let {
                        Image(
                            bitmap = it,
                            contentDescription = null,
                            contentScale = ContentScale.Fit,
                            filterQuality = FilterQuality.None,
                            modifier = Modifier.fillMaxSize(),
                        )
                    } ?: Text(stringResource(R.string.emulator_loading), color = Color.White)
                }

                Keypad(hapticsEnabled = settings.keypadHaptics, modifier = Modifier.fillMaxWidth().padding(8.dp))
            }

            // 좌상단: 성능 표시 (설정에서 켠 경우)
            performanceText?.let {
                Text(
                    it,
                    color = Color.White,
                    fontSize = 12.sp,
                    fontFamily = FontFamily.Monospace,
                    modifier = Modifier
                        .align(Alignment.TopStart)
                        .padding(10.dp)
                        .background(Color.Black.copy(alpha = 0.55f), RoundedCornerShape(4.dp))
                        .padding(horizontal = 6.dp, vertical = 3.dp),
                )
            }

            // 우상단: 일시정지 + 설정 (iOS와 동일 구성)
            Row(modifier = Modifier.align(Alignment.TopEnd)) {
                IconButton(onClick = {
                    if (!paused) {
                        paused = true
                        WipiNative.nativeSetPaused(true)
                    }
                }) {
                    Icon(Icons.Filled.Pause, stringResource(R.string.emulator_paused), tint = Color.White.copy(alpha = 0.6f))
                }
                IconButton(onClick = onSettings) {
                    Icon(Icons.Filled.Settings, stringResource(R.string.action_settings), tint = Color.White.copy(alpha = 0.6f))
                }
            }

            if (paused) {
                Box(
                    modifier = Modifier.fillMaxSize().background(Color.Black.copy(alpha = 0.6f)).pointerInput(Unit) {
                        detectTapGestures {
                            paused = false
                            WipiNative.nativeSetPaused(false)
                        }
                    },
                    contentAlignment = Alignment.Center,
                ) {
                    Column(horizontalAlignment = Alignment.CenterHorizontally) {
                        Icon(
                            Icons.Filled.PauseCircle, null,
                            tint = Color.White,
                            modifier = Modifier.size(64.dp),
                        )
                        Text(stringResource(R.string.emulator_paused), color = Color.White, fontSize = 18.sp, modifier = Modifier.padding(top = 8.dp))
                        Text(stringResource(R.string.emulator_resume_hint), color = Color.White.copy(alpha = 0.7f), fontSize = 14.sp)
                        Button(onClick = onExit, modifier = Modifier.padding(top = 24.dp)) {
                            Text(stringResource(R.string.emulator_exit))
                        }
                    }
                }
            }
        }
    }

    // ── 게임 라이브러리 화면 (iOS LibraryView가 명세) ──

    @OptIn(ExperimentalFoundationApi::class, ExperimentalMaterial3Api::class)
    @Composable
    private fun LibraryScreen(
        games: List<GameEntry>,
        error: String?,
        onDismissError: () -> Unit,
        onPlay: (GameEntry) -> Unit,
        onImport: (Uri) -> Boolean,
        onDelete: (GameEntry) -> Unit,
        onSettings: () -> Unit,
    ) {
        var importError by remember { mutableStateOf<String?>(null) }
        val readError = stringResource(R.string.import_read_error)
        var deleteTarget by remember { mutableStateOf<GameEntry?>(null) }

        // zip/jar만 (iOS allowedContentTypes 대응). octet-stream은 파일관리자가 타입을 모르는 경우 대비
        val picker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
            if (uri != null && !onImport(uri)) importError = readError
        }

        // Material 표준: TopAppBar(좌정렬 타이틀 + 우측 액션들)
        Scaffold(
            topBar = {
                TopAppBar(
                    title = { Text(stringResource(R.string.app_name)) },
                    actions = {
                        IconButton(onClick = {
                            picker.launch(arrayOf("application/zip", "application/java-archive", "application/octet-stream"))
                        }) { Icon(Icons.Filled.Add, stringResource(R.string.action_add_game)) }
                        IconButton(onClick = onSettings) { Icon(Icons.Filled.Settings, stringResource(R.string.action_settings)) }
                    },
                )
            },
        ) { innerPadding ->
            Box(modifier = Modifier.fillMaxSize().padding(innerPadding)) {
                Column(modifier = Modifier.fillMaxSize().padding(horizontal = 16.dp)) {
                if (games.isEmpty()) {
                    Column(
                        modifier = Modifier.fillMaxSize(),
                        verticalArrangement = Arrangement.Center,
                        horizontalAlignment = Alignment.CenterHorizontally,
                    ) {
                        Icon(
                            Icons.Filled.SportsEsports, null,
                            tint = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.size(48.dp),
                        )
                        Text(stringResource(R.string.library_empty_title), style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(top = 12.dp))
                        Text(
                            stringResource(R.string.library_empty_hint),
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            textAlign = TextAlign.Center,
                        )
                    }
                } else {
                    LazyVerticalGrid(
                        columns = GridCells.Adaptive(minSize = 100.dp),
                        horizontalArrangement = Arrangement.spacedBy(16.dp),
                        verticalArrangement = Arrangement.spacedBy(16.dp),
                        modifier = Modifier.padding(top = 16.dp),
                    ) {
                        items(games, key = { it.id }) { game ->
                            Column(
                                horizontalAlignment = Alignment.CenterHorizontally,
                                modifier = Modifier.combinedClickable(
                                    onClick = { onPlay(game) },
                                    onLongClick = { deleteTarget = game },
                                ),
                            ) {
                                Box(
                                    modifier = Modifier.size(100.dp).background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(12.dp)),
                                    contentAlignment = Alignment.Center,
                                ) {
                                    game.cover?.let {
                                        Image(
                                            bitmap = it.asImageBitmap(),
                                            contentDescription = null,
                                            filterQuality = FilterQuality.None, // 저해상도 아이콘을 nearest로 확대
                                            modifier = Modifier.fillMaxSize().padding(8.dp),
                                        )
                                    } ?: Icon(
                                        Icons.Filled.SportsEsports, null,
                                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                                        modifier = Modifier.size(32.dp),
                                    )
                                }
                                Text(
                                    game.name,
                                    fontSize = 12.sp,
                                    maxLines = 2,
                                    textAlign = TextAlign.Center,
                                    modifier = Modifier.padding(top = 4.dp).height(34.dp),
                                )
                            }
                        }
                    }
                }
            }

                // 실행 에러: 하단 배너, 탭하면 닫힘
                error?.let {
                    Text(
                        it,
                        color = MaterialTheme.colorScheme.onErrorContainer,
                        fontSize = 13.sp,
                        textAlign = TextAlign.Center,
                        modifier = Modifier
                            .align(Alignment.BottomCenter)
                            .fillMaxWidth()
                            .background(MaterialTheme.colorScheme.errorContainer)
                            .pointerInput(Unit) { detectTapGestures { onDismissError() } }
                            .padding(12.dp),
                    )
                }
            }
        }

        // 임포트 실패 알럿 (iOS alert 대응)
        importError?.let {
            AlertDialog(
                onDismissRequest = { importError = null },
                title = { Text(stringResource(R.string.import_failed_title)) },
                text = { Text(it) },
                confirmButton = { TextButton(onClick = { importError = null }) { Text(stringResource(R.string.action_ok)) } },
            )
        }

        // 삭제 확인 (iOS contextMenu의 확인 단계 대응 — 세이브까지 지워지므로 필수)
        deleteTarget?.let { target ->
            AlertDialog(
                onDismissRequest = { deleteTarget = null },
                title = { Text(stringResource(R.string.delete_game_title)) },
                text = { Text(stringResource(R.string.delete_game_message, target.name)) },
                confirmButton = {
                    TextButton(onClick = {
                        onDelete(target)
                        deleteTarget = null
                    }) { Text(stringResource(R.string.action_delete), color = MaterialTheme.colorScheme.error) }
                },
                dismissButton = { TextButton(onClick = { deleteTarget = null }) { Text(stringResource(R.string.action_cancel)) } },
            )
        }
    }
}

// ── 키패드 (iOS Keypad와 동일: 눌림 피드백 포함) ──

private val KEYPAD_ROWS = listOf(
    listOf("SOFT_L" to "◁", "UP" to "▲", "SOFT_R" to "▷"),
    listOf("LEFT" to "◀", "OK" to "OK", "RIGHT" to "▶"),
    listOf("CALL" to "📞", "DOWN" to "▼", "CLR" to "CLR"),
    listOf("1" to "1", "2" to "2", "3" to "3"),
    listOf("4" to "4", "5" to "5", "6" to "6"),
    listOf("7" to "7", "8" to "8", "9" to "9"),
    listOf("*" to "*", "0" to "0", "#" to "#"),
)

@Composable
fun Keypad(hapticsEnabled: Boolean, modifier: Modifier = Modifier) {
    Column(modifier = modifier, verticalArrangement = Arrangement.spacedBy(4.dp)) {
        KEYPAD_ROWS.forEach { row ->
            Row(horizontalArrangement = Arrangement.spacedBy(4.dp), modifier = Modifier.fillMaxWidth()) {
                row.forEach { (key, label) ->
                    KeyButton(key = key, label = label, hapticsEnabled = hapticsEnabled, modifier = Modifier.weight(1f))
                }
            }
        }
    }
}

@Composable
fun KeyButton(key: String, label: String, hapticsEnabled: Boolean, modifier: Modifier = Modifier) {
    var pressed by remember { mutableStateOf(false) }
    val view = androidx.compose.ui.platform.LocalView.current

    Box(
        modifier = modifier
            .height(44.dp)
            .background(Color(0xFF404040).copy(alpha = if (pressed) 0.6f else 1f), RoundedCornerShape(8.dp))
            .pointerInput(key) {
                detectTapGestures(
                    onPress = {
                        pressed = true
                        // 키 다운 시 가벼운 탭 — 하드웨어 키/게임패드에는 적용하지 않음 (물리 피드백 존재)
                        if (hapticsEnabled) view.performHapticFeedback(android.view.HapticFeedbackConstants.VIRTUAL_KEY)
                        WipiNative.nativeKeyDown(key)
                        try {
                            awaitRelease()
                        } finally {
                            pressed = false
                            WipiNative.nativeKeyUp(key)
                        }
                    },
                )
            },
        contentAlignment = Alignment.Center,
    ) {
        Text(label, color = Color.White, fontSize = 16.sp)
    }
}
