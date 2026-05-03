use nih_plug::context::gui::ParamSetter;
use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, widgets};
use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use taiyang_shared::engine::{SoundfontEntry, SynthEngine};

pub struct EditorState {
    pub params: Arc<crate::params::Taiyang16Params>,
    pub engine: Arc<Mutex<Option<SynthEngine>>>,
    pub selected_preset_idx: Arc<AtomicUsize>,
}

pub fn create(
    params: Arc<crate::params::Taiyang16Params>,
    engine: Arc<Mutex<Option<SynthEngine>>>,
) -> Option<Box<dyn Editor>> {
    let egui_state = params.editor_state.clone();
    let state = EditorState {
        params: params.clone(),
        engine,
        selected_preset_idx: Arc::new(AtomicUsize::new(0)),
    };

    create_egui_editor(
        egui_state,
        state,
        |_, _| {},
        move |egui_ctx, setter, state| {
            egui::CentralPanel::default().show(egui_ctx, |ui| {
                ui.heading("Taiyang16 — 16-Channel SoundFont Synth");
                ui.separator();

                draw_channel_header(ui, setter, state);
                ui.separator();
                draw_global_params(ui, setter, state);
                ui.separator();
                draw_soundfonts(ui, state);
                ui.separator();
                draw_presets(ui, setter, state);
                ui.separator();
                draw_channel_status(ui, state);
            });
        },
    )
}

fn draw_channel_header(ui: &mut egui::Ui, setter: &ParamSetter, state: &EditorState) {
    ui.horizontal(|ui| {
        ui.label("Editing Channel:");
        let mut ch = state.params.selected_channel.value() as u8;
        let prev_ch = ch;
        // Show channel number as 1-16 (MIDI convention)
        if ui
            .add(egui::DragValue::new(&mut ch).range(0..=15).suffix(""))
            .changed()
        {
            setter.set_parameter(&state.params.selected_channel, ch as i32);
        }

        if ch != prev_ch {
            // Channel changed — the process() will pick up the new channel
        }

        // Quick channel preset buttons
        for &c in &[0u8, 9u8] {
            if ui.button(if c == 9 { "Drums" } else { "1" }).clicked() {
                setter.set_parameter(&state.params.selected_channel, c as i32);
            }
        }

        ui.separator();

        if ch == 9 {
            ui.colored_label(
                egui::Color32::from_rgb(255, 200, 100),
                format!("Ch.{} 🥁", ch + 1),
            );
        } else {
            ui.label(format!("Ch.{}", ch + 1));
        }

        ui.separator();

        let mut is_drum = state.params.is_drum.value();
        if ui.checkbox(&mut is_drum, "Drum").changed() {
            setter.set_parameter(&state.params.is_drum, is_drum);
        }

        ui.separator();

        let mut locked = state.params.preset_locked.value();
        if ui.checkbox(&mut locked, "Lock").changed() {
            setter.set_parameter(&state.params.preset_locked, locked);
        }

        ui.separator();

        ui.label("Bank:");
        let mut bank = state.params.selected_bank.value();
        if ui
            .add(egui::DragValue::new(&mut bank).range(0..=127))
            .changed()
        {
            setter.set_parameter(&state.params.selected_bank, bank);
        }

        ui.label("Prog:");
        let mut prog = state.params.selected_program.value();
        if ui
            .add(egui::DragValue::new(&mut prog).range(0..=127))
            .changed()
        {
            setter.set_parameter(&state.params.selected_program, prog);
        }
    });
}

fn draw_global_params(ui: &mut egui::Ui, setter: &ParamSetter, state: &EditorState) {
    ui.horizontal(|ui| {
        ui.label("Gain:");
        ui.add(widgets::ParamSlider::for_param(&state.params.gain, setter));

        ui.separator();

        ui.label("Voices:");
        let voices = state
            .engine
            .lock()
            .as_ref()
            .map(|e| e.active_voices())
            .unwrap_or(0);
        ui.label(format!("{}", voices));
    });

    ui.separator();

    // Filter section
    ui.collapsing("Filter (Global)", |ui| {
        ui.horizontal(|ui| {
            ui.label("LP Cutoff:");
            ui.add(widgets::ParamSlider::for_param(
                &state.params.cutoff,
                setter,
            ));
            ui.separator();
            ui.label("LP Res:");
            ui.add(widgets::ParamSlider::for_param(
                &state.params.resonance,
                setter,
            ));
        });
        ui.horizontal(|ui| {
            ui.label("HP Cutoff:");
            ui.add(widgets::ParamSlider::for_param(
                &state.params.highpass_cutoff,
                setter,
            ));
            ui.separator();
            ui.label("HP Res:");
            ui.add(widgets::ParamSlider::for_param(
                &state.params.highpass_resonance,
                setter,
            ));
        });
    });

    ui.separator();

    // Envelope section
    ui.collapsing("Envelope (Global)", |ui| {
        ui.horizontal(|ui| {
            ui.label("Delay:");
            ui.add(widgets::ParamSlider::for_param(
                &state.params.env_delay,
                setter,
            ));
            ui.separator();
            ui.label("Attack:");
            ui.add(widgets::ParamSlider::for_param(
                &state.params.env_attack,
                setter,
            ));
            ui.separator();
            ui.label("Hold:");
            ui.add(widgets::ParamSlider::for_param(
                &state.params.env_hold,
                setter,
            ));
        });
        ui.horizontal(|ui| {
            ui.label("Decay:");
            ui.add(widgets::ParamSlider::for_param(
                &state.params.env_decay,
                setter,
            ));
            ui.separator();
            ui.label("Sustain:");
            ui.add(widgets::ParamSlider::for_param(
                &state.params.env_sustain,
                setter,
            ));
            ui.separator();
            ui.label("Release:");
            ui.add(widgets::ParamSlider::for_param(
                &state.params.env_release,
                setter,
            ));
        });
    });
}

