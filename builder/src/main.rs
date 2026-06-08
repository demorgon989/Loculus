mod packaging;

use crate::packaging::PackagingInput;
use eframe::egui::{self, RichText, Vec2};
use eframe::{App, Frame, NativeOptions};

#[derive(Debug, Clone)]
struct AppState {
    current_page: usize,

    payload_appimage_path: String,
    shell_binary_source_path: String,

    metadata_app_name: String,
    metadata_version: String,
    metadata_command_name: String,
    metadata_publisher: String,
    metadata_description: String,
    metadata_tagline: String,
    metadata_install_path: String,
    default_menu_entry: bool,
    default_desktop_shortcut: bool,
    default_path_symlink: bool,

    branding_icon_path: String,
    branding_logo_path: String,
    branding_banner_path: String,
    branding_watermark_path: String,

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

            payload_appimage_path: String::new(),
            shell_binary_source_path: "target/release/installer-shell".to_string(),

            metadata_app_name: String::new(),
            metadata_version: String::new(),
            metadata_command_name: String::new(),
            metadata_publisher: String::new(),
            metadata_description: String::new(),
            metadata_tagline: String::new(),
            metadata_install_path: String::new(),
            default_menu_entry: false,
            default_desktop_shortcut: false,
            default_path_symlink: false,

            branding_icon_path: String::new(),
            branding_logo_path: String::new(),
            branding_banner_path: String::new(),
            branding_watermark_path: String::new(),

            output_dir: String::new(),

            build_logs: Vec::new(),
            build_progress: 0.0,
            build_error: None,
            last_output_appimage: String::new(),
        }
    }
}

impl AppState {
    const PAGE_COUNT: usize = 5;

    fn page_title(&self) -> &'static str {
        match self.current_page {
            0 => "Welcome",
            1 => "Payload",
            2 => "Metadata",
            3 => "Branding",
            4 => "Build",
            _ => "Unknown",
        }
    }

    fn go_back(&mut self) {
        if self.current_page > 0 {
            self.current_page -= 1;
        }
    }

    fn go_next(&mut self) {
        if self.current_page + 1 < Self::PAGE_COUNT {
            self.current_page += 1;
        }
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
                "Page {} of {} — {}",
                self.state.current_page + 1,
                AppState::PAGE_COUNT,
                self.state.page_title()
            ));
            ui.separator();

            match self.state.current_page {
                0 => self.render_welcome_page(ui),
                1 => self.render_payload_page(ui),
                2 => self.render_metadata_page(ui),
                3 => self.render_branding_page(ui),
                4 => self.render_build_page(ui),
                _ => {
                    ui.label("Invalid page index.");
                }
            }

            ui.separator();
            ui.horizontal(|ui| {
                let back_enabled = self.state.current_page > 0;
                if ui
                    .add_enabled(back_enabled, egui::Button::new("Back"))
                    .clicked()
                {
                    self.state.go_back();
                }

                let next_enabled = self.state.current_page + 1 < AppState::PAGE_COUNT;
                if ui
                    .add_enabled(next_enabled, egui::Button::new("Next"))
                    .clicked()
                {
                    self.state.go_next();
                }
            });
        });
    }
}

impl BuilderApp {
    fn render_welcome_page(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Welcome").strong());
        ui.label("This builder creates an installer AppImage by packaging:");
        ui.label("• installer shell binary");
        ui.label("• payload AppImage");
        ui.label("• manifest + branding assets");
        ui.label("• AppDir metadata and AppRun");
        ui.add_space(8.0);
        ui.label("Use Next to fill all required inputs, then run Build on page 5.");
    }

