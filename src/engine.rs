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

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct SoundfontEntry {
    pub path: String,
    pub name: String,
    pub enabled: bool,
}

/// 全局音色库缓存，所有 Taiyang 实例共享
/// Key: (文件路径, 采样率) —— Soundfont 与采样率绑定，不同采样率不能复用
static GLOBAL_SF_CACHE: LazyLock<RwLock<HashMap<(String, u32), Arc<dyn SoundfontBase>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

pub struct SynthEngine {
    core: ChannelGroup,
    sample_rate: f32,
    presets: Vec<PresetInfo>,
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
            parallelism: ParallelismOptions::AUTO_PER_KEY,
        };

        let core = ChannelGroup::new(config);

        Self {
            core,
            sample_rate,
            presets: Vec::new(),
        }
    }

    pub fn send_event(&mut self, event: SynthEvent) {
        self.core.send_event(event);
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

    pub fn set_pitch_bend_range(&mut self, semitones: f32) {
        self.core.send_event(SynthEvent::Channel(
            0,
            ChannelEvent::Audio(ChannelAudioEvent::Control(
                ControlEvent::PitchBendSensitivity(semitones)
            )),
        ));
    }

    pub fn set_fine_tune(&mut self, cents: f32) {
        self.core.send_event(SynthEvent::Channel(
            0,
            ChannelEvent::Audio(ChannelAudioEvent::Control(
                ControlEvent::FineTune(cents)
            )),
        ));
    }

    pub fn set_coarse_tune(&mut self, semitones: f32) {
        self.core.send_event(SynthEvent::Channel(
            0,
            ChannelEvent::Audio(ChannelAudioEvent::Control(
                ControlEvent::CoarseTune(semitones)
            )),
        ));
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

    pub fn all_notes_killed(&mut self) {
        self.core.send_event(SynthEvent::Channel(
            0,
            ChannelEvent::Audio(ChannelAudioEvent::AllNotesKilled),
        ));
    }

    // === Filter setters ===

    fn send_control(&mut self, event: ControlEvent) {
        self.core.send_event(SynthEvent::Channel(
            0,
            ChannelEvent::Audio(ChannelAudioEvent::Control(event)),
        ));
    }

    pub fn set_cutoff(&mut self, freq: f32) {
        let value = if freq <= 0.0 {
            1.0 // 全截断（biquad 能接受的最低频率）
        } else if freq >= 20000.0 {
            self.sample_rate / 2.0 // 确保高于 threshold = 关
        } else {
            freq
        };
        self.send_control(ControlEvent::Cutoff(value));
    }

    pub fn set_resonance(&mut self, q: f32) {
        self.send_control(ControlEvent::Resonance(q.max(0.01)));
    }

    pub fn set_highpass_cutoff(&mut self, freq: f32) {
        let value = if freq <= 1.0 {
            0.0 // 确保低于 threshold = 关
        } else {
            freq
        };
        self.send_control(ControlEvent::HighPassCutoff(value));
    }

    pub fn set_highpass_resonance(&mut self, q: f32) {
        self.send_control(ControlEvent::HighPassResonance(q.max(0.01)));
    }

    // === Envelope setters (-1.0 = Auto/取消覆盖) ===

    pub fn set_env_delay(&mut self, seconds: f32) {
        self.send_control(ControlEvent::DelayTime(seconds));
    }

    pub fn set_env_attack(&mut self, seconds: f32) {
        self.send_control(ControlEvent::AttackTime(seconds));
    }

    pub fn set_env_hold(&mut self, seconds: f32) {
        self.send_control(ControlEvent::HoldTime(seconds));
    }

    pub fn set_env_decay(&mut self, seconds: f32) {
        self.send_control(ControlEvent::DecayTime(seconds));
    }

    pub fn set_env_sustain(&mut self, level: f32) {
        self.send_control(ControlEvent::SustainLevel(level));
    }

    pub fn set_env_release(&mut self, seconds: f32) {
        self.send_control(ControlEvent::ReleaseTime(seconds));
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
