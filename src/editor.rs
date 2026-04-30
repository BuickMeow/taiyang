use nih_plug::prelude::*;
use nih_plug::context::gui::ParamSetter;
use nih_plug_egui::{create_egui_editor, egui};
use std::sync::Arc;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct EditorState {
    pub params: Arc<crate::params::TaiyangParams>,
    pub engine: Arc<Mutex<Option<crate::engine::SynthEngine>>>,
    pub selected_preset_idx: Arc<AtomicUsize>,
}

pub fn create(
    params: Arc<crate::params::TaiyangParams>,
    engine: Arc<Mutex<Option<crate::engine::SynthEngine>>>,
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
                ui.heading("Taiyang");
                ui.separator();

                draw_params(ui, setter, state);
                ui.separator();
                draw_soundfonts(ui, state);
                ui.separator();
                draw_presets(ui, setter, state);
            });
        },
    )
}

fn draw_params(ui: &mut egui::Ui, setter: &ParamSetter, state: &EditorState) {
    ui.horizontal(|ui| {
        let mut is_drum = state.params.is_drum.value();
        if ui.checkbox(&mut is_drum, "Drum").changed() {
            setter.set_parameter(&state.params.is_drum, is_drum);
        }

        ui.separator();

        ui.label("Gain:");
        let mut gain = state.params.gain.value();
        let response = ui.add(egui::Slider::new(&mut gain, -30.0..=12.0).text("dB"));
        if response.changed() {
            setter.set_parameter(&state.params.gain, gain);
        }

        ui.separator();

        ui.label("PBR:");
        let mut pbr = state.params.pitch_bend_range.value() as i32;
        if ui.add(egui::DragValue::new(&mut pbr).range(0..=24)).changed() {
            setter.set_parameter(&state.params.pitch_bend_range, pbr);
        }

        ui.separator();

        ui.label("Tune:");
        let mut coarse = state.params.master_coarse_tune.value() as i32;
        if ui.add(egui::DragValue::new(&mut coarse).range(-64..=63)).changed() {
            setter.set_parameter(&state.params.master_coarse_tune, coarse);
        }
        ui.label(".");
        let mut fine = state.params.master_fine_tune.value() as i32;
        if ui.add(egui::DragValue::new(&mut fine).range(-100..=100)).changed() {
            setter.set_parameter(&state.params.master_fine_tune, fine);
        }

        ui.separator();

        ui.label("Voices:");
        let voices = state.engine.lock().as_ref()
            .map(|e| e.active_voices())
            .unwrap_or(0);
        ui.label(format!("{}", voices));
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
                let name = path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Unknown".to_string());

                let entry = crate::params::SoundfontEntryData {
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
    params: &crate::params::TaiyangParams,
    engine: &Arc<Mutex<Option<crate::engine::SynthEngine>>>,
) {
    if let Some(ref mut eng) = engine.lock().as_mut() {
        let entries: Vec<crate::engine::SoundfontEntry> = params.soundfont_entries.lock()
            .iter()
            .map(|e| crate::engine::SoundfontEntry {
                path: e.path.clone(),
                name: e.name.clone(),
                enabled: e.enabled,
            })
            .collect();

        if let Err(e) = eng.load_soundfonts(&entries) {
            nih_log!("Failed to reload soundfonts: {}", e);
        } else {
            nih_log!("Reloaded {} soundfonts", entries.len());
        }
    }
}

fn draw_presets(ui: &mut egui::Ui, setter: &ParamSetter, state: &EditorState) {
    ui.horizontal(|ui| {
        ui.label("Presets");
        let mut locked = state.params.preset_locked.value();
        if ui.checkbox(&mut locked, "Lock").changed() {
            setter.set_parameter(&state.params.preset_locked, locked);
        }
    });

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

    egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
        for (i, preset) in presets.iter().enumerate() {
            let is_selected = i == selected_idx;
            let text = format!("{} - {} (Bank {} Prog {})",
                preset.source_file, preset.name, preset.bank, preset.program);

            let response = ui.selectable_label(is_selected, text);
            if response.clicked() {
                state.selected_preset_idx.store(i, Ordering::Relaxed);
                setter.set_parameter(&state.params.preset_locked, true);
                setter.set_parameter(&state.params.selected_bank, preset.bank as i32);
                setter.set_parameter(&state.params.selected_program, preset.program as i32);

                if let Some(ref mut eng) = state.engine.lock().as_mut() {
                    eng.send_preset_all(preset.bank as u8, preset.program as u8);
                }
            }
        }
    });

    ui.horizontal(|ui| {
        ui.label(format!("Bank: {}", state.params.selected_bank.value()));
        ui.label(format!("Program: {}", state.params.selected_program.value()));
    });
}
