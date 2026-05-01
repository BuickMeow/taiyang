use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use std::sync::Arc;
use parking_lot::Mutex;

#[derive(Params)]
pub struct TaiyangParams {
    #[persist = "editor_state"]
    pub editor_state: Arc<EguiState>,

    #[persist = "soundfont_entries"]
    pub soundfont_entries: Arc<Mutex<Vec<crate::engine::SoundfontEntry>>>,

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
}

impl Default for TaiyangParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(640, 480),
            soundfont_entries: Arc::new(Mutex::new(Vec::new())),
            gain: FloatParam::new(
                "Gain",
                0.0,
                FloatRange::Linear { min: -30.0, max: 12.0 },
            )
            .with_smoother(SmoothingStyle::Linear(5.0))
            .with_unit(" dB"),
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
            pitch_bend_range: FloatParam::new(
                "Pitch Bend Range",
                2.0,
                FloatRange::Linear { min: 0.0, max: 127.0 },
            ),
            master_fine_tune: FloatParam::new(
                "Fine Tune",
                0.0,
                FloatRange::Linear { min: -100.0, max: 100.0 },
            ),
            master_coarse_tune: FloatParam::new(
                "Coarse Tune",
                0.0,
                FloatRange::Linear { min: -64.0, max: 63.0 },
            ),
        }
    }
}
