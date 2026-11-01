use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, widgets};
use std::path::Path;
use std::sync::Arc;

use crate::params::DrumParams;
use crate::sound::{self, SampleSlot};

pub fn create(params: Arc<DrumParams>, slot: Arc<SampleSlot>) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        params.editor_state.clone(),
        String::new(), // last load error
        |_, _| {},
        move |ctx, setter, error| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("DrumGrid");
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    if ui.button("Load sample…").clicked()
                        && let Some(path) = rfd::FileDialog::new().add_filter("WAV", &["wav"]).pick_file()
                    {
                        *error = load(&path, &params, &slot).err().unwrap_or_default();
                    }
                    ui.label(slot.name.lock().as_str());
                });
                if !error.is_empty() {
                    ui.colored_label(egui::Color32::LIGHT_RED, error.as_str());
                }
                ui.add_space(8.0);

                egui::Grid::new("params").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                    for param in [&params.gain, &params.attack, &params.release, &params.start, &params.end, &params.scan] {
                        ui.label(param.name());
                        ui.add(widgets::ParamSlider::for_param(param, setter).with_width(240.0));
                        ui.end_row();
                    }
                    ui.label(params.scan_mode.name());
                    ui.add(widgets::ParamSlider::for_param(&params.scan_mode, setter).with_width(240.0));
                    ui.end_row();
                    for param in [&params.jitter, &params.grain, &params.speed] {
                        ui.label(param.name());
                        ui.add(widgets::ParamSlider::for_param(param, setter).with_width(240.0));
                        ui.end_row();
                    }
                });
            });
        },
    )
}

fn load(path: &Path, params: &DrumParams, slot: &SampleSlot) -> Result<(), String> {
    use base64::Engine;
    let err = |e: &dyn std::fmt::Display| format!("Couldn't load {}: {e}", path.display());
    let bytes = std::fs::read(path).map_err(|e| err(&e))?;
    let sample = sound::decode_wav(std::io::Cursor::new(&bytes)).map_err(|e| err(&e))?;
    let data = if bytes.len() <= sound::MAX_EMBED_BYTES {
        base64::engine::general_purpose::STANDARD.encode(&bytes)
    } else {
        String::new()
    };
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    slot.offer(sample, name);
    *params.sample_path.write() = path.to_string_lossy().into_owned();
    *params.sample_data.write() = data;
    Ok(())
}
