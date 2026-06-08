mod packaging;

use crate::packaging::PackagingInput;
use eframe::egui::{self, RichText, Vec2};
use eframe::{App, Frame, NativeOptions};
use std::path::Path;

#[derive(Debug, Clone)]
struct AppState {
    current_page: usize,

    metadata_app_name: String,
    metadata_version: String,
    metadata_publisher: String,
    metadata_description: String,

    payload_appimage_path: String,
    payload_info: String,
    shell_binary_source_path: String,

    branding_logo_path: String,
    branding_sidebar_image_path: String,
    branding_banner_path: String,

    install_path: String,
    default_desktop_shortcut: bool,
    default_menu_entry: bool,
    default_path_symlink: bool,

    output_dir: String,

    build_logs: Vec<String>,
    build_progress: f32,
    build_error: Option<String>,
    last_output_appimage: String,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            current_page: 0,

            metadata_app_name: String::new(),
            metadata_version: "1.0.0".to_string(),
            metadata_publisher: String::new(),
            metadata_description: String::new(),

            payload_appimage_path: String::new(),
            payload_info: String::new(),
            shell_binary_source_path: "target/release/installer-shell".to_string(),

            branding_logo_path: String::new(),
            branding_sidebar_image_path: String::new(),
            branding_banner_path: String::new(),

            install_path: "~/.local/lib/<safe_app_name>".to_string(),
            default_desktop_shortcut: true,
            default_menu_entry: true,
            default_path_symlink: true,

            output_dir: "~/AppImage-Installers".to_string(),

            build_logs: Vec::new(),
            build_progress: 0.0,
            build_error: None,
            last_output_appimage: String::new(),
        }
    }
}

impl AppState {
    const PAGE_TITLES: [&'static str; 5] = [
        "Metadata",
        "Payload",
        "Branding",
        "Install Defaults",
        "Build",
    ];

    fn page_title(&self) -> &'static str {
        Self::PAGE_TITLES
            .get(self.current_page)
            .copied()
            .unwrap_or("Unknown")
    }
}

struct BuilderApp {
    state: AppState,
}

impl Default for BuilderApp {
    fn default() -> Self {
        Self {
            state: AppState::default(),
        }
    }
}

impl App for BuilderApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut Frame) {
        let ctx = ui.ctx().clone();
        egui::CentralPanel::default().show(&ctx, |ui| {
            ui.heading("Loculus Builder");
            ui.label(format!(
                "Step {} of {} — {}",
                self.state.current_page + 1,
                AppState::PAGE_TITLES.len(),
                self.state.page_title()
            ));
            ui.separator();

            ui.horizontal_wrapped(|ui| {
                for (idx, title) in AppState::PAGE_TITLES.iter().enumerate() {
                    let selected = self.state.current_page == idx;
                    if ui.selectable_label(selected, *title).clicked() {
                        self.state.current_page = idx;
                    }
                }
            });

            ui.separator();

            match self.state.current_page {
                0 => self.render_metadata_page(ui),
                1 => self.render_payload_page(ui),
                2 => self.render_branding_page(ui),
                3 => self.render_install_defaults_page(ui),
                4 => self.render_build_page(ui),
                _ => {
                    ui.label("Invalid step index.");
                }
            };
        });
    }
}

impl BuilderApp {
    fn render_metadata_page(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Metadata").strong());
        ui.add_space(6.0);

        ui.label("Application name:");
        ui.add(egui::TextEdit::singleline(&mut self.state.metadata_app_name).desired_width(f32::INFINITY));

        ui.label("Version:");
        ui.add(egui::TextEdit::singleline(&mut self.state.metadata_version).desired_width(f32::INFINITY));

        ui.label("Publisher:");
        ui.add(egui::TextEdit::singleline(&mut self.state.metadata_publisher).desired_width(f32::INFINITY));

        ui.label("Description:");
        ui.add(
            egui::TextEdit::multiline(&mut self.state.metadata_description)
                .desired_width(f32::INFINITY)
                .desired_rows(4),
        );
    }

    fn render_payload_page(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Payload").strong());
        ui.add_space(6.0);

        ui.label("Source AppImage:");
        ui.horizontal(|ui| {
            let field_width = (ui.available_width() - 96.0).max(140.0);
            ui.add_sized(
                [field_width, 0.0],
                egui::TextEdit::singleline(&mut self.state.payload_appimage_path),
            );
            if ui.button("Browse...").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("AppImage Files", &["AppImage"])
                    .pick_file()
                {
                    self.state.payload_appimage_path = path.display().to_string();
                    self.state.payload_info = payload_info_for(&path);
                }
            }
        });

