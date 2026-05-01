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
    last_pbr: f32,
    last_fine_tune: f32,
    last_coarse_tune: f32,
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
            last_pbr: -1.0,
            last_fine_tune: f32::NAN,
            last_coarse_tune: f32::NAN,
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
        let sample_rate = buffer_config.sample_rate;
        let mut engine = SynthEngine::new(sample_rate, 0);

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

        // 初始化 RPN 参数
        let pbr = self.params.pitch_bend_range.value();
        engine.set_pitch_bend_range(pbr);
        self.last_pbr = pbr;

        let fine = self.params.master_fine_tune.value();
        engine.set_fine_tune(fine);
        self.last_fine_tune = fine;

        let coarse = self.params.master_coarse_tune.value();
        engine.set_coarse_tune(coarse);
        self.last_coarse_tune = coarse;

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
            while let Some(event) = context.next_event() {
                midi::handle_note_event(event, engine, self.params.preset_locked.value());
            }

            let current_bank = self.params.selected_bank.value() as u8;
            let current_program = self.params.selected_program.value() as u8;
            if current_bank != self.last_bank || current_program != self.last_program {
                engine.send_preset(current_bank, current_program);
                self.last_bank = current_bank;
                self.last_program = current_program;
            }

            let current_is_drum = self.params.is_drum.value();
            if current_is_drum != self.last_is_drum {
                engine.set_percussion_mode(current_is_drum);
                self.last_is_drum = current_is_drum;
            }

            // RPN 参数变化检测
            let current_pbr = self.params.pitch_bend_range.value();
            if current_pbr != self.last_pbr {
                engine.set_pitch_bend_range(current_pbr);
                self.last_pbr = current_pbr;
            }

            let current_fine = self.params.master_fine_tune.value();
            if current_fine != self.last_fine_tune {
                engine.set_fine_tune(current_fine);
                self.last_fine_tune = current_fine;
            }

            let current_coarse = self.params.master_coarse_tune.value();
            if current_coarse != self.last_coarse_tune {
                engine.set_coarse_tune(current_coarse);
                self.last_coarse_tune = current_coarse;
            }

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

impl ClapPlugin for Taiyang {
    const CLAP_ID: &'static str = "com.jieneng.taiyang";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("SoundFont Synthesizer based on XSynth");
    const CLAP_MANUAL_URL: Option<&'static str> = Some("https://space.bilibili.com/433246974");
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Synthesizer,
    ];
}

nih_export_clap!(Taiyang);
