// C ABI for the wie emulator core (wipi_ios crate).
// Threading model: wipi_start spawns a dedicated emulator thread. The UI
// polls frames at 60fps with wipi_get_frame — no callbacks.

#ifndef WIPI_IOS_H
#define WIPI_IOS_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#define WIPI_SCREEN_WIDTH 240
#define WIPI_SCREEN_HEIGHT 320

#ifdef __cplusplus
extern "C" {
#endif

// Initializes logging and the panic hook. Call once at app startup.
void wipi_init(void);

// Loads a game (.zip/.jar bytes) and starts the emulator thread.
// soundfont_path may be "" for silent MIDI. Returns false on failure
// (check wipi_get_error). Fails if a session is already running.
bool wipi_start(const uint8_t *game_data, size_t game_data_len,
               const char *filename, const char *data_dir,
               const char *soundfont_path);

// Copies the latest frame as RGBA8888 (row-major, WIDTH*HEIGHT*4 bytes).
// Returns true when a new frame was written since the last call.
bool wipi_get_frame(uint8_t *out_rgba, size_t capacity);

// Key names: "UP" "DOWN" "LEFT" "RIGHT" "OK" "0".."9" "*" "#"
//            "CLR" "SOFT_L" "SOFT_R" "CALL" "HANGUP"
void wipi_key_down(const char *key);
void wipi_key_up(const char *key);

// Takes the pending error and clears it. Returns true if one was pending.
// out_kind: 0 = load failed (unsupported/corrupt file), 1 = runtime error
// (compatibility). buf receives the diagnostic message (UTF-8, English) —
// the host picks user-facing copy from the kind and shows the message as
// detail. 256-byte buffer recommended. out_kind may be NULL.
bool wipi_get_error(char *buf, size_t capacity, uint8_t *out_kind);

// Polls a pending vibration requested by the game. Returns true if one was
// pending and fills out_duration_ms (ms) and out_intensity (0..100).
bool wipi_poll_vibrate(uint64_t *out_duration_ms, uint8_t *out_intensity);

// Pauses/resumes emulation. While paused the tick loop is frozen (standard
// emulator auto-pause). Call with true when entering background.
void wipi_set_paused(bool paused);

// Sets host volumes (0.0~1.0, 0 mutes) for PCM (sound effects) and MIDI
// (background music) separately — soundfont loudness and in-game sample
// loudness vary per game, so users need balance control.
void wipi_set_volume(float pcm_volume, float midi_volume);

// Share of the last second the emulator thread spent running the game,
// 0..100. Near 100 the device limits the frame rate; well below it the game
// is pacing itself.
uint32_t wipi_cpu_load(void);

// Polls whether the game requested to exit. If true, the host should call
// wipi_stop and return to its menu/library UI.
bool wipi_poll_exit(void);

// Stops the emulator thread and tears down the session.
void wipi_stop(void);

// Extracts the cover icon (PNG bytes) from a game package (zip/jar) into `out`.
// Returns the actual PNG length; 0 means no icon or buffer too small.
// Pass out=NULL to query the required size.
size_t wipi_game_icon(const uint8_t *game_data, size_t game_data_len, uint8_t *out, size_t out_cap);

// Extracts the game name (__adf__ Name, EUC-KR raw bytes) into `out`.
// Returns the actual length; 0 means no name or buffer too small.
// The host must decode EUC-KR (CP949).
size_t wipi_game_name(const uint8_t *game_data, size_t game_data_len, uint8_t *out, size_t out_cap);

#ifdef __cplusplus
}
#endif

#endif // WIPI_IOS_H
