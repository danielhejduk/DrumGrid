use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, widgets};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{self, Receiver, TryRecvError};

use crate::params::DrumParams;
use crate::sound::{self, SampleSlot};

const WAVEFORM_HEIGHT: f32 = 90.0;
const START_COLOR: egui::Color32 = egui::Color32::from_rgb(90, 200, 120);
const END_COLOR: egui::Color32 = egui::Color32::from_rgb(230, 140, 70);

#[derive(Default)]
struct EditorState {
    error: String,
    loading: Option<Mutex<Receiver<Result<(), String>>>>,
    dragging: Option<Marker>,
}

#[derive(Clone, Copy)]
enum Marker {
    Start,
    End,
}

impl Marker {
    fn param(self, params: &DrumParams) -> &FloatParam {
        match self {
            Marker::Start => &params.start,
            Marker::End => &params.end,
        }
    }
}

pub fn create(params: Arc<DrumParams>, slot: Arc<SampleSlot>) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        params.editor_state.clone(),
        EditorState::default(),
        |_, _| {},
        move |ctx, setter, state| {
            poll_loading(state);
            egui::CentralPanel::default().show(ctx, |ui| egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("DrumGrid");
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    let busy = state.loading.is_some();
                    if ui.add_enabled(!busy, egui::Button::new("Load sample...")).clicked() {
                        state.error.clear();
                        state.loading = Some(Mutex::new(spawn_loader(ctx.clone(), params.clone(), slot.clone())));
                    }
                    if busy {
                        ui.spinner();
                    }
                    ui.label(slot.name.lock().as_str());
                });
                if !state.error.is_empty() {
                    ui.colored_label(egui::Color32::LIGHT_RED, state.error.as_str());
                }
                ui.add_space(8.0);

                waveform(ui, &params, setter, &slot.overview.lock(), &mut state.dragging);
                ui.add_space(8.0);

                egui::Grid::new("params")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        for param in [&params.gain, &params.attack, &params.release, &params.start, &params.end, &params.scan] {
                            param_row(ui, param, setter);
                        }
                        param_row(ui, &params.scan_mode, setter);
                        for param in [&params.jitter, &params.grain, &params.speed] {
                            param_row(ui, param, setter);
                        }
                        param_row(ui, &params.tune, setter);
                        param_row(ui, &params.fine, setter);
                        param_row(ui, &params.bend_range, setter);
                    });
            }));
        },
    )
}

fn param_row<P: Param>(ui: &mut egui::Ui, param: &P, setter: &ParamSetter) {
    ui.label(param.name());
    ui.add(widgets::ParamSlider::for_param(param, setter).with_width(240.0));
    ui.end_row();
}

fn spawn_loader(ctx: egui::Context, params: Arc<DrumParams>, slot: Arc<SampleSlot>) -> Receiver<Result<(), String>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        if let Some(path) = rfd::FileDialog::new().add_filter("WAV", &["wav"]).pick_file() {
            let _ = tx.send(load(&path, &params, &slot));
        }
        drop(tx);
        ctx.request_repaint();
    });
    rx
}

fn poll_loading(state: &mut EditorState) {
    let Some(rx) = &state.loading else { return };
    let received = rx.lock().map_or(Err(TryRecvError::Disconnected), |rx| rx.try_recv());
    match received {
        Ok(result) => {
            state.error = result.err().unwrap_or_default();
            state.loading = None;
        }
        Err(TryRecvError::Disconnected) => state.loading = None,
        Err(TryRecvError::Empty) => {}
    }
}

fn waveform(
    ui: &mut egui::Ui,
    params: &DrumParams,
    setter: &ParamSetter,
    peaks: &[[f32; 2]],
    dragging: &mut Option<Marker>,
) {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), WAVEFORM_HEIGHT),
        egui::Sense::click_and_drag());
    let response = response.on_hover_cursor(egui::CursorIcon::ResizeHorizontal);

    if let Some(pos) = response.interact_pointer_pos() {
        let t = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
        let marker = *dragging.get_or_insert_with(|| {
            let marker = if (t - params.start.value()).abs() <= (t - params.end.value()).abs() {
                Marker::Start
            } else {
                Marker::End
            };
            setter.begin_set_parameter(marker.param(params));
            marker
        });
        setter.set_parameter(marker.param(params), t);
    }
    if (response.drag_stopped() || response.clicked() || response.interact_pointer_pos().is_none()) && let Some(marker) = dragging.take() {
        setter.end_set_parameter(marker.param(params));
    }
    let painter = ui.painter_at(rect);
    let visuals = ui.visuals();
    painter.rect_filled(rect, 4.0, visuals.extreme_bg_color);

    let (start, end) = (params.start.value(), params.end.value());
    let (lo, hi) = (start.min(end), start.max(end));
    let x_of = |t: f32| rect.left() + t * rect.width();
    painter.rect_filled(egui::Rect::from_x_y_ranges(x_of(lo)..=x_of(hi), rect.y_range()), 0.0, visuals.faint_bg_color);

    let cols = rect.width().max(1.0) as usize;
    let (mid, half) = (rect.center().y, rect.height() * 0.5 - 4.0);
    if !peaks.is_empty() {
        for c in 0..cols {
            let from = c * peaks.len() / cols;
            let to = ((c + 1) * peaks.len() / cols).max(from + 1).min(peaks.len());
            let [min, max] = peaks[from..to].iter().fold([f32::MAX, f32::MIN], |[lo, hi], p| [lo.min(p[0]), hi.max(p[1])]);
            let (top, bottom) = (mid - max.clamp(-1.0, 1.0) * half, mid - min.clamp(-1.0, 1.0) * half);
            let t = (c as f32 + 0.5) / cols as f32;
            let color = if (lo..=hi).contains(&t) { visuals.strong_text_color() } else { visuals.weak_text_color() };
            let x = rect.left() + c as f32 + 0.5;
            painter.line_segment([egui::pos2(x, top.min(bottom - 1.0)), egui::pos2(x, bottom)],
                egui::Stroke::new(1.0_f32, color),
            );
        }
    }

    for (t, color, label) in [(start, START_COLOR, "S"), (end, END_COLOR, "E")] {
        let x = x_of(t);
        painter.line_segment([egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())], egui::Stroke::new(2.0_f32, color));
        painter.text(
            egui::pos2(x + 3.0, rect.top() + 2.0),
            egui::Align2::LEFT_TOP,
            label,
            egui::FontId::proportional(11.0),
            color,
        );
    }
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
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    slot.offer(sample, name);
    *params.sample_path.write() = path.to_string_lossy().into_owned();
    *params.sample_data.write() = data;
    Ok(())
}
