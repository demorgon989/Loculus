use eframe::egui::{self, RichText, Vec2};
use eframe::{App, Frame, NativeOptions};

#[derive(Debug, Clone, Default)]
struct AppState {
    current_page: usize,
    payload_appimage_path: Option<String>,
    metadata_app_name: String,
    metadata_version: String,
    metadata_command_name: String,
    branding_icon_path: String,
    branding_banner_path: String,
    branding_primary_color: String,
    branding_secondary_color: String,
    build_logs: Vec<String>,
    build_progress: f32,
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
                0 => {
                    ui.label(RichText::new("Welcome").strong());
                    ui.label("Stub: intro page for the builder workflow.");
                }
                1 => self.render_payload_page(ui),
                2 => {
                    ui.label(RichText::new("Metadata").strong());
                    ui.label("Stub: app name, version, and command name fields will go here.");
                }
                3 => {
                    ui.label(RichText::new("Branding").strong());
                    ui.label("Stub: icon path and optional banner/color controls will go here.");
                }
                4 => {
                    ui.label(RichText::new("Build").strong());
                    ui.label("Stub: build trigger, progress, and packaging logs will go here.");

                    ui.separator();
                    ui.label("Planned build log area (placeholder):");
                    egui::ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
                        if self.state.build_logs.is_empty() {
                            ui.label("(no build logs yet)");
                        } else {
                            for line in &self.state.build_logs {
                                ui.label(line);
                            }
                        }
                    });
                    ui.add(
                        egui::ProgressBar::new(self.state.build_progress)
                            .show_percentage()
                            .text("Build progress (placeholder)"),
                    );
                }
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
    fn render_payload_page(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Payload").strong());
        ui.label("Choose the source AppImage that will be wrapped by the installer.");

        if ui.button("Select AppImage...").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Select source AppImage")
                .add_filter("AppImage", &["AppImage", "appimage"])
                .pick_file()
            {
                self.state.payload_appimage_path = Some(path.display().to_string());
            }
        }

        ui.add_space(8.0);
        ui.label("Selected payload path:");
        match &self.state.payload_appimage_path {
            Some(path) => {
                ui.monospace(path);
            }
            None => {
                ui.label("(none selected)");
            }
        }
    }
}

fn main() -> eframe::Result<()> {
    let native_options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Loculus Builder")
            .with_inner_size(Vec2::new(1000.0, 660.0))
            .with_min_inner_size(Vec2::new(860.0, 560.0)),
        ..Default::default()
    };

    eframe::run_native(
        "Loculus Builder",
        native_options,
        Box::new(|_cc| Ok(Box::new(BuilderApp::default()))),
    )
}
