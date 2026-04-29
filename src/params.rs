use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use std::sync::Arc;
use parking_lot::Mutex;

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct SoundfontEntryData {
    pub path: String,
    pub name: String,
    pub enabled: bool,
}

#[derive(Params)]
pub struct TaiyangParams {
    #[persist = "editor_state"]
    pub editor_state: Arc<EguiState>,

    #[persist = "soundfont_entries"]
    pub soundfont_entries: Arc<Mutex<Vec<SoundfontEntryData>>>,

    #[id = "midi_channel"]
    pub midi_channel: IntParam,

    #[id = "gain"]
    pub gain: FloatParam,

    #[id = "max_voices"]
    pub max_voices: IntParam,

    #[id = "is_drum"]
    pub is_drum: BoolParam,

    #[id = "preset_locked"]
    pub preset_locked: BoolParam,

    #[id = "selected_bank"]
    pub selected_bank: IntParam,

    #[id = "selected_program"]
    pub selected_program: IntParam,
}

impl Default for TaiyangParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(640, 480),
            soundfont_entries: Arc::new(Mutex::new(Vec::new())),
            midi_channel: IntParam::new(
                "MIDI Channel",
                1,
                IntRange::Linear { min: 1, max: 16 },
            ),
            gain: FloatParam::new(
                "Gain",
                0.0,
                FloatRange::Linear { min: -30.0, max: 12.0 },
            )
            .with_smoother(SmoothingStyle::Linear(5.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(2))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),
            max_voices: IntParam::new(
                "Max Voices",
                1000,
                IntRange::Linear { min: 1, max: 10000 },
            ),
            is_drum: BoolParam::new("Drum Mode", false),
            preset_locked: BoolParam::new("Lock Preset", false),
            selected_bank: IntParam::new(
                "Bank",
                0,
                IntRange::Linear { min: 0, max: 127 },
            ),
            selected_program: IntParam::new(
                "Program",
                0,
                IntRange::Linear { min: 0, max: 127 },
            ),
        }
    }
}