        if !self.state.payload_info.trim().is_empty() {
            ui.label(&self.state.payload_info);
        }

        ui.add_space(10.0);
        ui.label("Shell binary source path:");
        ui.horizontal(|ui| {
            let field_width = (ui.available_width() - 96.0).max(140.0);
            ui.add_sized(
                [field_width, 0.0],
                egui::TextEdit::singleline(&mut self.state.shell_binary_source_path),
            );
            if ui.button("Browse...").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_file() {
                    self.state.shell_binary_source_path = path.display().to_string();
                }
            }
        });
    }

    fn render_branding_page(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Branding").strong());
        ui.add_space(6.0);

        ui.label("Logo / Icon:");
        ui.horizontal(|ui| {
            let field_width = (ui.available_width() - 96.0).max(140.0);
            ui.add_sized(
                [field_width, 0.0],
                egui::TextEdit::singleline(&mut self.state.branding_logo_path),
            );
            if ui.button("Browse...").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Images", &["png", "jpg", "jpeg", "svg", "webp"])
                    .pick_file()
                {
                    self.state.branding_logo_path = path.display().to_string();
                }
            }
        });
        ui.label(RichText::new("PNG, square, 128x128+ recommended.").small());

        ui.add_space(8.0);
        ui.label("Sidebar image (optional):");
        ui.horizontal(|ui| {
            let field_width = (ui.available_width() - 96.0).max(140.0);
            ui.add_sized(
                [field_width, 0.0],
                egui::TextEdit::singleline(&mut self.state.branding_sidebar_image_path),
            );
            if ui.button("Browse...").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Images", &["png", "jpg", "jpeg", "svg", "webp"])
                    .pick_file()
                {
                    self.state.branding_sidebar_image_path = path.display().to_string();
                }
            }
        });
        ui.label(RichText::new("~200x400 recommended.").small());

        ui.add_space(8.0);
        ui.label("Banner (optional):");
        ui.horizontal(|ui| {
            let field_width = (ui.available_width() - 96.0).max(140.0);
            ui.add_sized(
                [field_width, 0.0],
                egui::TextEdit::singleline(&mut self.state.branding_banner_path),
            );
            if ui.button("Browse...").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Images", &["png", "jpg", "jpeg", "svg", "webp"])
                    .pick_file()
                {
                    self.state.branding_banner_path = path.display().to_string();
                }
            }
        });
    }

    fn render_install_defaults_page(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Install Defaults").strong());
        ui.add_space(6.0);

        ui.label("Default install path:");
        ui.add(egui::TextEdit::singleline(&mut self.state.install_path).desired_width(f32::INFINITY));

        ui.add_space(8.0);
        ui.checkbox(
            &mut self.state.default_desktop_shortcut,
            "Create desktop shortcut",
        );
        ui.checkbox(&mut self.state.default_menu_entry, "Add to applications menu");
        ui.checkbox(
            &mut self.state.default_path_symlink,
            "Create command-line symlink",
        );
    }

    fn render_build_page(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Build").strong());
        ui.add_space(6.0);

        ui.label("Output directory:");
        ui.horizontal(|ui| {
            let field_width = (ui.available_width() - 96.0).max(140.0);
            ui.add_sized(
                [field_width, 0.0],
                egui::TextEdit::singleline(&mut self.state.output_dir),
            );
            if ui.button("Browse...").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    self.state.output_dir = path.display().to_string();
                }
            }
        });

        ui.add_space(8.0);
        ui.label(RichText::new("Configuration Summary").strong());
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.label(format!("App: {}", value_or_placeholder(&self.state.metadata_app_name, "(not set)")));
            ui.label(format!("Version: {}", value_or_placeholder(&self.state.metadata_version, "(not set)")));
            ui.label(format!(
                "Payload: {}",
                payload_filename_or_placeholder(&self.state.payload_appimage_path)
            ));
            ui.label(format!(
                "Logo/Icon: {}",
                value_or_placeholder(&self.state.branding_logo_path, "(not set)")
            ));
            ui.label(format!(
                "Sidebar image: {}",
                value_or_placeholder(&self.state.branding_sidebar_image_path, "(optional, not set)")
            ));
            ui.label(format!(
                "Banner: {}",
                value_or_placeholder(&self.state.branding_banner_path, "(optional, not set)")
            ));
            ui.label(format!("Install path: {}", value_or_placeholder(&self.state.install_path, "(not set)")));
            ui.label(format!("Output dir: {}", value_or_placeholder(&self.state.output_dir, "(not set)")));
        });

        ui.add_space(8.0);
        if ui.button("Build installer AppImage").clicked() {
            self.run_build();
        }

        ui.add(
            egui::ProgressBar::new(self.state.build_progress)
                .show_percentage()
                .text("Build progress"),
        );

        if !self.state.last_output_appimage.trim().is_empty() {
            ui.label(format!("Last output: {}", self.state.last_output_appimage));
        }

        if let Some(err) = &self.state.build_error {
            ui.colored_label(egui::Color32::from_rgb(190, 40, 40), format!("Build error: {err}"));
        }

        ui.separator();
        ui.label("Build log:");
        egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
            if self.state.build_logs.is_empty() {
                ui.label("(no build logs yet)");
            } else {
                for line in &self.state.build_logs {
                    ui.label(line);
                }
            }
        });
    }

    fn run_build(&mut self) {
        self.state.build_logs.clear();
        self.state.build_error = None;
        self.state.last_output_appimage.clear();
        self.state.build_progress = 0.0;

        let app_name = self.state.metadata_app_name.trim();
        if app_name.is_empty() {
            self.state.build_error = Some("Application name is required.".to_string());
            return;
        }

        if self.state.payload_appimage_path.trim().is_empty() {
            self.state.build_error = Some("Payload AppImage path is required.".to_string());
            return;
        }

        let payload_path = Path::new(self.state.payload_appimage_path.trim());
        if !payload_path.is_file() {
            self.state.build_error = Some("Payload AppImage path must point to an existing file.".to_string());
            return;
        }

        if self.state.branding_logo_path.trim().is_empty() {
            self.state.build_error = Some("Logo / Icon path is required.".to_string());
            return;
        }

        if self.state.output_dir.trim().is_empty() {
            self.state.build_error = Some("Output directory is required.".to_string());
            return;
        }

        self.state.build_progress = 0.15;
        self.state.build_logs.push("Starting packaging workflow...".to_string());

        let input = PackagingInput {
            app_name: self.state.metadata_app_name.clone(),
            app_version: self.state.metadata_version.clone(),
            publisher: self.state.metadata_publisher.clone(),
            description: self.state.metadata_description.clone(),
            tagline: String::new(),
            payload_appimage_source: self.state.payload_appimage_path.clone(),
            install_path_verbatim: self.state.install_path.clone(),
            default_menu_entry: self.state.default_menu_entry,
            default_desktop_shortcut: self.state.default_desktop_shortcut,
            default_path_symlink: self.state.default_path_symlink,
            icon_source: self.state.branding_logo_path.clone(),
            logo_source: self.state.branding_logo_path.clone(),
            banner_source: self.state.branding_banner_path.clone(),
            watermark_source: self.state.branding_sidebar_image_path.clone(),
            output_dir: self.state.output_dir.clone(),
            shell_binary_source: self.state.shell_binary_source_path.clone(),
        };

        match packaging::run_packaging(input) {
            Ok(result) => {
                self.state.build_progress = 1.0;
                self.state.last_output_appimage = result.output_appimage.display().to_string();
                self.state.build_logs.extend(result.logs);
                self.state.build_logs.push("Build completed successfully.".to_string());
            }
            Err(err) => {
                self.state.build_progress = 0.0;
                self.state.build_error = Some(err.clone());
                self.state.build_logs.push(format!("Build failed: {err}"));
            }
        }
    }
}

fn payload_info_for(path: &Path) -> String {
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "(unknown)".to_string());

    match std::fs::metadata(path) {
        Ok(meta) => {
            let mb = meta.len() as f64 / (1024.0 * 1024.0);
            format!("File: {file_name} ({mb:.1} MB)")
        }
        Err(_) => format!("File: {file_name}"),
    }
}

fn value_or_placeholder<'a>(value: &'a str, placeholder: &'a str) -> &'a str {
    if value.trim().is_empty() {
        placeholder
    } else {
        value
    }
}

fn payload_filename_or_placeholder(path_value: &str) -> String {
    if path_value.trim().is_empty() {
        return "(not selected)".to_string();
    }
    Path::new(path_value)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path_value.to_string())
}

fn main() -> eframe::Result<()> {
    let native_options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Loculus Builder")
            .with_inner_size(Vec2::new(1100.0, 760.0))
            .with_min_inner_size(Vec2::new(900.0, 620.0)),
        ..Default::default()
    };

    eframe::run_native(
        "Loculus Builder",
        native_options,
        Box::new(|_cc| Ok(Box::new(BuilderApp::default()))),
    )
}
