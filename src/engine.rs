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

// 只 Chase 最关键的 CC，避免和 DAW 自己的 Chase 冲突导致声音不稳定
// RPN/Bank Select/音色修改器 由 DAW 或 UI 参数控制，不在此处 Chase
// Pitch Bend 是 pianoroll 曲线，Chase 固定值没有意义
const CHASE_CC_LIST: &[u8] = &[
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
    cc_state: [[Option<u8>; 128]; 16],
    pb_state: [Option<f32>; 16],
    last_pbr: Option<u8>,
    last_fine_tune: Option<i32>,
    last_coarse_tune: Option<i32>,
    vol_state: [u8; 16],
    pb_range: u8,
}

impl SynthEngine {
    pub fn new(sample_rate: f32, _max_voices: usize) -> Self {
        let audio_params = AudioStreamParams {
            sample_rate: sample_rate as u32,
            channels: xsynth_core::ChannelCount::Stereo,
        };

        let config = ChannelGroupConfig {
            channel_init_options: ChannelInitOptions { fade_out_killing: true },
            format: SynthFormat::Midi,
            audio_params,
            parallelism: ParallelismOptions::AUTO_PER_CHANNEL,
        };

        let core = ChannelGroup::new(config);

        Self {
            core,
            sample_rate,
            presets: Vec::new(),
            cc_state: [[None; 128]; 16],
            pb_state: [None; 16],
            last_pbr: None,
            last_fine_tune: None,
            last_coarse_tune: None,
            vol_state: [127; 16],
            pb_range: 2,
        }
    }

    /// 记录 CC 最新值（按通道，用于 Chase）
    pub fn update_cc(&mut self, channel: u32, cc: u8, value: u8) {
        if let Some(ch_state) = self.cc_state.get_mut(channel as usize) {
            ch_state[cc as usize] = Some(value);
        }
    }

    /// 记录 Pitch Bend 最新值（按通道，用于 Chase）
    pub fn update_pb(&mut self, channel: u32, value: f32) {
        if let Some(ch_state) = self.pb_state.get_mut(channel as usize) {
            *ch_state = Some(value);
        }
    }

    /// 弯音音量补偿：音高高时自动压低音量，音高低时抬高音量
    /// 补偿曲线 -3dB/octave，通过修改 Volume(CC7) 实现
    fn compensated_vol(&self, channel: u32, pb_value: f32) -> Option<u8> {
        let user_vol = *self.vol_state.get(channel as usize)?;
        let semitone_shift = pb_value * self.pb_range as f32;
        // -3dB/octave: compensate_amp = 10^(-3 * semitone_shift / 20 / 12) = 10^(-semitone_shift / 80)
        let compensate_amp = 10.0f32.powf(-semitone_shift / 80.0);
        let vol = (user_vol as f32 / 127.0 * compensate_amp).clamp(0.0, 1.0);
        Some((vol * 127.0) as u8)
    }

    pub fn apply_pb_volume_comp(&mut self, channel: u32, pb_value: f32) {
        if let Some(vol_val) = self.compensated_vol(channel, pb_value) {
            self.core.send_event(SynthEvent::Channel(
                channel,
                ChannelEvent::Audio(ChannelAudioEvent::Control(
                    ControlEvent::Raw(7, vol_val)
                )),
            ));
        }
    }

    pub fn update_vol_raw_and_compensate(&mut self, channel: u32, value: u8) {
        if let Some(ch_state) = self.vol_state.get_mut(channel as usize) {
            *ch_state = value;
        }
        let pb = self.pb_state.get(channel as usize).copied().flatten().unwrap_or(0.0);
        if let Some(vol_val) = self.compensated_vol(channel, pb) {
            self.core.send_event(SynthEvent::Channel(
                channel,
                ChannelEvent::Audio(ChannelAudioEvent::Control(
                    ControlEvent::Raw(7, vol_val)
                )),
            ));
        }
    }