    fn render_payload_page(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Payload").strong());
        ui.label("Provide source paths used at packaging build time.");
        ui.add_space(8.0);

        ui.label("Payload AppImage source path:");
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.state.payload_appimage_path)
                    .desired_width(f32::INFINITY),
            );
            if ui.button("Browse...").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Select source AppImage")
                    .add_filter("AppImage", &["AppImage", "appimage"])
                    .pick_file()
                {
                    self.state.payload_appimage_path = path.display().to_string();
                }
            }
        });

        ui.add_space(8.0);
        ui.label("Shell binary source path (default points to workspace release target):");
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.state.shell_binary_source_path)
                    .desired_width(f32::INFINITY),
            );
            if ui.button("Browse...").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Select installer-shell binary")
                    .pick_file()
                {
                    self.state.shell_binary_source_path = path.display().to_string();
                }
            }
        });
    }

    fn render_metadata_page(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Metadata").strong());
        ui.label("These values map directly into manifest.json keys consumed by the installer shell.");
        ui.add_space(8.0);

        ui.label("App name (manifest: name):");
        ui.add(egui::TextEdit::singleline(&mut self.state.metadata_app_name).desired_width(f32::INFINITY));

        ui.label("Version (manifest: version):");
        ui.add(egui::TextEdit::singleline(&mut self.state.metadata_version).desired_width(f32::INFINITY));

        ui.label("Command name (builder metadata for later use):");
        ui.add(
            egui::TextEdit::singleline(&mut self.state.metadata_command_name)
                .desired_width(f32::INFINITY),
        );

        ui.label("Publisher (manifest: publisher):");
        ui.add(egui::TextEdit::singleline(&mut self.state.metadata_publisher).desired_width(f32::INFINITY));

        ui.label("Tagline (manifest: tagline):");
        ui.add(egui::TextEdit::singleline(&mut self.state.metadata_tagline).desired_width(f32::INFINITY));

        ui.label("Description (manifest: description):");
        ui.add(
            egui::TextEdit::multiline(&mut self.state.metadata_description)
                .desired_width(f32::INFINITY)
                .desired_rows(4),
        );

        ui.label("Install path (manifest: install_path, stored verbatim):");
        ui.add(
            egui::TextEdit::singleline(&mut self.state.metadata_install_path)
                .desired_width(f32::INFINITY),
        );

        ui.add_space(8.0);
        ui.label("Default installer options in manifest:");
        ui.checkbox(&mut self.state.default_menu_entry, "default_menu_entry");
        ui.checkbox(
            &mut self.state.default_desktop_shortcut,
            "default_desktop_shortcut",
        );
        ui.checkbox(&mut self.state.default_path_symlink, "default_path_symlink");
    }

    fn render_branding_page(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Branding").strong());
        ui.label("Set installer icon and optional shell asset images.");
        ui.add_space(8.0);

        Self::path_picker_row(
            ui,
            "Installer icon path (required, copied to <AppDir>/<safename>.png):",
            &mut self.state.branding_icon_path,
            Some(("Images", vec!["png", "jpg", "jpeg"])),
        );

        Self::path_picker_row(
            ui,
            "Logo path (optional, manifest logo -> assets/logo.png):",
            &mut self.state.branding_logo_path,
            Some(("Images", vec!["png", "jpg", "jpeg"])),
        );

        Self::path_picker_row(
            ui,
            "Banner path (optional, manifest banner -> assets/banner.png):",
            &mut self.state.branding_banner_path,
            Some(("Images", vec!["png", "jpg", "jpeg"])),
        );

        Self::path_picker_row(
            ui,
            "Watermark path (optional, manifest watermark -> assets/watermark.png):",
            &mut self.state.branding_watermark_path,
            Some(("Images", vec!["png", "jpg", "jpeg"])),
        );
    }

    fn render_build_page(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Build").strong());
        ui.label("Choose output location and run packaging.");
        ui.add_space(8.0);

        ui.label("Output directory:");
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.state.output_dir).desired_width(f32::INFINITY));
            if ui.button("Browse...").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Select output directory")
                    .pick_folder()
                {
                    self.state.output_dir = path.display().to_string();
                }
            }
        });

        if ui.button("Build installer AppImage").clicked() {
            self.run_build();
        }

        ui.add_space(8.0);
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
        ui.label("Build logs:");
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

    fn path_picker_row(
        ui: &mut egui::Ui,
        label: &str,
        value: &mut String,
        filter: Option<(&str, Vec<&str>)>,
    ) {
        ui.label(label);
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(value).desired_width(f32::INFINITY));
            if ui.button("Browse...").clicked() {
                let mut dialog = rfd::FileDialog::new();
                if let Some((name, extensions)) = &filter {
                    dialog = dialog.add_filter(*name, extensions);
                }
                if let Some(path) = dialog.pick_file() {
                    *value = path.display().to_string();
                }
            }
        });
    }

    fn run_build(&mut self) {
        self.state.build_logs.clear();
        self.state.build_error = None;
        self.state.last_output_appimage.clear();
        self.state.build_progress = 0.05;
        self.state.build_logs.push("Validating inputs...".to_string());

        let input = PackagingInput {
            app_name: self.state.metadata_app_name.clone(),
            app_version: self.state.metadata_version.clone(),
            publisher: self.state.metadata_publisher.clone(),
            description: self.state.metadata_description.clone(),
            tagline: self.state.metadata_tagline.clone(),
            payload_appimage_source: self.state.payload_appimage_path.clone(),
            install_path_verbatim: self.state.metadata_install_path.clone(),
            default_menu_entry: self.state.default_menu_entry,
            default_desktop_shortcut: self.state.default_desktop_shortcut,
            default_path_symlink: self.state.default_path_symlink,
            icon_source: self.state.branding_icon_path.clone(),
            logo_source: self.state.branding_logo_path.clone(),
            banner_source: self.state.branding_banner_path.clone(),
            watermark_source: self.state.branding_watermark_path.clone(),
            output_dir: self.state.output_dir.clone(),
            shell_binary_source: self.state.shell_binary_source_path.clone(),
        };

        self.state.build_progress = 0.2;
        self.state
            .build_logs
            .push("Running packaging workflow...".to_string());

        match packaging::run_packaging(input) {
            Ok(result) => {
                self.state.build_progress = 1.0;
                self.state.last_output_appimage = result.output_appimage.display().to_string();
                self.state.build_logs.extend(result.logs);
                self.state
                    .build_logs
                    .push("Build completed successfully.".to_string());
            }
            Err(err) => {
                self.state.build_progress = 0.0;
                self.state.build_error = Some(err.clone());
                self.state.build_logs.push(format!("Build failed: {err}"));
            }
        }
    }
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
