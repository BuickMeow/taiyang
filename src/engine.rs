use xsynth_core::{
    AudioPipe, AudioStreamParams,
    channel::{ChannelAudioEvent, ChannelEvent, ChannelConfigEvent, ChannelInitOptions, ControlEvent},
    channel_group::{ChannelGroup, ChannelGroupConfig, ParallelismOptions, SynthEvent, SynthFormat},
    soundfont::{SampleSoundfont, SoundfontInitOptions, SoundfontBase},
};
use std::sync::{Arc, LazyLock};
use std::collections::HashMap;
use parking_lot::RwLock;
use nih_plug::prelude::nih_log;

#[derive(Clone, Debug)]
pub struct PresetInfo {
    pub name: String,
    pub bank: u16,
    pub program: u16,
    pub source_file: String,
}

#[derive(Clone, Debug)]
pub struct SoundfontEntry {
    pub path: String,
    pub name: String,
    pub enabled: bool,
}

/// 全局音色库缓存，所有 Taiyang 实例共享
/// Key: (文件路径, 采样率) —— Soundfont 与采样率绑定，不同采样率不能复用
static GLOBAL_SF_CACHE: LazyLock<RwLock<HashMap<(String, u32), Arc<dyn SoundfontBase>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// 需要 Chase 的 CC 列表（按发送顺序排列，RPN 相关必须先发）
const CHASE_CC_LIST: &[u8] = &[
    101, // RPN MSB
    100, // RPN LSB
    6,   // Data Entry MSB
    38,  // Data Entry LSB
    0,   // Bank Select MSB
    32,  // Bank Select LSB
    7,   // Volume
    10,  // Pan
    11,  // Expression
    64,  // Sustain
    73,  // Attack
    72,  // Release
    74,  // Brightness/Cutoff
    71,  // Resonance
];

pub struct SynthEngine {
    core: ChannelGroup,
    sample_rate: f32,
    presets: Vec<PresetInfo>,
    cc_state: [Option<u8>; 128],
    pb_state: Option<f32>,
}

impl SynthEngine {
    pub fn new(sample_rate: f32, _max_voices: usize) -> Self {
        let audio_params = AudioStreamParams {
            sample_rate: sample_rate as u32,
            channels: xsynth_core::ChannelCount::Stereo,
        };

        let config = ChannelGroupConfig {
            channel_init_options: ChannelInitOptions { fade_out_killing: true },
            format: SynthFormat::Custom { channels: 1 },
            audio_params,
            parallelism: ParallelismOptions::AUTO_PER_CHANNEL,
        };

        let core = ChannelGroup::new(config);

        Self {
            core,
            sample_rate,
            presets: Vec::new(),
            cc_state: [None; 128],
            pb_state: None,
        }
    }

    /// 记录 CC 最新值（用于 Chase）
    pub fn update_cc(&mut self, cc: u8, value: u8) {
        self.cc_state[cc as usize] = Some(value);
    }

    /// 记录 Pitch Bend 最新值（用于 Chase）
    pub fn update_pb(&mut self, value: f32) {
        self.pb_state = Some(value);
    }

    /// 播放开始时：Reset All Controllers + Chase 所有记录的 CC + PB
    pub fn reset_and_chase(&mut self) {
        // 1. Kill 所有音符（防止卡音）
        self.core.send_event(SynthEvent::Channel(
            0,
            ChannelEvent::Audio(ChannelAudioEvent::AllNotesKilled),
        ));

        // 2. Reset All Controllers
        self.core.send_event(SynthEvent::Channel(
            0,
            ChannelEvent::Audio(ChannelAudioEvent::ResetControl),
        ));

        // 3. Chase：按顺序重新发送所有记录过的 CC
        for &cc_num in CHASE_CC_LIST {
            if let Some(value) = self.cc_state[cc_num as usize] {
                self.core.send_event(SynthEvent::Channel(
                    0,
                    ChannelEvent::Audio(ChannelAudioEvent::Control(
                        ControlEvent::Raw(cc_num, value)
                    )),
                ));
            }
        }

        // 4. Chase Pitch Bend
        if let Some(pb) = self.pb_state {
            self.core.send_event(SynthEvent::Channel(
                0,
                ChannelEvent::Audio(ChannelAudioEvent::Control(
                    ControlEvent::PitchBendValue(pb)
                )),
            ));
        }
    }

    pub fn load_soundfonts(&mut self, entries: &[SoundfontEntry]) -> Result<(), String> {
        let mut soundfonts: Vec<Arc<dyn SoundfontBase>> = Vec::new();
        let mut all_presets: Vec<PresetInfo> = Vec::new();

        for entry in entries {
            if !entry.enabled {
                continue;
            }

            let cache_key = (entry.path.clone(), self.sample_rate as u32);

            let sf = if let Some(sf) = GLOBAL_SF_CACHE.read().get(&cache_key) {
                sf.clone()
            } else {
                match SampleSoundfont::new(
                    &entry.path,
                    self.core.stream_params().clone(),
                    SoundfontInitOptions::default(),
                ) {
                    Ok(sf) => {
                        let arc = Arc::new(sf) as Arc<dyn SoundfontBase>;
                        GLOBAL_SF_CACHE.write().insert(cache_key, arc.clone());
                        nih_log!("Loaded soundfont into global cache: {}", entry.path);
                        arc
                    }
                    Err(e) => {
                        nih_log!("Failed to load {}: {:?}", entry.path, e);
                        continue;
                    }
                }
            };

            soundfonts.push(sf);

            if entry.path.ends_with(".sf2") || entry.path.ends_with(".SF2") {
                if let Ok(presets) = xsynth_soundfonts::sf2::load_soundfont(&entry.path, self.sample_rate as u32) {
                    for p in presets {
                        all_presets.push(PresetInfo {
                            name: format!("Bank {} Prog {}", p.bank, p.preset),
                            bank: p.bank,
                            program: p.preset,
                            source_file: entry.name.clone(),
                        });
                    }
                }
            }
        }

        self.core.send_event(SynthEvent::Channel(
            0,
            ChannelEvent::Config(ChannelConfigEvent::SetSoundfonts(soundfonts))
        ));

        all_presets.sort_by(|a, b| {
            a.bank.cmp(&b.bank)
                .then_with(|| a.program.cmp(&b.program))
        });

        self.presets = all_presets;
        Ok(())
    }

