//! PCM은 rodio(AAudio), MIDI는 rustysynth 소프트신스로 재생.
//! 코어는 파싱된 SMAF 타임라인(AudioSequence)을 AudioCommand::Play/Stop으로
//! 넘기고, 전용 오디오 스레드가 이벤트 시각에 맞춰 신스/PCM에 전달한다
//! (wie 데스크톱 프론트엔드의 스케줄러 구조를 rustysynth로 이식).

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    num::NonZero,
    path::Path,
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, RecvTimeoutError, Sender, channel},
    },
    time::{Duration, Instant},
};

use rodio::{DeviceSinkBuilder, Player, Source, buffer::SamplesBuffer, conversions::SampleTypeConverter};
use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};
use wie_backend::{AudioCommand, AudioEventData, AudioHandle, AudioSequence, TimedAudioEvent};

const SYNTH_SAMPLE_RATE: u32 = 44100;
/// 한 번에 렌더링하는 프레임 수 (~12ms @ 44.1kHz)
const SYNTH_CHUNK_FRAMES: usize = 512;

/// 오디오 스레드로 보내는 명령. 코어의 시퀀스 재생 외에 호스트 설정(볼륨)도 이 채널로 전달한다.
enum AppAudioCommand {
    Backend(AudioCommand),
    /// 호스트 볼륨 (0.0~1.0). PCM(효과음)과 MIDI(배경음악)를 분리 조절 —
    /// 사운드폰트 음량과 게임 내장 샘플 음량이 게임마다 달라 밸런스 보정이 필요 (웹버전과 동일).
    SetVolume {
        pcm: f32,
        midi: f32,
    },
}

pub struct AudioEngine {
    tx: Sender<AppAudioCommand>,
}

impl AudioEngine {
    pub fn new(soundfont_path: Option<&Path>) -> Self {
        tracing::info!("AudioEngine::new(soundfont: {soundfont_path:?})");

        let synth = soundfont_path.and_then(|path| match Self::create_synthesizer(path) {
            Ok(synth) => {
                tracing::info!("soundfont loaded");
                Some(Arc::new(Mutex::new(synth)))
            }
            Err(e) => {
                tracing::warn!("Failed to load soundfont {path:?}: {e} - MIDI will be silent");
                None
            }
        });

        let (tx, rx) = channel();
        std::thread::Builder::new()
            .name("wie-audio".into())
            .spawn(move || Self::audio_thread(rx, synth))
            .unwrap();

        Self { tx }
    }

    fn create_synthesizer(path: &Path) -> anyhow::Result<Synthesizer> {
        let mut file = File::open(path)?;
        let sound_font = Arc::new(SoundFont::new(&mut file)?);
        let settings = SynthesizerSettings::new(SYNTH_SAMPLE_RATE as i32);

        Ok(Synthesizer::new(&sound_font, &settings)?)
    }

    fn audio_thread(rx: Receiver<AppAudioCommand>, synth: Option<Arc<Mutex<Synthesizer>>>) {
        let output = match DeviceSinkBuilder::open_default_sink() {
            Ok(x) => x,
            Err(e) => {
                tracing::warn!("Failed to open audio output: {e} - audio disabled");
                while rx.recv().is_ok() {}
                return;
            }
        };
        tracing::info!("audio output opened");

        let pcm_player = Player::connect_new(output.mixer());

        // 신스는 무한 소스로 믹서에 상시 연결
        let midi_player = synth.clone().map(|synth| {
            let player = Player::connect_new(output.mixer());
            player.append(SynthSource::new(synth));
            player
        });

        let mut playbacks: BTreeMap<AudioHandle, Playback> = BTreeMap::new();

        loop {
            let command = if let Some(deadline) = playbacks.values().map(Playback::next_deadline).min() {
                match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                    Ok(command) => Some(command),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            } else {
                match rx.recv() {
                    Ok(command) => Some(command),
                    Err(_) => break,
                }
            };

            if let Some(command) = command {
                match command {
                    AppAudioCommand::Backend(AudioCommand::Play { handle, sequence, repeat }) => {
                        if let Some(mut playback) = playbacks.remove(&handle) {
                            cleanup(&synth, &mut playback);
                        }
                        playbacks.insert(handle, Playback::new(sequence, repeat));
                    }
                    AppAudioCommand::Backend(AudioCommand::Stop { handle }) => {
                        if let Some(mut playback) = playbacks.remove(&handle) {
                            cleanup(&synth, &mut playback);
                        }
                    }
                    AppAudioCommand::SetVolume { pcm, midi } => {
                        pcm_player.set_volume(pcm.clamp(0.0, 1.0));
                        if let Some(midi_player) = &midi_player {
                            midi_player.set_volume(midi.clamp(0.0, 1.0));
                        }
                    }
                }
                continue;
            }

            let now = Instant::now();
            let handles: Vec<AudioHandle> = playbacks.keys().copied().collect();
            for handle in handles {
                let playback = playbacks.get_mut(&handle).unwrap();

                while let Some(event) = playback.sequence.events.get(playback.next_event) {
                    if playback.started_at + Duration::from_millis(event.time) > now {
                        break;
                    }

                    play_event(&synth, &pcm_player, event, &mut playback.active_notes, &mut playback.used_channels);
                    playback.next_event += 1;
                }

                if playback.next_event == playback.sequence.events.len()
                    && playback.started_at + Duration::from_millis(playback.sequence.duration) <= now
                {
                    cleanup(&synth, playback);

                    if playback.repeat && playback.sequence.duration != 0 {
                        playback.started_at = now;
                        playback.next_event = 0;
                    } else {
                        playbacks.remove(&handle);
                    }
                }
            }
        }
    }

