// 메모리 치트 (iOS CheatView가 명세): 값 검색 → 좁히기 → 주소 저장 → 값 쓰기/고정.
// 검색 상태는 코어(wipi_core::cheat)가 세션 동안 들고 있고, 여기는 화면 상태와 게임별 저장 목록을 맡는다.

package com.parkjeongseop.wipi

import android.content.Context
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.core.content.edit
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject

/** 화면에 보여 줄 검색 후보 수 */
private const val RESULT_LIMIT = 50

/** 코어가 보관하는 후보 수 상한 (wipi_core::cheat::MAX_CANDIDATES) */
private const val MAX_CANDIDATES = 1_000_000L

private val WIDTHS = listOf(4, 2, 1)

/** 검색 후보 하나. 주소·값은 u32 비트 패턴. */
data class CheatResult(val address: Int, val value: Int)

/** 저장된 치트. 주소가 곧 식별자다 (코어의 고정 목록도 주소로 구분). */
data class SavedCheat(val name: String, val address: Int, val width: Int, val value: Int, val frozen: Boolean)

/** 4바이트 값은 부호 있는 수로, 1·2바이트는 부호 없는 수로 보여 준다 */
fun cheatValueText(value: Int, width: Int): String = when (width) {
    1 -> (value and 0xFF).toString()
    2 -> (value and 0xFFFF).toString()
    else -> value.toString()
}

/** 음수도 받는다 (폭에 맞춰 잘림). 숫자가 아니거나 32비트를 넘으면 null. */
fun parseCheatValue(text: String): Int? =
    text.trim().toLongOrNull()?.takeIf { it in Int.MIN_VALUE.toLong()..0xFFFF_FFFFL }?.toInt()

