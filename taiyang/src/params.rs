use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use parking_lot::Mutex;
use std::sync::Arc;

fn env_time_formatter(value: f32) -> String {
    if value < 0.0 {
        "Auto".to_string()
    } else {
        format!("{:.3} s", value)
    }
}

fn env_time_parser(string: &str) -> Option<f32> {
    if string.eq_ignore_ascii_case("auto") {
        Some(-1.0)
    } else {
        string.trim().parse::<f32>().ok().map(|v| v.max(-1.0))
    }
}

fn sustain_formatter(value: f32) -> String {
    if value < 0.0 {
        "Auto".to_string()
    } else {
        format!("{:.1} %", value * 100.0)
    }
}

fn sustain_parser(string: &str) -> Option<f32> {
    if string.eq_ignore_ascii_case("auto") {
        Some(-1.0)
    } else {
        string
            .trim()
            .parse::<f32>()
            .ok()
            .map(|v| (v / 100.0).clamp(-1.0, 1.0))
    }
}

#[derive(Params)]
pub struct TaiyangParams {
    #[persist = "editor_state"]
    pub editor_state: Arc<EguiState>,

    #[persist = "soundfont_entries"]
    pub soundfont_entries: Arc<Mutex<Vec<taiyang_shared::engine::SoundfontEntry>>>,

    #[id = "gain"]
    pub gain: FloatParam,

    #[id = "is_drum"]
    pub is_drum: BoolParam,

    #[id = "preset_locked"]
    pub preset_locked: BoolParam,

    #[id = "selected_bank"]
    pub selected_bank: IntParam,

    #[id = "selected_program"]
    pub selected_program: IntParam,

    #[id = "pitch_bend_range"]
    pub pitch_bend_range: FloatParam,

    #[id = "master_fine_tune"]
    pub master_fine_tune: FloatParam,

    #[id = "master_coarse_tune"]
    pub master_coarse_tune: FloatParam,

    // Low-pass filter
    #[id = "cutoff"]
    pub cutoff: FloatParam,

    #[id = "resonance"]
    pub resonance: FloatParam,

    // High-pass filter
    #[id = "highpass_cutoff"]
    pub highpass_cutoff: FloatParam,

    #[id = "highpass_resonance"]
    pub highpass_resonance: FloatParam,

    // Envelope
    #[id = "env_delay"]
    pub env_delay: FloatParam,

    #[id = "env_attack"]
    pub env_attack: FloatParam,

    #[id = "env_hold"]
    pub env_hold: FloatParam,

    #[id = "env_decay"]
    pub env_decay: FloatParam,

    #[id = "env_sustain"]
    pub env_sustain: FloatParam,

    #[id = "env_release"]
    pub env_release: FloatParam,
}

impl Default for TaiyangParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(640, 480),
            soundfont_entries: Arc::new(Mutex::new(Vec::new())),
            gain: FloatParam::new("Gain", 1.0, FloatRange::Linear { min: 0.0, max: 2.0 })
                .with_smoother(SmoothingStyle::None)
                .with_unit(" dB")
                .with_value_to_string(formatters::v2s_f32_gain_to_db(2))
                .with_string_to_value(formatters::s2v_f32_gain_to_db()),
            is_drum: BoolParam::new("Drum Mode", false),
            preset_locked: BoolParam::new("Lock Preset", false),
            selected_bank: IntParam::new("Bank", 0, IntRange::Linear { min: 0, max: 127 }),
            selected_program: IntParam::new("Program", 0, IntRange::Linear { min: 0, max: 127 }),
            pitch_bend_range: FloatParam::new(
                "Pitch Bend Range",
                2.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 127.0,
                },
            ),
            master_fine_tune: FloatParam::new(
                "Fine Tune",
                0.0,
                FloatRange::Linear {
                    min: -100.0,
                    max: 100.0,
                },
            ),
            master_coarse_tune: FloatParam::new(
                "Coarse Tune",
                0.0,
                FloatRange::Linear {
                    min: -64.0,
                    max: 63.0,
                },
            ),

            // Filter
            cutoff: FloatParam::new(
                "Cutoff",
                20000.0,
                FloatRange::Skewed {
                    min: 0.0,
                    max: 20000.0,
                    factor: 0.3,
                },
            )
            .with_smoother(SmoothingStyle::None)
            .with_unit(" Hz")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),

            resonance: FloatParam::new(
                "Resonance",
                0.70710677,
                FloatRange::Skewed {
                    min: 0.1,
                    max: 10.0,
                    factor: 0.5,
                },
            )
            .with_smoother(SmoothingStyle::None)
            .with_value_to_string(formatters::v2s_f32_rounded(3)),

            highpass_cutoff: FloatParam::new(
                "HP Cutoff",
                0.0,
                FloatRange::Skewed {
                    min: 0.0,
                    max: 20000.0,
                    factor: 0.3,
                },
            )
            .with_smoother(SmoothingStyle::None)
            .with_unit(" Hz")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),

            highpass_resonance: FloatParam::new(
                "HP Resonance",
                0.70710677,
                FloatRange::Skewed {
                    min: 0.1,
                    max: 10.0,
                    factor: 0.5,
                },
            )
            .with_smoother(SmoothingStyle::None)
            .with_value_to_string(formatters::v2s_f32_rounded(3)),

            // Envelope
            env_delay: FloatParam::new(
                "Delay",
                -1.0,
                FloatRange::Skewed {
                    min: -0.001,
                    max: 10.0,
                    factor: 0.33,
                },
            )
            .with_smoother(SmoothingStyle::None)
            .with_value_to_string(Arc::new(env_time_formatter))
            .with_string_to_value(Arc::new(env_time_parser)),

            env_attack: FloatParam::new(
                "Attack",
                -1.0,
                FloatRange::Skewed {
                    min: -0.001,
                    max: 10.0,
                    factor: 0.33,
                },
            )
            .with_smoother(SmoothingStyle::None)
            .with_value_to_string(Arc::new(env_time_formatter))
            .with_string_to_value(Arc::new(env_time_parser)),

            env_hold: FloatParam::new(
                "Hold",
                -1.0,
                FloatRange::Skewed {
                    min: -0.001,
                    max: 10.0,
                    factor: 0.33,
                },
            )
            .with_smoother(SmoothingStyle::None)
            .with_value_to_string(Arc::new(env_time_formatter))
            .with_string_to_value(Arc::new(env_time_parser)),

            env_decay: FloatParam::new(
                "Decay",
                -1.0,
                FloatRange::Skewed {
                    min: -0.001,
                    max: 10.0,
                    factor: 0.33,
                },
            )
            .with_smoother(SmoothingStyle::None)
            .with_value_to_string(Arc::new(env_time_formatter))
            .with_string_to_value(Arc::new(env_time_parser)),

            env_sustain: FloatParam::new(
                "Sustain",
                -1.0,
                FloatRange::Linear {
                    min: -0.001,
                    max: 1.0,
                },
            )
            .with_smoother(SmoothingStyle::None)
            .with_value_to_string(Arc::new(sustain_formatter))
            .with_string_to_value(Arc::new(sustain_parser)),

            env_release: FloatParam::new(
                "Release",
                -1.0,
                FloatRange::Skewed {
                    min: -0.001,
                    max: 10.0,
                    factor: 0.33,
                },
            )
            .with_smoother(SmoothingStyle::None)
            .with_value_to_string(Arc::new(env_time_formatter))
            .with_string_to_value(Arc::new(env_time_parser)),
        }
    }
}
