use nih_plug::prelude::*;
use parking_lot::Mutex;
use std::sync::Arc;
use taiyang_shared::engine::SynthEngine;
use taiyang_shared::midi;

mod editor;
mod params;

use params::Taiyang16Params;

pub struct Taiyang16 {
    params: Arc<Taiyang16Params>,
    engine: Arc<Mutex<Option<SynthEngine>>>,
    pipeline: Pipeline,

    // Per-channel state (source of truth for UI)
    channel_bank: [u8; 16],
    channel_program: [u8; 16],
    channel_is_drum: [bool; 16],

    // Global filter/envelope tracking
    last_cutoff: f32,
    last_resonance: f32,
    last_hp_cutoff: f32,
    last_hp_resonance: f32,
    last_env_delay: f32,
    last_env_attack: f32,
    last_env_hold: f32,
    last_env_decay: f32,
    last_env_sustain: f32,
    last_env_release: f32,

    // Current selected channel (for detecting UI changes)
    last_selected_channel: u8,

    was_playing: bool,
}

struct Pipeline {
    interleaved: Vec<f32>,
}

impl Pipeline {
    fn new() -> Self {
        Self {
            interleaved: Vec::new(),
        }
    }

    fn with_capacity(max_frames: usize) -> Self {
        Self {
            interleaved: vec![0.0f32; max_frames * 2],
        }
    }

    fn render(&mut self, buffer: &mut Buffer, engine: &mut SynthEngine, params: &Taiyang16Params) {
        let num_frames = buffer.samples();
        let slice = &mut self.interleaved[..num_frames * 2];

        engine.read_samples(slice);
        let gain = params.gain.smoothed.next();

        let output = buffer.as_slice();
        let (left_out, rest) = output.split_at_mut(1);
        let left = &mut left_out[0][..num_frames];
        let right = &mut rest[0][..num_frames];

        for i in 0..num_frames {
            left[i] = self.interleaved[i * 2] * gain;
            right[i] = self.interleaved[i * 2 + 1] * gain;
        }
    }
}

impl Default for Taiyang16 {
    fn default() -> Self {
        let mut channel_is_drum = [false; 16];
        // MIDI convention: channel 9 (0-indexed) is percussion
        channel_is_drum[9] = true;

        Self {
            params: Arc::new(Taiyang16Params::default()),
            engine: Arc::new(Mutex::new(None)),
            pipeline: Pipeline::new(),
            channel_bank: [0u8; 16],
            channel_program: [0u8; 16],
            channel_is_drum,
            last_cutoff: 20000.0,
            last_resonance: 0.70710677,
            last_hp_cutoff: 0.0,
            last_hp_resonance: 0.70710677,
            last_env_delay: -1.0,
            last_env_attack: -1.0,
            last_env_hold: -1.0,
            last_env_decay: -1.0,
            last_env_sustain: -1.0,
            last_env_release: -1.0,
            last_selected_channel: 0,
            was_playing: false,
        }
    }
}

impl Plugin for Taiyang16 {
    const NAME: &'static str = "Taiyang16";
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
        let mut engine = SynthEngine::new(sample_rate, 16, 0);

        let entries = self.params.soundfont_entries.lock().clone();
        if !entries.is_empty() {
            if let Err(e) = engine.load_soundfonts(&entries) {
                nih_log!("Soundfont loading failed: {}", e);
            } else {
                nih_log!("Loaded {} soundfonts", entries.len());
            }
        }

        // Initialize all 16 channels
        for ch in 0u8..16 {
            // Default bank/program
            engine.send_preset(ch, 0, 0);

            // Set drum mode for channel 9 (MIDI standard)
            let is_drum = self.channel_is_drum[ch as usize];
            engine.set_percussion_mode(ch, is_drum);
        }

        // Global params — send to all channels
        let cutoff = self.params.cutoff.value();
        engine.set_cutoff_all(cutoff);

        let resonance = self.params.resonance.value();
        engine.set_resonance_all(resonance);

        let hp_cutoff = self.params.highpass_cutoff.value();
        engine.set_highpass_cutoff_all(hp_cutoff);

        let hp_resonance = self.params.highpass_resonance.value();
        engine.set_highpass_resonance_all(hp_resonance);

        let env_delay = self.params.env_delay.value();
        engine.set_env_delay_all(env_delay);

        let env_attack = self.params.env_attack.value();
        engine.set_env_attack_all(env_attack);

        let env_hold = self.params.env_hold.value();
        engine.set_env_hold_all(env_hold);

        let env_decay = self.params.env_decay.value();
        engine.set_env_decay_all(env_decay);

        let env_sustain = self.params.env_sustain.value();
        engine.set_env_sustain_all(env_sustain);

        let env_release = self.params.env_release.value();
        engine.set_env_release_all(env_release);

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
        let transport = context.transport();
        let is_playing = transport.playing;