/** 게임 한 판(세션) 동안의 치트 상태. 화면을 닫았다 열어도 검색 진행이 유지된다. */
class CheatState(context: Context, private val gameId: String) {
    private val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)

    // 네이티브 호출은 에뮬레이터 스레드의 응답을 기다리므로 백그라운드에서, 요청 순서대로 하나씩
    private val lock = Mutex()

    /** null = 확인 중 */
    var supported by mutableStateOf<Boolean?>(null)
        private set
    var busy by mutableStateOf(false)
        private set

    /** 직전 검색의 일치 수 (null = 아직 검색 안 함) */
    var matchCount by mutableStateOf<Long?>(null)
        private set
    var searchWidth by mutableIntStateOf(4)
        private set
    var results by mutableStateOf<List<CheatResult>>(emptyList())
        private set
    val saved = mutableStateListOf<SavedCheat>()

    /** 저장된 치트 주소의 현재 값 (읽을 수 없는 주소는 없음) */
    var current by mutableStateOf<Map<Int, Int>>(emptyMap())
        private set

    init {
        saved += load()
        native({ WipiNative.nativeCheatSupported() }) { supported = it }
    }

    fun close() = scope.cancel()

    private fun <T> native(block: () -> T, then: (T) -> Unit = {}) {
        scope.launch {
            then(lock.withLock { withContext(Dispatchers.IO) { block() } })
        }
    }

    private fun readResults(): List<CheatResult> =
        WipiNative.nativeCheatResults(RESULT_LIMIT)?.map { CheatResult((it ushr 32).toInt(), it.toInt()) } ?: emptyList()

    fun search(value: Int, width: Int, refine: Boolean) {
        busy = true
        native({ WipiNative.nativeCheatSearch(value, width, refine) to readResults() }) { (count, found) ->
            busy = false
            if (count >= 0) {
                matchCount = count
                if (!refine) searchWidth = width
                results = found
            }
        }
    }

    /** 후보와 저장된 치트의 현재 값을 다시 읽는다 (화면을 열 때 — 그 사이 게임이 값을 바꿨을 수 있다) */
    fun refresh() {
        if (matchCount != null) native({ readResults() }) { results = it }
        val cheats = saved.toList()
        native({
            cheats.mapNotNull { cheat ->
                WipiNative.nativeCheatRead(cheat.address, cheat.width).takeIf { it >= 0 }?.let { cheat.address to it.toInt() }
            }.toMap()
        }) { current = it }
    }

    fun add(result: CheatResult) {
        if (saved.none { it.address == result.address }) {
            saved += SavedCheat("", result.address, searchWidth, result.value, false)
            current = current + (result.address to result.value)
            persist()
        }
    }

    private fun update(address: Int, change: (SavedCheat) -> SavedCheat): SavedCheat? {
        val index = saved.indexOfFirst { it.address == address }
        if (index < 0) return null
        saved[index] = change(saved[index])
        persist()
        return saved[index]
    }

    fun rename(cheat: SavedCheat, name: String) {
        update(cheat.address) { it.copy(name = name) }
    }

    /** 값을 바꾼다. 고정 중이면 새 값으로 바로 고정한다. */
    fun setValue(cheat: SavedCheat, value: Int) {
        val updated = update(cheat.address) { it.copy(value = value) } ?: return
        if (updated.frozen) native({ WipiNative.nativeCheatFreeze(updated.address, updated.width, updated.value) })
    }

    /** 값을 한 번 쓴다 */
    fun write(cheat: SavedCheat) {
        native({ WipiNative.nativeCheatWrite(cheat.address, cheat.width, cheat.value) }) { ok ->
            if (ok) current = current + (cheat.address to cheat.value)
        }
    }

    fun setFrozen(cheat: SavedCheat, frozen: Boolean) {
        val updated = update(cheat.address) { it.copy(frozen = frozen) } ?: return
        if (frozen) {
            native({ WipiNative.nativeCheatFreeze(updated.address, updated.width, updated.value) }) { ok ->
                if (ok) {
                    current = current + (updated.address to updated.value)
                } else {
                    // 이번 실행에선 쓸 수 없는 주소 (게임을 다시 시작해 메모리 배치가 달라진 경우)
                    update(updated.address) { it.copy(frozen = false) }
                }
            }
        } else {
            native({ WipiNative.nativeCheatUnfreeze(updated.address) })
        }
    }

    fun remove(cheat: SavedCheat) {
        saved.removeAll { it.address == cheat.address }
        persist()
        native({ WipiNative.nativeCheatUnfreeze(cheat.address) })
    }

    // 저장: 게임 id → [{name, address, width, value}]. 고정 여부는 저장하지 않는다 —
    // 주소는 실행마다 달라질 수 있어 다음 실행에선 꺼진 상태로 불러온다.

    private fun load(): List<SavedCheat> = try {
        val array = JSONArray(prefs.getString(gameId, "[]"))
        (0 until array.length()).map { index ->
            val item = array.getJSONObject(index)
            SavedCheat(item.getString("name"), item.getInt("address"), item.getInt("width"), item.getInt("value"), false)
        }
    } catch (e: Exception) {
        emptyList()
    }

    private fun persist() {
        val array = JSONArray()
        saved.forEach {
            array.put(JSONObject().put("name", it.name).put("address", it.address).put("width", it.width).put("value", it.value))
        }
        prefs.edit { putString(gameId, array.toString()) }
    }

    companion object {
        private const val PREFS = "cheats"

        /** 게임을 지울 때 그 게임의 저장된 치트도 지운다 */
        fun forget(context: Context, gameId: String) {
            context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit { remove(gameId) }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CheatScreen(state: CheatState, onClose: () -> Unit) {
    BackHandler(onBack = onClose)
    LaunchedEffect(state) { state.refresh() }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.cheat_title)) },
                navigationIcon = {
                    IconButton(onClick = onClose) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.action_back)) }
                },
            )
        },
    ) { innerPadding ->
        when (state.supported) {
            null -> Box(Modifier.fillMaxSize().padding(innerPadding), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
            false -> Text(
                stringResource(R.string.cheat_unsupported),
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(innerPadding).padding(16.dp),
            )
            true -> CheatList(state, innerPadding)
        }
    }
}

@Composable
private fun SectionHeader(title: String) {
    Text(
        title,
        style = MaterialTheme.typography.labelLarge,
        color = MaterialTheme.colorScheme.primary,
        modifier = Modifier.padding(top = 20.dp, bottom = 4.dp),
    )
}