    pub fn set_percussion_mode(&mut self, percussion: bool) {
        self.core.send_event(SynthEvent::Channel(
            0,
            ChannelEvent::Config(ChannelConfigEvent::SetPercussionMode(percussion)),
        ));
    }

    pub fn send_event(&mut self, event: SynthEvent) {
        self.core.send_event(event);
    }

    pub fn set_pitch_bend_range(&mut self, semitones: u8) {
        // RPN 0x0000: Pitch Bend Range
        // XSynth: pitch_bend_sensitivity = MSB + LSB/100.0
        self.core.send_event(SynthEvent::Channel(
            0,
            ChannelEvent::Audio(ChannelAudioEvent::Control(ControlEvent::Raw(101, 0)))),
        );
        self.core.send_event(SynthEvent::Channel(
            0,
            ChannelEvent::Audio(ChannelAudioEvent::Control(ControlEvent::Raw(100, 0)))),
        );
        self.core.send_event(SynthEvent::Channel(
            0,
            ChannelEvent::Audio(ChannelAudioEvent::Control(ControlEvent::Raw(6, semitones)))),
        );
        self.core.send_event(SynthEvent::Channel(
            0,
            ChannelEvent::Audio(ChannelAudioEvent::Control(ControlEvent::Raw(38, 0)))),
        );
    }

    pub fn set_fine_tune(&mut self, cents: i32) {
        // RPN 0x0001: Fine Tune
        // XSynth: val = MSB<<6 + LSB, (val - 4096) / 4096 * 100
        let val = (cents + 4096) as u16;
        let msb = (val >> 6) as u8;
        let lsb = (val & 0x3F) as u8;
        self.core.send_event(SynthEvent::Channel(
            0,
            ChannelEvent::Audio(ChannelAudioEvent::Control(ControlEvent::Raw(101, 0)))),
        );
        self.core.send_event(SynthEvent::Channel(
            0,
            ChannelEvent::Audio(ChannelAudioEvent::Control(ControlEvent::Raw(100, 1)))),
        );
        self.core.send_event(SynthEvent::Channel(
            0,
            ChannelEvent::Audio(ChannelAudioEvent::Control(ControlEvent::Raw(6, msb)))),
        );
        self.core.send_event(SynthEvent::Channel(
            0,
            ChannelEvent::Audio(ChannelAudioEvent::Control(ControlEvent::Raw(38, lsb)))),
        );
    }

    pub fn set_coarse_tune(&mut self, semitones: i32) {
        // RPN 0x0002: Coarse Tune
        // XSynth: coarse_tune = value - 64.0
        let value = (semitones + 64) as u8;
        self.core.send_event(SynthEvent::Channel(
            0,
            ChannelEvent::Audio(ChannelAudioEvent::Control(ControlEvent::Raw(101, 0)))),
        );
        self.core.send_event(SynthEvent::Channel(
            0,
            ChannelEvent::Audio(ChannelAudioEvent::Control(ControlEvent::Raw(100, 2)))),
        );
        self.core.send_event(SynthEvent::Channel(
            0,
            ChannelEvent::Audio(ChannelAudioEvent::Control(ControlEvent::Raw(6, value)))),
        );
        self.core.send_event(SynthEvent::Channel(
            0,
            ChannelEvent::Audio(ChannelAudioEvent::Control(ControlEvent::Raw(38, 0)))),
        );
    }

    pub fn send_preset(&mut self, bank: u8, program: u8) {
        self.core.send_event(SynthEvent::Channel(
            0,
            ChannelEvent::Audio(ChannelAudioEvent::Control(
                ControlEvent::Raw(0, bank)
            )),
        ));
        self.core.send_event(SynthEvent::Channel(
            0,
            ChannelEvent::Audio(ChannelAudioEvent::ProgramChange(program)),
        ));
    }

    pub fn all_notes_off(&mut self) {
        self.core.send_event(SynthEvent::Channel(
            0,
            ChannelEvent::Audio(ChannelAudioEvent::AllNotesOff),
        ));
    }

    pub fn all_notes_killed(&mut self) {
        self.core.send_event(SynthEvent::Channel(
            0,
            ChannelEvent::Audio(ChannelAudioEvent::AllNotesKilled),
        ));
    }

    pub fn read_samples(&mut self, buffer: &mut [f32]) {
        self.core.read_samples(buffer);
    }

    pub fn presets(&self) -> &[PresetInfo] {
        &self.presets
    }

    pub fn active_voices(&self) -> u64 {
        self.core.voice_count()
    }
}