fn draw_soundfonts(ui: &mut egui::Ui, state: &EditorState) {
    ui.horizontal(|ui| {
        ui.label("SoundFonts");
        if ui.button("Add").clicked() {
            spawn_add_soundfont_dialog(state);
        }
    });

    let mut entries = state.params.soundfont_entries.lock();
    let mut need_reload = false;
    let mut remove_idx = None;

    for (i, entry) in entries.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            let mut enabled = entry.enabled;
            if ui.checkbox(&mut enabled, "").changed() {
                entry.enabled = enabled;
                need_reload = true;
            }
            ui.label(&entry.name);
            ui.label(egui::RichText::new(&entry.path).small().weak());
            if ui.button("Remove").clicked() {
                remove_idx = Some(i);
            }
        });
    }

    if let Some(idx) = remove_idx {
        entries.remove(idx);
        need_reload = true;
    }

    drop(entries);

    if need_reload {
        reload_soundfonts(state);
    }
}

fn spawn_add_soundfont_dialog(state: &EditorState) {
    let params = state.params.clone();
    let engine = state.engine.clone();

    std::thread::spawn(move || {
        let paths = rfd::FileDialog::new()
            .add_filter("SoundFont", &["sf2", "sfz"])
            .pick_files();

        if let Some(paths) = paths {
            let mut added = false;
            for path in paths {
                let path_str = path.to_string_lossy().to_string();
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Unknown".to_string());

                let entry = SoundfontEntry {
                    path: path_str,
                    name,
                    enabled: true,
                };

                params.soundfont_entries.lock().push(entry);
                added = true;
            }

            if added {
                reload_soundfonts_from_state(&params, &engine);
            }
        }
    });
}

fn reload_soundfonts(state: &EditorState) {
    reload_soundfonts_from_state(&state.params, &state.engine);
}

fn reload_soundfonts_from_state(
    params: &crate::params::Taiyang16Params,
    engine: &Arc<Mutex<Option<SynthEngine>>>,
) {
    if let Some(ref mut eng) = engine.lock().as_mut() {
        let entries = params.soundfont_entries.lock().clone();

        if let Err(e) = eng.load_soundfonts(&entries) {
            nih_log!("Failed to reload soundfonts: {}", e);
        } else {
            nih_log!("Reloaded {} soundfonts", entries.len());
        }
    }
}

fn draw_presets(ui: &mut egui::Ui, setter: &ParamSetter, state: &EditorState) {
    ui.label("Presets (click to assign to current channel)");

    let presets = {
        let engine_guard = state.engine.lock();
        if let Some(ref engine) = engine_guard.as_ref() {
            engine.presets().to_vec()
        } else {
            Vec::new()
        }
    };

    if presets.is_empty() {
        ui.label("No presets available");
        return;
    }

    let selected_idx = state.selected_preset_idx.load(Ordering::Relaxed);

    egui::ScrollArea::vertical()
        .max_height(200.0)
        .show(ui, |ui| {
            for (i, preset) in presets.iter().enumerate() {
                let is_selected = i == selected_idx;
                let text = format!(
                    "{} - {} (Bank {} Prog {})",
                    preset.source_file, preset.name, preset.bank, preset.program
                );

                let response = ui.selectable_label(is_selected, text);
                if response.clicked() {
                    state.selected_preset_idx.store(i, Ordering::Relaxed);
                    setter.set_parameter(&state.params.preset_locked, true);
                    setter.set_parameter(&state.params.selected_bank, preset.bank as i32);
                    setter.set_parameter(&state.params.selected_program, preset.program as i32);

                    let ch = state.params.selected_channel.value() as u8;
                    if let Some(ref mut eng) = state.engine.lock().as_mut() {
                        eng.send_preset(ch, preset.bank as u8, preset.program as u8);
                    }
                }
            }
        });
}

fn draw_channel_status(ui: &mut egui::Ui, _state: &EditorState) {
    ui.collapsing("Channel Status", |ui| {
        egui::Grid::new("ch_status_grid")
            .striped(true)
            .show(ui, |ui| {
                ui.label("Ch");
                ui.label("Bank");
                ui.label("Prog");
                ui.label("Drum");
                ui.end_row();

                for ch in 0u8..16 {
                    let drum_str = if ch == 9 { "🥁" } else { "" };
                    ui.label(format!("{} {}", ch + 1, drum_str));
                    ui.label("—");
                    ui.label("—");
                    ui.label(if ch == 9 { "Yes" } else { "No" });
                    ui.end_row();
                }
            });
        ui.label("(Bank/Prog status updated by incoming MIDI)");
    });
}