@Composable
private fun Note(text: String) {
    Text(text, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun CheatList(state: CheatState, innerPadding: PaddingValues) {
    var valueText by rememberSaveable { mutableStateOf("") }
    var width by rememberSaveable { mutableIntStateOf(4) }
    val value = parseCheatValue(valueText)
    val matchCount = state.matchCount

    LazyColumn(
        modifier = Modifier.fillMaxSize().padding(innerPadding),
        contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 24.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        item {
            SectionHeader(stringResource(R.string.cheat_search_header))
            Note(stringResource(R.string.cheat_howto))
        }
        item {
            OutlinedTextField(
                value = valueText,
                onValueChange = { valueText = it },
                label = { Text(stringResource(R.string.cheat_value)) },
                singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                modifier = Modifier.fillMaxWidth(),
            )
        }
        item {
            SingleChoiceSegmentedButtonRow(modifier = Modifier.fillMaxWidth()) {
                WIDTHS.forEachIndexed { index, option ->
                    SegmentedButton(
                        selected = option == width,
                        onClick = { width = option },
                        shape = SegmentedButtonDefaults.itemShape(index, WIDTHS.size),
                        icon = {},
                    ) { Text(stringResource(R.string.cheat_width_bytes, option)) }
                }
            }
        }
        item {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button(
                    onClick = { value?.let { state.search(it, width, refine = false) } },
                    enabled = value != null && !state.busy,
                    modifier = Modifier.weight(1f),
                ) { Text(stringResource(R.string.cheat_new_search)) }
                OutlinedButton(
                    onClick = { value?.let { state.search(it, width, refine = true) } },
                    enabled = value != null && !state.busy && (matchCount ?: 0) > 0,
                    modifier = Modifier.weight(1f),
                ) { Text(stringResource(R.string.cheat_refine)) }
            }
        }
        item {
            when {
                state.busy -> Note(stringResource(R.string.cheat_searching))
                matchCount == null -> {}
                matchCount == 0L -> Note(stringResource(R.string.cheat_no_match))
                else -> Column {
                    Text(stringResource(R.string.cheat_match_count, matchCount), style = MaterialTheme.typography.titleSmall)
                    if (matchCount > MAX_CANDIDATES) {
                        Note(stringResource(R.string.cheat_too_many))
                    } else if (matchCount > state.results.size) {
                        Note(stringResource(R.string.cheat_showing_first, state.results.size))
                    }
                }
            }
        }
        items(state.results, key = { "result-${it.address}" }) { result ->
            val isSaved = state.saved.any { it.address == result.address }
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text("0x%08X".format(result.address), fontFamily = FontFamily.Monospace, modifier = Modifier.weight(1f))
                Text(cheatValueText(result.value, state.searchWidth), fontFamily = FontFamily.Monospace)
                IconButton(onClick = { state.add(result) }, enabled = !isSaved) {
                    Icon(if (isSaved) Icons.Filled.Check else Icons.Filled.Add, stringResource(R.string.cheat_add))
                }
            }
        }

        item {
            SectionHeader(stringResource(R.string.cheat_saved_header))
            Note(stringResource(if (state.saved.isEmpty()) R.string.cheat_saved_empty else R.string.cheat_saved_footer))
        }
        items(state.saved, key = { "saved-${it.address}" }) { cheat ->
            SavedCheatRow(state, cheat)
            HorizontalDivider(modifier = Modifier.padding(top = 8.dp))
        }
    }
}

@Composable
private fun SavedCheatRow(state: CheatState, cheat: SavedCheat) {
    // 입력 중인 글자는 여기서 들고, 숫자로 읽힐 때만 상태에 반영한다
    var valueText by remember(cheat.address) { mutableStateOf(cheatValueText(cheat.value, cheat.width)) }

    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            OutlinedTextField(
                value = cheat.name,
                onValueChange = { state.rename(cheat, it) },
                label = { Text(stringResource(R.string.cheat_name)) },
                singleLine = true,
                modifier = Modifier.weight(1f),
            )
            IconButton(onClick = { state.remove(cheat) }) { Icon(Icons.Filled.Delete, stringResource(R.string.action_delete)) }
        }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedTextField(
                value = valueText,
                onValueChange = { text ->
                    valueText = text
                    parseCheatValue(text)?.let { state.setValue(cheat, it) }
                },
                label = { Text(stringResource(R.string.cheat_value)) },
                supportingText = {
                    // "0x40001234 · 4바이트 · 현재 1200"
                    val current = state.current[cheat.address]?.let {
                        " · " + stringResource(R.string.cheat_current, cheatValueText(it, cheat.width))
                    }
                    Text(
                        "0x%08X · ".format(cheat.address) + stringResource(R.string.cheat_width_bytes, cheat.width) + current.orEmpty(),
                        fontFamily = FontFamily.Monospace,
                    )
                },
                singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                modifier = Modifier.weight(1f),
            )
            TextButton(onClick = { state.write(cheat) }) { Text(stringResource(R.string.cheat_write)) }
            Column(horizontalAlignment = Alignment.CenterHorizontally) {
                Switch(checked = cheat.frozen, onCheckedChange = { state.setFrozen(cheat, it) })
                Text(stringResource(R.string.cheat_freeze), style = MaterialTheme.typography.labelSmall)
            }
        }
    }
}
