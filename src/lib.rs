use nih_plug::prelude::*;
use std::sync::Arc;
use parking_lot::Mutex;

mod engine;
mod midi;
mod params;
mod editor;

use engine::SynthEngine;
use params::TaiyangParams;

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct SoundfontEntry {
    pub path: String,
    pub name: String,
    pub enabled: bool,
}

pub struct Taiyang {
    params: Arc<TaiyangParams>,
    engine: Arc<Mutex<Option<SynthEngine>>>,
    pipeline: Pipeline,
    last_bank: u8,
    last_program: u8,
    last_is_drum: bool,
    was_playing: bool,
}

struct Pipeline {
    interleaved: Vec<f32>,
}

impl Pipeline {
    fn new() -> Self {
        Self { interleaved: Vec::new() }
    }

    fn with_capacity(max_frames: usize) -> Self {
        Self {
            interleaved: vec![0.0f32; max_frames * 2],
        }
    }

    fn render(&mut self, buffer: &mut Buffer, engine: &mut SynthEngine, params: &TaiyangParams) {
        let num_frames = buffer.samples();
        let slice = &mut self.interleaved[..num_frames * 2];

        engine.read_samples(slice);
        let gain_db = params.gain.smoothed.next();
        let gain = util::db_to_gain(gain_db);

        for (i, mut channel_samples) in buffer.iter_samples().enumerate() {
            let l = self.interleaved[i * 2] * gain;
            let r = self.interleaved[i * 2 + 1] * gain;

            let mut iter = channel_samples.iter_mut();
            *iter.next().unwrap() = l;
            *iter.next().unwrap() = r;
        }
    }
}

impl Default for Taiyang {
    fn default() -> Self {
        Self {
            params: Arc::new(TaiyangParams::default()),
            engine: Arc::new(Mutex::new(None)),
            pipeline: Pipeline::new(),
            last_bank: 0,
            last_program: 0,
            last_is_drum: false,
            was_playing: false,
        }
    }
}

impl Plugin for Taiyang {
    const NAME: &'static str = "Taiyang";
    const VENDOR: &'static str = "Jieneng";
    const URL: &'static str = "https://space.bilibili.com/433246974";
    const EMAIL: &'static str = "3347830431@qq.com";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: NonZeroU32::new(2),
        aux_input_ports: &[],
        aux_output_ports: &[],
        names: PortNames::const_default(),
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::None;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        let max_voices = self.params.max_voices.value() as usize;
        let sample_rate = buffer_config.sample_rate;
        let mut engine = SynthEngine::new(sample_rate, max_voices);

        let entries = self.params.soundfont_entries.lock().clone();
        if !entries.is_empty() {
            let sf_entries: Vec<engine::SoundfontEntry> = entries.iter().map(|e| engine::SoundfontEntry {
                path: e.path.clone(),
                name: e.name.clone(),
                enabled: e.enabled,
            }).collect();

            if let Err(e) = engine.load_soundfonts(&sf_entries) {
                nih_log!("Soundfont loading failed: {}", e);
            } else {
                nih_log!("Loaded {} soundfonts", sf_entries.len());
            }
        }

        let is_drum = self.params.is_drum.value();
        engine.set_percussion_mode(is_drum);
        self.last_is_drum = is_drum;

        let bank = self.params.selected_bank.value() as u8;
        let program = self.params.selected_program.value() as u8;
        engine.send_preset(bank, program);
        self.last_bank = bank;
        self.last_program = program;

        *self.engine.lock() = Some(engine);

        let max_frames = buffer_config.max_buffer_size as usize;
        self.pipeline = Pipeline::with_capacity(max_frames);

        true
    }

    fn reset(&mut self) {
        if let Some(ref mut engine) = self.engine.lock().as_mut() {
            engine.all_notes_killed();
        }
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let mut engine_guard = self.engine.lock();

        if let Some(ref mut engine) = engine_guard.as_mut() {
            let transport = context.transport();
            let is_playing = transport.playing;

            // 1. DAW 停止 → 松开踏板 + 停止所有音符
            if !is_playing && self.was_playing {
                engine.all_notes_killed();
            }

            // 2. DAW 开始播放（从停止恢复）→ Reset + Chase
            if is_playing && !self.was_playing {
                engine.reset_and_chase();
            }

            let mut has_midi = false;
            let mut has_ch10 = false;

            while let Some(event) = context.next_event() {
                has_midi = true;
                if midi::is_channel_10(&event) {
                    has_ch10 = true;
                }
                midi::handle_note_event(event, engine, &self.params);
            }

            let current_bank = self.params.selected_bank.value() as u8;
            let current_program = self.params.selected_program.value() as u8;
            if current_bank != self.last_bank || current_program != self.last_program {
                engine.send_preset(current_bank, current_program);
                self.last_bank = current_bank;
                self.last_program = current_program;
            }

            let force_drum = self.params.is_drum.value();
            let desired_drum = if force_drum {
                true
            } else if has_midi {
                has_ch10
            } else {
                self.last_is_drum
            };

            if desired_drum != self.last_is_drum {
                engine.set_percussion_mode(desired_drum);
                self.last_is_drum = desired_drum;
            }

            self.was_playing = is_playing;
            self.pipeline.render(buffer, engine, &self.params);
        }

        ProcessStatus::Normal
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(
            self.params.clone(),
            self.engine.clone(),
        )
    }
}

impl Vst3Plugin for Taiyang {
    const VST3_CLASS_ID: [u8; 16] = *b"TaiyangVSTi00000";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
        Vst3SubCategory::Instrument,
        Vst3SubCategory::Synth,
    ];
}

nih_export_vst3!(Taiyang);