        // Collect MIDI events
        let mut midi_events = Vec::with_capacity(16);
        while let Some(event) = context.next_event() {
            midi_events.push(event);
        }

        // Read params
        let preset_locked = self.params.preset_locked.value();
        let selected_channel = self.params.selected_channel.value() as u8;

        // Per-channel params (for the currently selected channel)
        let current_bank = self.params.selected_bank.value() as u8;
        let current_program = self.params.selected_program.value() as u8;
        let current_is_drum = self.params.is_drum.value();

        // Global params
        let current_cutoff = self.params.cutoff.value();
        let current_resonance = self.params.resonance.value();
        let current_hp_cutoff = self.params.highpass_cutoff.value();
        let current_hp_resonance = self.params.highpass_resonance.value();
        let current_env_delay = self.params.env_delay.value();
        let current_env_attack = self.params.env_attack.value();
        let current_env_hold = self.params.env_hold.value();
        let current_env_decay = self.params.env_decay.value();
        let current_env_sustain = self.params.env_sustain.value();
        let current_env_release = self.params.env_release.value();

        let mut engine_guard = self.engine.lock();
        if let Some(ref mut engine) = engine_guard.as_mut() {
            // Kill notes on transport restart
            if is_playing && !self.was_playing {
                engine.all_notes_killed();
            }
            self.was_playing = is_playing;

            // Route MIDI events to their respective channels (no override)
            for event in midi_events {
                midi::handle_note_event(event, engine, preset_locked, None);
            }

            // --- UI-initiated per-channel changes ---

            // If the selected channel changed, update per-channel params
            if selected_channel != self.last_selected_channel {
                // The editor should have updated the bank/program/drum params
                // to match the new channel. We track the change here.
                self.last_selected_channel = selected_channel;
            }

            let ch = selected_channel;
            if current_bank != self.channel_bank[ch as usize]
                || current_program != self.channel_program[ch as usize]
            {
                engine.send_preset(ch, current_bank, current_program);
                self.channel_bank[ch as usize] = current_bank;
                self.channel_program[ch as usize] = current_program;
            }

            if current_is_drum != self.channel_is_drum[ch as usize] {
                engine.set_percussion_mode(ch, current_is_drum);
                self.channel_is_drum[ch as usize] = current_is_drum;
            }

            // --- Global filter/envelope changes (apply to all channels) ---

            if current_cutoff != self.last_cutoff {
                engine.set_cutoff_all(current_cutoff);
                self.last_cutoff = current_cutoff;
            }

            if current_resonance != self.last_resonance {
                engine.set_resonance_all(current_resonance);
                self.last_resonance = current_resonance;
            }

            if current_hp_cutoff != self.last_hp_cutoff {
                engine.set_highpass_cutoff_all(current_hp_cutoff);
                self.last_hp_cutoff = current_hp_cutoff;
            }

            if current_hp_resonance != self.last_hp_resonance {
                engine.set_highpass_resonance_all(current_hp_resonance);
                self.last_hp_resonance = current_hp_resonance;
            }

            if current_env_delay != self.last_env_delay {
                engine.set_env_delay_all(current_env_delay);
                self.last_env_delay = current_env_delay;
            }

            if current_env_attack != self.last_env_attack {
                engine.set_env_attack_all(current_env_attack);
                self.last_env_attack = current_env_attack;
            }

            if current_env_hold != self.last_env_hold {
                engine.set_env_hold_all(current_env_hold);
                self.last_env_hold = current_env_hold;
            }

            if current_env_decay != self.last_env_decay {
                engine.set_env_decay_all(current_env_decay);
                self.last_env_decay = current_env_decay;
            }

            if current_env_sustain != self.last_env_sustain {
                engine.set_env_sustain_all(current_env_sustain);
                self.last_env_sustain = current_env_sustain;
            }

            if current_env_release != self.last_env_release {
                engine.set_env_release_all(current_env_release);
                self.last_env_release = current_env_release;
            }

            self.pipeline.render(buffer, engine, &self.params);
        }

        ProcessStatus::Normal
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(self.params.clone(), self.engine.clone())
    }
}

impl Vst3Plugin for Taiyang16 {
    const VST3_CLASS_ID: [u8; 16] = *b"Taiyang16VSTi000";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument, Vst3SubCategory::Synth];
}

nih_export_vst3!(Taiyang16);

impl ClapPlugin for Taiyang16 {
    const CLAP_ID: &'static str = "com.jieneng.taiyang16";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("16-Channel SoundFont Synthesizer based on XSynth");
    const CLAP_MANUAL_URL: Option<&'static str> = Some("https://space.bilibili.com/433246974");
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] =
        &[ClapFeature::Instrument, ClapFeature::Synthesizer];
}

nih_export_clap!(Taiyang16);