    pub fn reset_and_chase(&mut self) {
        for ch in 0..16u32 {
            self.core.send_event(SynthEvent::Channel(
                ch,
                ChannelEvent::Audio(ChannelAudioEvent::AllNotesKilled),
            ));

            self.core.send_event(SynthEvent::Channel(
                ch,
                ChannelEvent::Audio(ChannelAudioEvent::ResetControl),
            ));

            if let Some(ch_state) = self.cc_state.get(ch as usize) {
                for &cc_num in CHASE_CC_LIST {
                    if let Some(value) = ch_state[cc_num as usize] {
                        let val = if cc_num == 7 {
                            // Volume: 发送补偿后的值
                            let pb = self.pb_state.get(ch as usize).copied().flatten().unwrap_or(0.0);
                            self.compensated_vol(ch, pb).unwrap_or(value)
                        } else {
                            value
                        };
                        self.core.send_event(SynthEvent::Channel(
                            ch,
                            ChannelEvent::Audio(ChannelAudioEvent::Control(
                                ControlEvent::Raw(cc_num, val)
                            )),
                        ));
                    }
                }
            }
        }

        // Chase RPN 语义化参数（PBR/Fine/Coarse 走 XSynth 专用事件，不依赖 CC101/100）
        if let Some(pbr) = self.last_pbr {
            for ch in 0..16u32 {
                self.core.send_event(SynthEvent::Channel(ch, ChannelEvent::Audio(
                    ChannelAudioEvent::Control(ControlEvent::PitchBendSensitivity(pbr as f32))
                )));
            }
        }
        if let Some(fine) = self.last_fine_tune {
            for ch in 0..16u32 {
                self.core.send_event(SynthEvent::Channel(ch, ChannelEvent::Audio(
                    ChannelAudioEvent::Control(ControlEvent::FineTune(fine as f32))
                )));
            }
        }
        if let Some(coarse) = self.last_coarse_tune {
            for ch in 0..16u32 {
                self.core.send_event(SynthEvent::Channel(ch, ChannelEvent::Audio(
                    ChannelAudioEvent::Control(ControlEvent::CoarseTune(coarse as f32))
                )));
            }
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

        for ch in 0..16u32 {
            self.core.send_event(SynthEvent::Channel(
                ch,
                ChannelEvent::Config(ChannelConfigEvent::SetSoundfonts(soundfonts.clone()))
            ));
        }

        all_presets.sort_by(|a, b| {
            a.bank.cmp(&b.bank)
                .then_with(|| a.program.cmp(&b.program))
        });

        self.presets = all_presets;
        Ok(())
    }

    pub fn set_percussion_mode(&mut self, ch: u32, percussion: bool) {
        self.core.send_event(SynthEvent::Channel(
            ch,
            ChannelEvent::Config(ChannelConfigEvent::SetPercussionMode(percussion)),
        ));
    }

    pub fn set_percussion_mode_all(&mut self, percussion: bool) {
        for ch in 0..16u32 {
            self.core.send_event(SynthEvent::Channel(
                ch,
                ChannelEvent::Config(ChannelConfigEvent::SetPercussionMode(percussion)),
            ));
        }
    }

    pub fn send_event(&mut self, event: SynthEvent) {
        self.core.send_event(event);
    }

    pub fn set_pitch_bend_range_all(&mut self, semitones: u8) {
        self.last_pbr = Some(semitones);
        self.pb_range = semitones;
        for ch in 0..16u32 {
            self.core.send_event(SynthEvent::Channel(
                ch,
                ChannelEvent::Audio(ChannelAudioEvent::Control(
                    ControlEvent::PitchBendSensitivity(semitones as f32)
                )),
            ));
        }
    }

    pub fn set_fine_tune_all(&mut self, cents: i32) {
        self.last_fine_tune = Some(cents);
        for ch in 0..16u32 {
            self.core.send_event(SynthEvent::Channel(
                ch,
                ChannelEvent::Audio(ChannelAudioEvent::Control(
                    ControlEvent::FineTune(cents as f32)
                )),
            ));
        }
    }

    pub fn set_coarse_tune_all(&mut self, semitones: i32) {
        self.last_coarse_tune = Some(semitones);
        for ch in 0..16u32 {
            self.core.send_event(SynthEvent::Channel(
                ch,
                ChannelEvent::Audio(ChannelAudioEvent::Control(
                    ControlEvent::CoarseTune(semitones as f32)
                )),
            ));
        }
    }

    pub fn send_preset(&mut self, ch: u32, bank: u8, program: u8) {
        self.core.send_event(SynthEvent::Channel(
            ch,
            ChannelEvent::Audio(ChannelAudioEvent::Control(
                ControlEvent::Raw(0, bank)
            )),
        ));
        self.core.send_event(SynthEvent::Channel(
            ch,
            ChannelEvent::Audio(ChannelAudioEvent::ProgramChange(program)),
        ));
    }

    pub fn send_preset_all(&mut self, bank: u8, program: u8) {
        for ch in 0..16u32 {
            self.send_preset(ch, bank, program);
        }
    }

    pub fn all_notes_off(&mut self, ch: u32) {
        self.core.send_event(SynthEvent::Channel(
            ch,
            ChannelEvent::Audio(ChannelAudioEvent::AllNotesOff),
        ));
    }

    pub fn all_notes_killed(&mut self) {
        for ch in 0..16u32 {
            self.core.send_event(SynthEvent::Channel(
                ch,
                ChannelEvent::Audio(ChannelAudioEvent::AllNotesKilled),
            ));
        }
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