    /// 볼륨 설정 (0.0~1.0, 0이면 음소거) — PCM(효과음)/MIDI(배경음악) 분리
    pub fn set_volume(&self, pcm: f32, midi: f32) {
        let _ = self.tx.send(AppAudioCommand::SetVolume { pcm, midi });
    }

    pub fn sink(&self) -> AudioSink {
        AudioSink { tx: self.tx.clone() }
    }
}

struct Playback {
    sequence: Arc<AudioSequence>,
    repeat: bool,
    started_at: Instant,
    next_event: usize,
    active_notes: BTreeSet<(u8, u8)>,
    used_channels: BTreeSet<u8>,
}

impl Playback {
    fn new(sequence: Arc<AudioSequence>, repeat: bool) -> Self {
        Self {
            sequence,
            repeat,
            started_at: Instant::now(),
            next_event: 0,
            active_notes: BTreeSet::new(),
            used_channels: BTreeSet::new(),
        }
    }

    fn next_deadline(&self) -> Instant {
        let time = self
            .sequence
            .events
            .get(self.next_event)
            .map_or(self.sequence.duration, |event| event.time);
        self.started_at + Duration::from_millis(time)
    }
}

fn play_event(
    synth: &Option<Arc<Mutex<Synthesizer>>>,
    pcm_player: &Player,
    event: &TimedAudioEvent,
    active_notes: &mut BTreeSet<(u8, u8)>,
    used_channels: &mut BTreeSet<u8>,
) {
    match &event.data {
        AudioEventData::Midi(data) => {
            let Some(status) = data.first().copied() else {
                return;
            };

            if (0x80..0xf0).contains(&status) {
                let channel = status & 0x0f;
                used_channels.insert(channel);

                if let Some(note) = data.get(1).copied() {
                    match status & 0xf0 {
                        0x80 => {
                            active_notes.remove(&(channel, note));
                        }
                        0x90 if data.get(2).copied().unwrap_or(0) == 0 => {
                            active_notes.remove(&(channel, note));
                        }
                        0x90 => {
                            active_notes.insert((channel, note));
                        }
                        _ => {}
                    }
                }

                if let Some(synth) = synth {
                    let data1 = data.get(1).copied().unwrap_or(0) as i32;
                    let data2 = data.get(2).copied().unwrap_or(0) as i32;
                    synth
                        .lock()
                        .unwrap()
                        .process_midi_message(channel as i32, (status & 0xf0) as i32, data1, data2);
                }
            }
            // sysex 등 0xf0 이상은 소프트신스에서 무시
        }
        AudioEventData::Wave {
            channels,
            sampling_rate,
            samples,
        } => {
            let (Some(channels), Some(sampling_rate)) = (NonZero::new((*channels).into()), NonZero::new(*sampling_rate)) else {
                return;
            };

            // TODO 다중 PCM 동시 재생 (wie 데스크톱과 동일한 한계)
            pcm_player.append(SamplesBuffer::new(
                channels,
                sampling_rate,
                SampleTypeConverter::new(samples.iter().copied()).collect::<Vec<_>>(),
            ));
        }
    }
}

fn cleanup(synth: &Option<Arc<Mutex<Synthesizer>>>, playback: &mut Playback) {
    if let Some(synth) = synth {
        let mut synth = synth.lock().unwrap();
        for (channel, note) in &playback.active_notes {
            synth.note_off(*channel as i32, *note as i32);
        }
        for channel in &playback.used_channels {
            // sustain 해제 + all sound/notes off
            for control in [64, 120, 123] {
                synth.process_midi_message(*channel as i32, 0xb0, control as i32, 0);
            }
        }
    }

    playback.active_notes.clear();
    playback.used_channels.clear();
}

/// rustysynth 출력을 rodio 소스로 노출 (스테레오 인터리브, 무한)
struct SynthSource {
    synth: Arc<Mutex<Synthesizer>>,
    left: Vec<f32>,
    right: Vec<f32>,
    interleaved: Vec<f32>,
    pos: usize,
}

impl SynthSource {
    fn new(synth: Arc<Mutex<Synthesizer>>) -> Self {
        Self {
            synth,
            left: vec![0.0; SYNTH_CHUNK_FRAMES],
            right: vec![0.0; SYNTH_CHUNK_FRAMES],
            interleaved: Vec::new(),
            pos: 0,
        }
    }
}

impl Iterator for SynthSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.pos >= self.interleaved.len() {
            self.synth.lock().unwrap().render(&mut self.left, &mut self.right);

            self.interleaved.clear();
            for i in 0..SYNTH_CHUNK_FRAMES {
                self.interleaved.push(self.left[i]);
                self.interleaved.push(self.right[i]);
            }
            self.pos = 0;
        }

        let sample = self.interleaved[self.pos];
        self.pos += 1;
        Some(sample)
    }
}

impl Source for SynthSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> NonZero<u16> {
        NonZero::new(2).unwrap()
    }

    fn sample_rate(&self) -> NonZero<u32> {
        NonZero::new(SYNTH_SAMPLE_RATE).unwrap()
    }

    fn total_duration(&self) -> Option<core::time::Duration> {
        None
    }
}

pub struct AudioSink {
    tx: Sender<AppAudioCommand>,
}

impl wie_backend::AudioSink for AudioSink {
    fn send(&self, command: AudioCommand) {
        if self.tx.send(AppAudioCommand::Backend(command)).is_err() {
            tracing::warn!("Audio worker is unavailable");
        }
    }
}
