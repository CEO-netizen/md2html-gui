use eframe::egui;
use pulldown_cmark::{html, Options, Parser};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct AppState {
    input_files: Vec<PathBuf>,
    output_files: Vec<PathBuf>,
    css_path: Option<PathBuf>,
    title: String,
    preview: bool,
    #[serde(skip)]
    status_message: String,
    #[serde(skip)]
    progress: f32,
}

impl AppState {
    fn convert_all(&mut self) {
        if self.input_files.len() != self.output_files.len() {
            self.status_message = "❌ Input/output file count mismatch.".to_string();
            return;
        }
        for (input, output) in self.input_files.iter().zip(self.output_files.iter()) {
            match fs::read_to_string(input) {
                Ok(md) => {
                    let mut options = Options::empty();
                    options.insert(Options::ENABLE_STRIKETHROUGH);
                    let parser = Parser::new_ext(&md, options);
                    let mut html_body = String::new();
                    html::push_html(&mut html_body, parser);
                    let title = if self.title.is_empty() {
                        input.file_name().unwrap_or_default().to_string_lossy().to_string()
                    } else {
                        self.title.clone()
                    };
                    let mut html_output = format!(
                        "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>{}</title>",
                        title
                    );
                    if let Some(css_path) = &self.css_path {
                        match fs::read_to_string(css_path) {
                            Ok(css) => {
                                html_output += &format!("<style>\n{}\n</style>", css);
                            }
                            Err(_) => {
                                html_output += &format!(
                                    "<link rel=\"stylesheet\" href=\"{}\">",
                                    css_path.display()
                                );
                            }
                        }
                    }
                    html_output += &format!("</head><body>{}</body></html>", html_body);
                    if let Err(e) = fs::write(output, html_output) {
                        self.status_message = format!("❌ Failed to write {}: {}", output.display(), e);
                        return;
                    }
                    if self.preview {
                        let _ = open_in_browser(output);
                    }
                    self.status_message = format!("✅ Converted: {} → {}", input.display(), output.display());
                    self.progress = 1.0;
                }
                Err(e) => {
                    self.status_message = format!("❌ Failed to read {}: {}", input.display(), e);
                    return;
                }
            }
        }
    }
    fn save_state(&self) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write("app_state.json", json);
        }
    }
    fn load_state() -> Self {
        fs::read_to_string("app_state.json")
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }
}

impl eframe::App for AppState {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Auto theme
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        ctx.set_visuals(egui::Visuals::default());
        #[cfg(target_os = "linux")]
        ctx.set_visuals(egui::Visuals::dark());
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading("📄 Markdown to HTML Converter");
            });
            ui.add_space(10.0);
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.label("📂 Input & Output Files");
                    if ui.button("➕ Add Markdown File").clicked() {
                        if let Some(md) = rfd::FileDialog::new()
                            .add_filter("Markdown", &["md"])
                            .pick_file()
                        {
                            self.input_files.push(md.clone());
                            let mut out = md.clone();
                            out.set_extension("html");
                            self.output_files.push(out);
                        }
                    }
                    let mut remove_indices = Vec::new();
                    for (i, input) in self.input_files.iter().enumerate() {
                        if let Some(output) = self.output_files.get(i) {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(format!("📄 {}", input.display()));
                                ui.label("➡");
                                ui.label(format!("💾 {}", output.display()));
                                if ui.button("❌ Remove").clicked() {
                                    remove_indices.push(i);
                                }
                            });
                        }
                    }
                    for &i in remove_indices.iter().rev() {
                        self.input_files.remove(i);
                        self.output_files.remove(i);
                    }
                });
            });
            ui.add_space(10.0);
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.label("🎨 CSS & Page Settings");
                    if ui.button("🖌 Select CSS File").clicked() {
                        if let Some(css) = rfd::FileDialog::new()
                            .add_filter("CSS", &["css"])
                            .pick_file()
                        {
                            self.css_path = Some(css);
                        }
                    }
                    if let Some(css) = self.css_path.clone() {
                        ui.horizontal(|ui| {
                            ui.monospace(format!("CSS: {}", css.display()));
                            if ui.button("❌ Remove CSS").clicked() {
                                self.css_path = None;
                            }
                        });
                    }
                    ui.horizontal(|ui| {
                        ui.label("📝 Title:");
                        ui.text_edit_singleline(&mut self.title);
                    });
                    ui.checkbox(&mut self.preview, "🌐 Open in browser after conversion");
                });
            });
            ui.add_space(15.0);
            ui.vertical_centered(|ui| {
                if ui
                    .add(egui::Button::new("🚀 Convert to HTML").fill(egui::Color32::from_rgb(80, 170, 255)))
                    .clicked()
                {
                    self.convert_all();
                    self.save_state();
                }
            });
            ui.add_space(10.0);
            ui.add(
                egui::ProgressBar::new(self.progress)
                    .show_percentage()
                    .desired_width(f32::INFINITY),
            );
            if !self.status_message.is_empty() {
                ui.label(
                    egui::RichText::new(&self.status_message)
                        .color(egui::Color32::LIGHT_YELLOW)
                        .strong(),
                );
            }
        });
    }
}

fn open_in_browser(path: &PathBuf) -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    return Command::new("xdg-open").arg(path).spawn().map(|_| ());
    #[cfg(target_os = "macos")]
    return Command::new("open").arg(path).spawn().map(|_| ());
    #[cfg(target_os = "windows")]
    return Command::new("cmd").args(["/C", "start", path.to_str().unwrap_or("")]).spawn().map(|_| ());
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Markdown to HTML GUI",
        options,
        Box::new(|_cc| Box::new(AppState::load_state())),
    )
}
