use eframe::egui::{self, Color32, Pos2, Rect, RichText, Stroke, TextureHandle, Vec2};
use eframe::{App, CreationContext, Frame, NativeOptions};
use image::ImageReader;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use walkdir::WalkDir;

const ACCENT: Color32 = Color32::from_rgb(35, 90, 196);
const ACCENT_DARK: Color32 = Color32::from_rgb(24, 63, 138);
const APP_CATEGORIES: &str = "Utility;";

#[derive(Debug, Clone, Deserialize, Default)]
struct ManifestData {
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    publisher: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    tagline: String,
    #[serde(default)]
    logo: String,
    #[serde(default)]
    banner: String,
    #[serde(default)]
    watermark: String,
    #[serde(default)]
    appimage: String,
    #[serde(default)]
    install_path: String,
    #[serde(default = "default_true")]
    default_menu_entry: bool,
    #[serde(default = "default_true")]
    default_desktop_shortcut: bool,
    #[serde(default = "default_true")]
    default_path_symlink: bool,
}

#[derive(Debug, Clone)]
struct LoadedManifest {
    data: ManifestData,
    path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
struct InstallOptions {
    install_path: String,
    menu_entry: bool,
    desktop_shortcut: bool,
    path_symlink: bool,
    symlink_name: String,
    generate_uninstaller: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct InstallRecord {
    app_name: String,
    safe_name: String,
    install_dir: String,
    appimage_path: String,
    icon_path: Option<String>,
    app_desktop_path: Option<String>,
    desktop_shortcut_path: Option<String>,
    symlink_path: Option<String>,
    uninstall_desktop_path: Option<String>,
    uninstaller_binary_path: String,
    uninstall_script_path: Option<String>,
    install_record_path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UninstallStep {
    Confirm,
    Removing,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WizardStep {
    Welcome,
    InstallPath,
    Integration,
    Review,
    Installing,
    Complete,
}

impl WizardStep {
    fn all() -> &'static [WizardStep] {
        &[
            WizardStep::Welcome,
            WizardStep::InstallPath,
            WizardStep::Integration,
            WizardStep::Review,
            WizardStep::Installing,
            WizardStep::Complete,
        ]
    }

    fn title(self) -> &'static str {
        match self {
            WizardStep::Welcome => "Welcome",
            WizardStep::InstallPath => "Installation Location",
            WizardStep::Integration => "Desktop Integration",
            WizardStep::Review => "Review",
            WizardStep::Installing => "Installing",
            WizardStep::Complete => "Complete",
        }
    }

    fn subtitle(self) -> &'static str {
        match self {
            WizardStep::Welcome => "Prepare to install your application.",
            WizardStep::InstallPath => "Choose where to install the bundled AppImage.",
            WizardStep::Integration => "Set desktop/menu/command-line integration options.",
            WizardStep::Review => "Confirm settings before installing.",
            WizardStep::Installing => "Applying installation steps...",
            WizardStep::Complete => "Installation finished.",
        }
    }

    fn index(self) -> usize {
        Self::all().iter().position(|s| *s == self).unwrap_or(0)
    }
}

struct InstallerApp {
    manifest: LoadedManifest,
    options: InstallOptions,
    step: WizardStep,
    logs: Vec<String>,
    error: Option<String>,
    did_install_run: bool,
    logo_texture: Option<TextureHandle>,
    watermark_texture: Option<TextureHandle>,
}

struct UninstallApp {
    record: InstallRecord,
    step: UninstallStep,
    logs: Vec<String>,
    error: Option<String>,
    did_uninstall_run: bool,
    logo_texture: Option<TextureHandle>,
    deferred_self_cleanup: Option<(PathBuf, PathBuf)>,
}

fn default_true() -> bool {
    true
}

fn main() {
    if env::args().any(|a| a == "--uninstall") {
        let record = match load_install_record_for_uninstall() {
            Ok(r) => r,
            Err(err) => {
                eprintln!("Failed to load uninstall record: {err}");
                std::process::exit(1);
            }
        };

        let window_title = format!("Uninstall {}", record.app_name);
        let native_options = NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title(window_title.clone())
                .with_inner_size(Vec2::new(860.0, 560.0))
                .with_min_inner_size(Vec2::new(760.0, 500.0)),
            ..Default::default()
        };

        let _ = eframe::run_native(
            &window_title,
            native_options,
            Box::new(move |cc| Ok(Box::new(UninstallApp::new(cc, record.clone())))),
        );
        return;
    }

    if env::args().any(|a| a == "--auto-install") {
        let manifest = load_manifest();
        let app_name = if manifest.data.name.trim().is_empty() {
            "Application".to_string()
        } else {
            manifest.data.name.clone()
        };
        let install_path = resolve_install_path_default(&manifest, &app_name);
        let options = InstallOptions {
            install_path,
            menu_entry: manifest.data.default_menu_entry,
            desktop_shortcut: manifest.data.default_desktop_shortcut,
            path_symlink: manifest.data.default_path_symlink,
            symlink_name: sanitize_name(&app_name),
            generate_uninstaller: true,
        };

        match perform_install(&manifest, &options) {
            Ok(lines) => {
                for line in lines {
                    println!("{line}");
                }
                std::process::exit(0);
            }
            Err(err) => {
                eprintln!("{err}");
                std::process::exit(1);
            }
        }
    }

    let manifest = load_manifest();
    let window_title = window_title_for_manifest(&manifest);

    let native_options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(window_title.clone())
            .with_inner_size(Vec2::new(1024.0, 680.0))
            .with_min_inner_size(Vec2::new(900.0, 620.0)),
        ..Default::default()
    };

    let _ = eframe::run_native(
        &window_title,
        native_options,
        Box::new(move |cc| Ok(Box::new(InstallerApp::new(cc, manifest.clone())))),
    );
}

impl InstallerApp {
    fn new(cc: &CreationContext<'_>, manifest: LoadedManifest) -> Self {
        apply_theme(&cc.egui_ctx);
        let app_name = if manifest.data.name.trim().is_empty() {
            "Application".to_string()
        } else {
            manifest.data.name.clone()
        };

        let install_path = resolve_install_path_default(&manifest, &app_name);

        let options = InstallOptions {
            install_path,
            menu_entry: manifest.data.default_menu_entry,
            desktop_shortcut: manifest.data.default_desktop_shortcut,
            path_symlink: manifest.data.default_path_symlink,
            symlink_name: sanitize_name(&app_name),
            generate_uninstaller: true,
        };

        let logo_texture = load_texture_from_manifest(&cc.egui_ctx, &manifest, &manifest.data.logo, "sidebar-logo");
        let watermark_texture = load_texture_from_manifest(
            &cc.egui_ctx,
            &manifest,
            &manifest.data.watermark,
            "sidebar-watermark",
        );

        Self {
            manifest,
            options,
            step: WizardStep::Welcome,
            logs: vec!["Ready to install.".to_string()],
            error: None,
            did_install_run: false,
            logo_texture,
            watermark_texture,
        }
    }

    fn app_name(&self) -> String {
        if self.manifest.data.name.trim().is_empty() {
            "Application".to_string()
        } else {
            self.manifest.data.name.clone()
        }
    }

    fn app_version(&self) -> String {
        if self.manifest.data.version.trim().is_empty() {
            "1.0.0".to_string()
        } else {
            self.manifest.data.version.clone()
        }
    }

    fn publisher(&self) -> String {
        if self.manifest.data.publisher.trim().is_empty() {
            "Unknown Publisher".to_string()
        } else {
            self.manifest.data.publisher.clone()
        }
    }

    fn next(&mut self) {
        self.error = None;
        self.step = match self.step {
            WizardStep::Welcome => WizardStep::InstallPath,
            WizardStep::InstallPath => WizardStep::Integration,
            WizardStep::Integration => WizardStep::Review,
            WizardStep::Review => {
                self.did_install_run = false;
                WizardStep::Installing
            }
            WizardStep::Installing => WizardStep::Complete,
            WizardStep::Complete => WizardStep::Complete,
        };
    }

    fn back(&mut self) {
        self.error = None;
        self.step = match self.step {
            WizardStep::Welcome => WizardStep::Welcome,
            WizardStep::InstallPath => WizardStep::Welcome,
            WizardStep::Integration => WizardStep::InstallPath,
            WizardStep::Review => WizardStep::Integration,
            WizardStep::Installing => WizardStep::Review,
            WizardStep::Complete => WizardStep::Complete,
        };
    }

    fn run_install_if_needed(&mut self) {
        if self.step != WizardStep::Installing || self.did_install_run {
            return;
        }

        self.logs.clear();
        match perform_install(&self.manifest, &self.options) {
            Ok(lines) => {
                self.logs = lines;
                self.error = None;
            }
            Err(err) => {
                self.logs.push("Installation failed".to_string());
                self.error = Some(err);
            }
        }

        self.did_install_run = true;
        self.step = WizardStep::Complete;
    }
}

impl App for InstallerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut Frame) {
        let ctx = ui.ctx().clone();
        self.run_install_if_needed();

        egui::SidePanel::left("brand")
            .exact_width(260.0)
            .frame(
                egui::Frame::new()
                    .fill(ACCENT_DARK)
                    .inner_margin(egui::Margin::same(0)),
            )
            .show(&ctx, |ui| {
                let panel_rect = ui.max_rect();
                let painter = ui.painter_at(panel_rect);

                let mut has_watermark = false;
                if let Some(tex) = &self.watermark_texture {
                    let tex_size = tex.size_vec2();
                    if tex_size.x > 0.0 && tex_size.y > 0.0 {
                        let panel_aspect = panel_rect.width() / panel_rect.height().max(1.0);
                        let tex_aspect = tex_size.x / tex_size.y;

                        let uv = if tex_aspect > panel_aspect {
                            // Texture is wider: crop horizontally
                            let visible_u = panel_aspect / tex_aspect;
                            let margin = (1.0 - visible_u) * 0.5;
                            Rect::from_min_max(Pos2::new(margin, 0.0), Pos2::new(1.0 - margin, 1.0))
                        } else {
                            // Texture is taller: crop vertically
                            let visible_v = tex_aspect / panel_aspect.max(0.001);
                            let margin = (1.0 - visible_v) * 0.5;
                            Rect::from_min_max(Pos2::new(0.0, margin), Pos2::new(1.0, 1.0 - margin))
                        };

                        painter.image(tex.id(), panel_rect, uv, Color32::WHITE);
                        has_watermark = true;
                    }
                }

                if has_watermark {
                    // Neutral dark scrim for readability (no blue tint over watermark)
                    painter.rect_filled(
                        panel_rect,
                        0.0,
                        Color32::from_rgba_premultiplied(0, 0, 0, 96),
                    );
                } else {
                    // Fallback brand background if no watermark is provided
                    painter.rect_filled(panel_rect, 0.0, ACCENT_DARK);
                }

                ui.horizontal(|ui| {
                    ui.add_space(18.0);
                    ui.vertical(|ui| {
                        ui.add_space(18.0);
                        ui.label(
                            RichText::new(format!("{} Installer", self.app_name()))
                                .color(Color32::WHITE)
                                .size(22.0)
                                .strong(),
                        );
                        ui.label(
                            RichText::new(format!("Version {}", self.app_version()))
                                .color(Color32::from_rgb(220, 228, 246)),
                        );
                    });
                });
            });

        egui::CentralPanel::default().show(&ctx, |ui| {
            let subtitle_color = if ui.visuals().dark_mode {
                Color32::from_rgb(210, 220, 236)
            } else {
                Color32::from_rgb(85, 93, 112)
            };
            let pending_step_color = if ui.visuals().dark_mode {
                Color32::from_rgb(190, 198, 214)
            } else {
                Color32::from_rgb(126, 136, 157)
            };

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.heading(self.step.title());
                    ui.label(RichText::new(self.step.subtitle()).color(subtitle_color));
                });

                if let Some(tex) = &self.logo_texture {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        let size = fit_size_preserving_aspect(tex.size_vec2(), 112.0, 72.0);
                        ui.image((tex.id(), size));
                    });
                }
            });
            ui.add_space(8.0);

            ui.horizontal_wrapped(|ui| {
                for step in WizardStep::all() {
                    let idx = step.index();
                    let current = self.step.index();
                    let (text, color) = if idx < current {
                        (format!("✓ {}", step.title()), Color32::from_rgb(16, 132, 84))
                    } else if idx == current {
                        (format!("● {}", step.title()), ACCENT)
                    } else {
                        (format!("○ {}", step.title()), pending_step_color)
                    };
                    ui.label(RichText::new(text).color(color));
                    if idx < WizardStep::all().len() - 1 {
                        ui.label(RichText::new("—").color(Color32::LIGHT_GRAY));
                    }
                }
            });

            ui.separator();
            ui.add_space(8.0);

            match self.step {
                WizardStep::Welcome => {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.label(RichText::new(format!("Welcome to {}", self.app_name())).strong());
                        ui.label(format!("Publisher: {}", self.publisher()));
                        ui.label(if self.manifest.data.description.trim().is_empty() {
                            "This installer will install the bundled AppImage and set up integrations.".to_string()
                        } else {
                            self.manifest.data.description.clone()
                        });
                        ui.add_space(6.0);
                        ui.label("This wizard will:");
                        ui.label("• Copy the bundled AppImage to your install path");
                        ui.label("• Create desktop/menu integration");
                        ui.label("• Optionally create a command symlink");
                        ui.label("• Generate an uninstaller script");
                    });
                }
                WizardStep::InstallPath => {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.label("Installation Path:");
                        ui.add(egui::TextEdit::singleline(&mut self.options.install_path).desired_width(f32::INFINITY));
                        ui.label(RichText::new("Examples: ~/.local/bin, ~/Applications, /opt").small());
                    });
                }
                WizardStep::Integration => {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.checkbox(&mut self.options.menu_entry, "Add to applications menu");
                        ui.checkbox(&mut self.options.desktop_shortcut, "Create desktop shortcut");
                        ui.checkbox(&mut self.options.path_symlink, "Create command-line symlink");
                        ui.horizontal(|ui| {
                            ui.label("Command name:");
                            ui.add_enabled(
                                self.options.path_symlink,
                                egui::TextEdit::singleline(&mut self.options.symlink_name).desired_width(220.0),
                            );
                        });
                        ui.checkbox(&mut self.options.generate_uninstaller, "Generate uninstaller");
                    });
                }
                WizardStep::Review => {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.label(RichText::new("Review settings").strong());
                        ui.label(format!("Application: {}", self.app_name()));
                        ui.label(format!("Version: {}", self.app_version()));
                        ui.label(format!("Install path: {}", self.options.install_path));
                        ui.label(format!("Menu entry: {}", yes_no(self.options.menu_entry)));
                        ui.label(format!(
                            "Desktop shortcut: {}",
                            yes_no(self.options.desktop_shortcut)
                        ));
                        ui.label(format!("PATH symlink: {}", yes_no(self.options.path_symlink)));
                        ui.label(format!(
                            "Command name: {}",
                            if self.options.symlink_name.trim().is_empty() {
                                "(default)"
                            } else {
                                &self.options.symlink_name
                            }
                        ));
                        ui.label(format!(
                            "Generate uninstaller: {}",
                            yes_no(self.options.generate_uninstaller)
                        ));
                    });
                }
                WizardStep::Installing => {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.add(egui::Spinner::new());
                        ui.label("Running installation steps...");
                    });
                }
                WizardStep::Complete => {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        if let Some(err) = &self.error {
                            ui.colored_label(Color32::from_rgb(184, 36, 36), "Installation failed:");
                            ui.label(err);
                        } else {
                            ui.colored_label(Color32::from_rgb(16, 132, 84), "Installation completed.");
                        }

                        ui.separator();
                        ui.label("Log:");
                        egui::ScrollArea::vertical().max_height(250.0).show(ui, |ui| {
                            for line in &self.logs {
                                ui.label(line);
                            }
                        });
                    });
                }
            }

            ui.add_space(10.0);
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }

                ui.add_space(8.0);
                let back_enabled = !matches!(self.step, WizardStep::Welcome | WizardStep::Installing | WizardStep::Complete);
                if ui
                    .add_enabled(back_enabled, egui::Button::new("Back"))
                    .clicked()
                {
                    self.back();
                }

                ui.add_space(8.0);
                let next_label = match self.step {
                    WizardStep::Review => "Install",
                    WizardStep::Complete => "Finish",
                    WizardStep::Installing => "Installing...",
                    _ => "Next",
                };
                let next_enabled = !matches!(self.step, WizardStep::Installing);

                if ui
                    .add_enabled(next_enabled, egui::Button::new(next_label))
                    .clicked()
                {
                    if self.step == WizardStep::Complete {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    } else {
                        self.next();
                    }
                }
            });
        });
    }
}

impl UninstallApp {
    fn new(cc: &CreationContext<'_>, record: InstallRecord) -> Self {
        apply_theme(&cc.egui_ctx);
        let logo_texture = record
            .icon_path
            .as_ref()
            .and_then(|p| load_texture_from_path(&cc.egui_ctx, Path::new(p), "uninstall-logo"));

        Self {
            record,
            step: UninstallStep::Confirm,
            logs: Vec::new(),
            error: None,
            did_uninstall_run: false,
            logo_texture,
            deferred_self_cleanup: None,
        }
    }

    fn run_uninstall_if_needed(&mut self) {
        if self.step != UninstallStep::Removing || self.did_uninstall_run {
            return;
        }
        let outcome = perform_uninstall_from_record(&self.record);
        self.logs = outcome.logs;
        self.error = outcome.error;
        self.deferred_self_cleanup = outcome.deferred_self_cleanup;
        if let Some((binary, install_dir)) = &self.deferred_self_cleanup {
            spawn_deferred_self_cleanup(binary, install_dir);
        }
        self.did_uninstall_run = true;
        self.step = UninstallStep::Done;
    }

    fn removal_items(&self) -> Vec<String> {
        let mut items = vec![
            format!("Application file: {}", self.record.appimage_path),
            format!("Install directory (if empty): {}", self.record.install_dir),
        ];
        if let Some(v) = &self.record.app_desktop_path {
            items.push(format!("Menu entry: {v}"));
        }
        if let Some(v) = &self.record.uninstall_desktop_path {
            items.push(format!("Uninstall menu entry: {v}"));
        }
        if let Some(v) = &self.record.symlink_path {
            items.push(format!("Command symlink: {v}"));
        }
        if let Some(v) = &self.record.desktop_shortcut_path {
            items.push(format!("Desktop shortcut (if present): {v}"));
        }
        if let Some(v) = &self.record.icon_path {
            items.push(format!("Icon file: {v}"));
        }
        items.push(format!("GUI uninstaller binary: {}", self.record.uninstaller_binary_path));
        items
    }
}

impl App for UninstallApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut Frame) {
        let ctx = ui.ctx().clone();
        self.run_uninstall_if_needed();

        egui::CentralPanel::default().show(&ctx, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.heading(match self.step {
                        UninstallStep::Confirm => format!("Remove {}?", self.record.app_name),
                        UninstallStep::Removing => "Removing application...".to_string(),
                        UninstallStep::Done => format!("Uninstall {}", self.record.app_name),
                    });
                    ui.label(
                        RichText::new(match self.step {
                            UninstallStep::Confirm => "Review what will be removed.",
                            UninstallStep::Removing => "Applying removal steps...",
                            UninstallStep::Done => "Uninstall operation finished.",
                        })
                        .color(Color32::from_rgb(210, 220, 236)),
                    );
                });

                if let Some(tex) = &self.logo_texture {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        let size = fit_size_preserving_aspect(tex.size_vec2(), 96.0, 64.0);
                        ui.image((tex.id(), size));
                    });
                }
            });

            ui.separator();
            ui.add_space(8.0);

            match self.step {
                UninstallStep::Confirm => {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.label(RichText::new("The following items will be removed:").strong());
                        ui.add_space(6.0);
                        for item in self.removal_items() {
                            ui.label(format!("• {item}"));
                        }
                    });
                }
                UninstallStep::Removing => {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.add(egui::Spinner::new());
                        ui.label("Removing files and entries...");
                        ui.separator();
                        egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                            for line in &self.logs {
                                ui.label(line);
                            }
                        });
                    });
                }
                UninstallStep::Done => {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        if let Some(err) = &self.error {
                            ui.colored_label(Color32::from_rgb(184, 36, 36), "Uninstall completed with issues:");
                            ui.label(err);
                        } else {
                            ui.colored_label(Color32::from_rgb(16, 132, 84), "Uninstall completed successfully.");
                        }
                        ui.separator();
                        ui.label("Log:");
                        egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                            for line in &self.logs {
                                ui.label(line);
                            }
                        });
                    });
                }
            }

            ui.add_space(10.0);
            ui.separator();
            ui.horizontal(|ui| match self.step {
                UninstallStep::Confirm => {
                    if ui.button("Cancel").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    ui.add_space(8.0);
                    if ui.button("Uninstall").clicked() {
                        self.step = UninstallStep::Removing;
                    }
                }
                UninstallStep::Removing => {
                    ui.add_enabled(false, egui::Button::new("Removing..."));
                }
                UninstallStep::Done => {
                    if ui.button("Close").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            });
        });
    }
}

fn apply_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    let dark_mode = style.visuals.dark_mode;

    style.spacing.item_spacing = Vec2::new(10.0, 10.0);
    style.spacing.button_padding = Vec2::new(16.0, 10.0);
    style.visuals.window_corner_radius = 10.into();
    style.visuals.widgets.inactive.corner_radius = 8.into();
    style.visuals.widgets.active.corner_radius = 8.into();
    style.visuals.widgets.hovered.corner_radius = 8.into();

    if dark_mode {
        // Ensure right-side text remains readable on dark backgrounds
        style.visuals.override_text_color = Some(Color32::from_rgb(234, 240, 252));
        style.visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(28, 31, 39);
        style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(228, 235, 248));

        // High-contrast base state for controls
        style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(39, 44, 57);
        style.visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(39, 44, 57);
        style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.4, Color32::from_rgb(146, 162, 192));
        style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.3, Color32::from_rgb(232, 239, 252));

        // Strong hover feedback
        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(56, 69, 101);
        style.visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(56, 69, 101);
        style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.9, ACCENT);
        style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.4, Color32::from_rgb(240, 245, 255));

        // Strong active/checked/focused feedback
        style.visuals.widgets.active.bg_fill = ACCENT;
        style.visuals.widgets.active.weak_bg_fill = ACCENT;
        style.visuals.widgets.active.bg_stroke = Stroke::new(2.0, Color32::from_rgb(132, 170, 255));
        style.visuals.widgets.active.fg_stroke = Stroke::new(1.7, Color32::WHITE);

        style.visuals.widgets.open.bg_fill = Color32::from_rgb(66, 82, 121);
        style.visuals.widgets.open.weak_bg_fill = Color32::from_rgb(66, 82, 121);
        style.visuals.widgets.open.bg_stroke = Stroke::new(1.9, ACCENT);
        style.visuals.widgets.open.fg_stroke = Stroke::new(1.4, Color32::from_rgb(242, 247, 255));

        style.visuals.selection.bg_fill = ACCENT;
        style.visuals.selection.stroke = Stroke::new(1.7, Color32::WHITE);
        style.visuals.hyperlink_color = Color32::from_rgb(154, 190, 255);
        style.visuals.window_fill = Color32::from_rgb(23, 25, 32);
        style.visuals.extreme_bg_color = Color32::from_rgb(18, 20, 26);
    } else {
        style.visuals.override_text_color = Some(Color32::from_rgb(22, 30, 46));
        style.visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(252, 253, 255);
        style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(36, 43, 58));

        // High-contrast base state for controls
        style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(246, 248, 252);
        style.visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(246, 248, 252);
        style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.4, Color32::from_rgb(120, 133, 160));
        style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.2, Color32::from_rgb(24, 34, 53));

        // Strong hover feedback
        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(224, 235, 255);
        style.visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(224, 235, 255);
        style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.8, ACCENT);
        style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.4, Color32::from_rgb(14, 24, 42));

        // Strong active/checked/focused feedback
        style.visuals.widgets.active.bg_fill = ACCENT;
        style.visuals.widgets.active.weak_bg_fill = ACCENT;
        style.visuals.widgets.active.bg_stroke = Stroke::new(2.0, ACCENT_DARK);
        style.visuals.widgets.active.fg_stroke = Stroke::new(1.6, Color32::WHITE);

        style.visuals.widgets.open.bg_fill = Color32::from_rgb(214, 228, 255);
        style.visuals.widgets.open.weak_bg_fill = Color32::from_rgb(214, 228, 255);
        style.visuals.widgets.open.bg_stroke = Stroke::new(1.8, ACCENT);
        style.visuals.widgets.open.fg_stroke = Stroke::new(1.3, Color32::from_rgb(15, 27, 48));

        // Selection/focus emphasis
        style.visuals.selection.bg_fill = ACCENT;
        style.visuals.selection.stroke = Stroke::new(1.6, Color32::WHITE);
        style.visuals.hyperlink_color = ACCENT_DARK;
        style.visuals.window_fill = Color32::from_rgb(252, 253, 255);
        style.visuals.extreme_bg_color = Color32::from_rgb(244, 246, 250);
    }

    ctx.set_style(style);
}

fn yes_no(v: bool) -> &'static str {
    if v {
        "Yes"
    } else {
        "No"
    }
}

fn home_dir() -> PathBuf {
    env::var("HOME").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("."))
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        return home_dir().join(rest);
    }
    if path == "~" {
        return home_dir();
    }
    PathBuf::from(path)
}

fn sanitize_name(raw: &str) -> String {
    let filtered: String = raw
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if filtered.is_empty() {
        "app".to_string()
    } else {
        filtered
    }
}

fn default_install_path_for(app_name: &str) -> String {
    format!("~/.local/lib/{}", sanitize_name(app_name))
}

fn resolve_install_path_default(manifest: &LoadedManifest, app_name: &str) -> String {
    if manifest.data.install_path.trim().is_empty() {
        return default_install_path_for(app_name);
    }

    manifest
        .data
        .install_path
        .replace("<safe_app_name>", &sanitize_name(app_name))
}

fn resolve_manifest_asset_path(manifest: &LoadedManifest, asset: &str) -> Option<PathBuf> {
    if asset.trim().is_empty() {
        return None;
    }
    let p = PathBuf::from(asset);
    if p.is_absolute() {
        return Some(p);
    }
    if let Some(manifest_path) = &manifest.path
        && let Some(parent) = manifest_path.parent()
    {
        return Some(parent.join(p));
    }
    Some(p)
}

fn load_texture_from_manifest(
    ctx: &egui::Context,
    manifest: &LoadedManifest,
    asset_path: &str,
    texture_name: &str,
) -> Option<TextureHandle> {
    let logo_path = resolve_manifest_asset_path(manifest, asset_path)?;
    let img = ImageReader::open(logo_path).ok()?.decode().ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let pixels = img.as_raw();
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels);
    Some(ctx.load_texture(
        texture_name,
        color_image,
        egui::TextureOptions::LINEAR,
    ))
}

fn fit_size_preserving_aspect(src: Vec2, max_w: f32, max_h: f32) -> Vec2 {
    if src.x <= 0.0 || src.y <= 0.0 {
        return Vec2::new(max_w, max_h);
    }
    let scale = (max_w / src.x).min(max_h / src.y);
    src * scale
}

fn window_title_for_manifest(manifest: &LoadedManifest) -> String {
    let name = manifest.data.name.trim();
    if name.is_empty() {
        "Installer".to_string()
    } else {
        format!("{} Installer", name)
    }
}

fn find_manifest_path() -> Option<PathBuf> {
    if let Ok(p) = env::var("APPIMAGE_MANIFEST_PATH") {
        let path = PathBuf::from(p);
        if path.exists() {
            return Some(path);
        }
    }

    let mut candidates = Vec::new();
    if let Ok(exe) = env::current_exe()
        && let Some(dir) = exe.parent()
    {
        candidates.push(dir.join("manifest.json"));
        candidates.push(dir.join("..").join("manifest.json"));
    }
    candidates.push(PathBuf::from("manifest.json"));

    candidates.into_iter().find(|p| p.exists())
}

fn load_manifest() -> LoadedManifest {
    if let Some(path) = find_manifest_path()
        && let Ok(raw) = fs::read_to_string(&path)
        && let Ok(data) = serde_json::from_str::<ManifestData>(&raw)
    {
        return LoadedManifest {
            data,
            path: Some(path),
        };
    }

    LoadedManifest {
        data: ManifestData {
            name: "Application".to_string(),
            version: "1.0.0".to_string(),
            publisher: "Unknown Publisher".to_string(),
            description: "Manifest not found; running with defaults.".to_string(),
            tagline: String::new(),
            logo: String::new(),
            banner: String::new(),
            watermark: String::new(),
            appimage: String::new(),
            install_path: default_install_path_for("Application"),
            default_menu_entry: true,
            default_desktop_shortcut: true,
            default_path_symlink: true,
        },
        path: None,
    }
}

fn resolve_payload_path(manifest: &LoadedManifest) -> Option<PathBuf> {
    if manifest.data.appimage.trim().is_empty() {
        return None;
    }
    let p = PathBuf::from(&manifest.data.appimage);
    if p.is_absolute() {
        return Some(p);
    }
    if let Some(manifest_path) = &manifest.path
        && let Some(parent) = manifest_path.parent()
    {
        return Some(parent.join(p));
    }
    Some(p)
}

fn perform_install(manifest: &LoadedManifest, options: &InstallOptions) -> Result<Vec<String>, String> {
    let mut logs = Vec::new();

    let raw_name = if manifest.data.name.trim().is_empty() {
        "Application"
    } else {
        &manifest.data.name
    };
    let safe_name = sanitize_name(raw_name);
    let install_path = expand_tilde(&options.install_path);

    logs.push(format!("Installing {}", raw_name));
    fs::create_dir_all(&install_path).map_err(|e| format!("Failed to create install directory: {e}"))?;
    logs.push(format!("Created/verified install dir: {}", install_path.display()));

    let payload = resolve_payload_path(manifest).ok_or_else(|| "No payload AppImage path found in manifest".to_string())?;
    if !payload.exists() {
        return Err(format!("Bundled payload AppImage not found: {}", payload.display()));
    }

    let target_path = install_path.join(format!("{safe_name}.AppImage"));
    fs::copy(&payload, &target_path).map_err(|e| format!("Failed to copy payload: {e}"))?;

    let mut perms = fs::metadata(&target_path)
        .map_err(|e| format!("Failed to stat installed AppImage: {e}"))?
        .permissions();
    perms.set_mode(perms.mode() | 0o111);
    fs::set_permissions(&target_path, perms).map_err(|e| format!("Failed to chmod +x installed AppImage: {e}"))?;
    logs.push(format!("Installed payload to: {}", target_path.display()));

    let icon_path = extract_icon(&target_path, &install_path, &safe_name);
    if icon_path.is_some() {
        logs.push("Extracted app icon from payload".to_string());
    } else {
        logs.push("Icon extraction skipped/failed (non-fatal)".to_string());
    }

    let desktop_file = if options.menu_entry || options.desktop_shortcut {
        Some(create_desktop_entry(raw_name, &safe_name, &target_path, icon_path.as_deref())?)
    } else {
        None
    };

    if options.menu_entry {
        if let Some(df) = &desktop_file {
            register_menu_entry(df);
            logs.push("Registered menu entry".to_string());
        }
    }

    let desktop_shortcut_path = if options.desktop_shortcut {
        if let Some(df) = &desktop_file {
            let created = create_desktop_shortcut(df);
            if created.is_some() {
                logs.push("Created desktop shortcut".to_string());
            }
            created
        } else {
            None
        }
    } else {
        None
    };

    let symlink_target = if options.path_symlink {
        let link_name = if options.symlink_name.trim().is_empty() {
            safe_name.clone()
        } else {
            sanitize_name(&options.symlink_name)
        };
        let symlink_path = create_path_symlink(&target_path, &link_name)?;
        logs.push(format!("Created PATH symlink: {}", symlink_path.display()));
        Some(symlink_path)
    } else {
        None
    };

    let (uninstaller_binary_path, uninstaller_size) = copy_uninstaller_binary(&install_path, &safe_name)?;
    logs.push(format!(
        "Installed GUI uninstaller binary: {} ({})",
        uninstaller_binary_path.display(),
        human_size(uninstaller_size)
    ));

    let uninstall_desktop_path = create_uninstall_desktop_entry(
        raw_name,
        &safe_name,
        &uninstaller_binary_path,
        icon_path.as_deref(),
    )?;
    register_menu_entry(&uninstall_desktop_path);
    logs.push(format!(
        "Registered uninstall launcher: {}",
        uninstall_desktop_path.display()
    ));

    let record_path = install_path.join(".appimage-installer-record.json");
    let uninstall_script_path = if options.generate_uninstaller {
        let script_path = generate_uninstaller(
            &install_path,
            &safe_name,
            raw_name,
            &target_path,
            desktop_file.as_deref(),
            icon_path.as_deref(),
            symlink_target.as_deref(),
            desktop_shortcut_path.as_deref(),
            Some(&uninstall_desktop_path),
            &uninstaller_binary_path,
            &record_path,
        )?;
        logs.push("Generated uninstaller script".to_string());
        Some(script_path)
    } else {
        None
    };

    let install_record = InstallRecord {
        app_name: raw_name.to_string(),
        safe_name: safe_name.clone(),
        install_dir: install_path.to_string_lossy().to_string(),
        appimage_path: target_path.to_string_lossy().to_string(),
        icon_path: icon_path.as_ref().map(|p| p.to_string_lossy().to_string()),
        app_desktop_path: desktop_file.as_ref().map(|p| p.to_string_lossy().to_string()),
        desktop_shortcut_path: desktop_shortcut_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string()),
        symlink_path: symlink_target.as_ref().map(|p| p.to_string_lossy().to_string()),
        uninstall_desktop_path: Some(uninstall_desktop_path.to_string_lossy().to_string()),
        uninstaller_binary_path: uninstaller_binary_path.to_string_lossy().to_string(),
        uninstall_script_path: uninstall_script_path.as_ref().map(|p| p.to_string_lossy().to_string()),
        install_record_path: Some(record_path.to_string_lossy().to_string()),
    };
    write_install_record(&record_path, &install_record)?;
    logs.push(format!("Wrote install record: {}", record_path.display()));

    refresh_desktop_db();
    logs.push("Refreshed desktop databases (best effort)".to_string());
    logs.push("Installation complete".to_string());

    Ok(logs)
}

fn extract_icon(target_path: &Path, install_path: &Path, safe_name: &str) -> Option<PathBuf> {
    let _ = Command::new(target_path)
        .arg("--appimage-extract")
        .current_dir(install_path)
        .output();

    let squash = install_path.join("squashfs-root");
    if !squash.exists() {
        return None;
    }

    let mut found: Option<PathBuf> = None;
    for entry in WalkDir::new(&squash).into_iter().flatten() {
        if entry.file_type().is_file()
            && let Some(ext) = entry.path().extension().and_then(|s| s.to_str())
            && ["png", "svg", "xpm"].contains(&ext)
        {
            found = Some(entry.path().to_path_buf());
            break;
        }
    }

    let icon_dest = if let Some(src) = found {
        let ext = src.extension().and_then(|s| s.to_str()).unwrap_or("png");
        let dest = install_path.join(format!("{safe_name}.{ext}"));
        if fs::copy(&src, &dest).is_ok() {
            Some(dest)
        } else {
            None
        }
    } else {
        None
    };

    let _ = fs::remove_dir_all(squash);
    icon_dest
}

fn create_desktop_entry(
    raw_name: &str,
    safe_name: &str,
    target_path: &Path,
    icon_path: Option<&Path>,
) -> Result<PathBuf, String> {
    let apps_dir = home_dir().join(".local/share/applications");
    fs::create_dir_all(&apps_dir).map_err(|e| format!("Failed to create applications dir: {e}"))?;

    let desktop_file = apps_dir.join(format!("{safe_name}.desktop"));
    let mut content = format!(
        "[Desktop Entry]\nName={}\nExec={}\nType=Application\nCategories={}\nTerminal=false\n",
        raw_name,
        target_path.display(),
        APP_CATEGORIES,
    );
    if let Some(icon) = icon_path {
        content.push_str(&format!("Icon={}\n", icon.display()));
    }

    fs::write(&desktop_file, content).map_err(|e| format!("Failed to write desktop file: {e}"))?;
    Ok(desktop_file)
}

fn register_menu_entry(desktop_file: &Path) {
    let _ = Command::new("xdg-desktop-menu")
        .args(["install", "--noupdate"])
        .arg(desktop_file)
        .output();
}

fn desktop_shortcut_target(desktop_file: &Path) -> Option<PathBuf> {
    let desktop_dir = home_dir().join("Desktop");
    if !desktop_dir.exists() {
        return None;
    }
    Some(desktop_dir.join(
        desktop_file
            .file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("app.desktop")),
    ))
}

fn create_desktop_shortcut(desktop_file: &Path) -> Option<PathBuf> {
    let dest = desktop_shortcut_target(desktop_file)?;
    let _ = fs::remove_file(&dest);
    if std::os::unix::fs::symlink(desktop_file, &dest).is_err() {
        let _ = fs::copy(desktop_file, &dest);
    }
    Some(dest)
}

fn create_path_symlink(target: &Path, name: &str) -> Result<PathBuf, String> {
    let local_bin = home_dir().join(".local/bin");
    fs::create_dir_all(&local_bin).map_err(|e| format!("Failed to create ~/.local/bin: {e}"))?;
    let symlink = local_bin.join(name);
    let _ = fs::remove_file(&symlink);
    std::os::unix::fs::symlink(target, &symlink)
        .map_err(|e| format!("Failed to create symlink {}: {e}", symlink.display()))?;
    Ok(symlink)
}

#[allow(clippy::too_many_arguments)]
fn generate_uninstaller(
    install_path: &Path,
    safe_name: &str,
    raw_name: &str,
    target_path: &Path,
    desktop_file: Option<&Path>,
    icon_path: Option<&Path>,
    symlink_path: Option<&Path>,
    desktop_shortcut_path: Option<&Path>,
    uninstall_desktop_path: Option<&Path>,
    gui_uninstaller_path: &Path,
    install_record_path: &Path,
) -> Result<PathBuf, String> {
    let script_path = install_path.join(format!("uninstall-{safe_name}.sh"));

    let desktop_rm = desktop_file
        .map(|p| format!("rm -f '{}'\n", p.display()))
        .unwrap_or_default();
    let icon_rm = icon_path
        .map(|p| format!("rm -f '{}'\n", p.display()))
        .unwrap_or_default();
    let symlink_rm = symlink_path
        .map(|p| format!("rm -f '{}'\n", p.display()))
        .unwrap_or_default();
    let desktop_shortcut_rm = desktop_shortcut_path
        .map(|p| {
            format!(
                "if [ -L '{0}' ] || [ -e '{0}' ]; then rm -f '{0}'; fi\n",
                p.display()
            )
        })
        .unwrap_or_default();
    let uninstall_desktop_rm = uninstall_desktop_path
        .map(|p| format!("rm -f '{}'\n", p.display()))
        .unwrap_or_default();

    let script = format!(
        "#!/bin/bash\nrm -f '{app}'\n{desktop}{icon}{symlink}{desktop_shortcut}{uninstall_desktop}rm -f '{record}'\nrm -f '{gui_uninstaller}'\nupdate-desktop-database ~/.local/share/applications 2>/dev/null || true\nrm -f '{self_script}'\nrmdir '{install_dir}' 2>/dev/null || true\necho 'Uninstalled {name}'\n",
        app = target_path.display(),
        desktop = desktop_rm,
        icon = icon_rm,
        symlink = symlink_rm,
        desktop_shortcut = desktop_shortcut_rm,
        uninstall_desktop = uninstall_desktop_rm,
        record = install_record_path.display(),
        gui_uninstaller = gui_uninstaller_path.display(),
        install_dir = install_path.display(),
        self_script = script_path.display(),
        name = raw_name
    );

    fs::write(&script_path, script).map_err(|e| format!("Failed to write uninstaller script: {e}"))?;
    let mut perms = fs::metadata(&script_path)
        .map_err(|e| format!("Failed to stat uninstaller script: {e}"))?
        .permissions();
    perms.set_mode(perms.mode() | 0o111);
    fs::set_permissions(&script_path, perms)
        .map_err(|e| format!("Failed to chmod uninstaller script: {e}"))?;
    Ok(script_path)
}

fn copy_uninstaller_binary(install_path: &Path, safe_name: &str) -> Result<(PathBuf, u64), String> {
    let source = env::current_exe().map_err(|e| format!("Failed to resolve current executable: {e}"))?;
    let target = install_path.join(format!("uninstall-{safe_name}"));
    fs::copy(&source, &target).map_err(|e| format!("Failed to copy GUI uninstaller binary: {e}"))?;
    let mut perms = fs::metadata(&target)
        .map_err(|e| format!("Failed to stat GUI uninstaller binary: {e}"))?
        .permissions();
    perms.set_mode(perms.mode() | 0o111);
    fs::set_permissions(&target, perms)
        .map_err(|e| format!("Failed to chmod GUI uninstaller binary: {e}"))?;
    let size = fs::metadata(&target)
        .map_err(|e| format!("Failed to stat GUI uninstaller binary size: {e}"))?
        .len();
    Ok((target, size))
}

fn create_uninstall_desktop_entry(
    raw_name: &str,
    safe_name: &str,
    uninstaller_binary_path: &Path,
    icon_path: Option<&Path>,
) -> Result<PathBuf, String> {
    let apps_dir = home_dir().join(".local/share/applications");
    fs::create_dir_all(&apps_dir).map_err(|e| format!("Failed to create applications dir: {e}"))?;
    let desktop_path = apps_dir.join(format!("{safe_name}-uninstall.desktop"));

    let mut content = format!(
        "[Desktop Entry]\nName=Uninstall {}\nExec=\"{}\" --uninstall\nType=Application\nCategories={}\nTerminal=false\n",
        raw_name,
        uninstaller_binary_path.display(),
        APP_CATEGORIES,
    );
    if let Some(icon) = icon_path {
        content.push_str(&format!("Icon={}\n", icon.display()));
    }

    fs::write(&desktop_path, content)
        .map_err(|e| format!("Failed to write uninstall desktop entry: {e}"))?;
    Ok(desktop_path)
}

fn write_install_record(path: &Path, record: &InstallRecord) -> Result<(), String> {
    let serialized = serde_json::to_string_pretty(record)
        .map_err(|e| format!("Failed to serialize install record: {e}"))?;
    fs::write(path, serialized).map_err(|e| format!("Failed to write install record: {e}"))
}

fn human_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

fn refresh_desktop_db() {
    let apps_dir = home_dir().join(".local/share/applications");
    let icons_dir = home_dir().join(".local/share/icons");

    let _ = Command::new("update-desktop-database").arg(apps_dir).output();
    let _ = Command::new("gtk-update-icon-cache").arg(icons_dir).output();
}

fn load_install_record_for_uninstall() -> Result<InstallRecord, String> {
    let mut candidates = Vec::new();
    if let Ok(p) = env::var("APPIMAGE_INSTALL_RECORD_PATH") {
        candidates.push(PathBuf::from(p));
    }
    if let Ok(exe) = env::current_exe()
        && let Some(dir) = exe.parent()
    {
        candidates.push(dir.join(".appimage-installer-record.json"));
    }
    candidates.push(PathBuf::from(".appimage-installer-record.json"));

    for path in candidates {
        if !path.exists() {
            continue;
        }
        let raw = fs::read_to_string(&path)
            .map_err(|e| format!("Failed reading install record {}: {e}", path.display()))?;
        let mut record = serde_json::from_str::<InstallRecord>(&raw)
            .map_err(|e| format!("Failed parsing install record {}: {e}", path.display()))?;
        if record.install_record_path.is_none() {
            record.install_record_path = Some(path.to_string_lossy().to_string());
        }
        return Ok(record);
    }

    Err("No install record found for uninstall mode".to_string())
}

struct UninstallOutcome {
    logs: Vec<String>,
    error: Option<String>,
    deferred_self_cleanup: Option<(PathBuf, PathBuf)>,
}

fn perform_uninstall_from_record(record: &InstallRecord) -> UninstallOutcome {
    let mut logs = Vec::new();
    let mut errs = Vec::new();

    let install_dir = PathBuf::from(&record.install_dir);
    let appimage = PathBuf::from(&record.appimage_path);
    remove_file_exact(&appimage, "Application AppImage", &mut logs, &mut errs);

    if let Some(path) = &record.app_desktop_path {
        let p = PathBuf::from(path);
        unregister_menu_entry(&p);
        remove_file_exact(&p, "Application menu entry", &mut logs, &mut errs);
    }

    if let Some(path) = &record.desktop_shortcut_path {
        remove_file_exact(Path::new(path), "Desktop shortcut", &mut logs, &mut errs);
    }

    if let Some(path) = &record.symlink_path {
        remove_file_exact(Path::new(path), "Command symlink", &mut logs, &mut errs);
    }

    if let Some(path) = &record.icon_path {
        remove_file_exact(Path::new(path), "Application icon", &mut logs, &mut errs);
    }

    if let Some(path) = &record.uninstall_desktop_path {
        let p = PathBuf::from(path);
        unregister_menu_entry(&p);
        remove_file_exact(&p, "Uninstall menu entry", &mut logs, &mut errs);
    }

    if let Some(path) = &record.uninstall_script_path {
        remove_file_exact(Path::new(path), "Fallback uninstall script", &mut logs, &mut errs);
    }

    if let Some(path) = &record.install_record_path {
        remove_file_exact(Path::new(path), "Install record", &mut logs, &mut errs);
    }

    refresh_desktop_db();
    logs.push("Refreshed desktop database".to_string());

    let self_binary = PathBuf::from(&record.uninstaller_binary_path);
    let current_exe = env::current_exe().ok();
    let deferred_self_cleanup = if current_exe.as_ref() == Some(&self_binary) {
        logs.push("Deferring self-binary removal until process exits".to_string());
        Some((self_binary.clone(), install_dir.clone()))
    } else {
        remove_file_exact(&self_binary, "GUI uninstaller binary", &mut logs, &mut errs);
        remove_dir_if_empty(&install_dir, "Install directory", &mut logs, &mut errs);
        None
    };

    let error = if errs.is_empty() {
        None
    } else {
        Some(errs.join("\n"))
    };

    UninstallOutcome {
        logs,
        error,
        deferred_self_cleanup,
    }
}

fn remove_file_exact(path: &Path, label: &str, logs: &mut Vec<String>, errs: &mut Vec<String>) {
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            logs.push(format!("{label}: not present ({})", path.display()));
            return;
        }
        Err(e) => {
            errs.push(format!("Failed to inspect {label} {}: {e}", path.display()));
            return;
        }
    };

    if meta.file_type().is_dir() {
        match fs::remove_dir(path) {
            Ok(_) => logs.push(format!("Removed {label}: {}", path.display())),
            Err(e) => errs.push(format!("Failed to remove directory {label} {}: {e}", path.display())),
        }
        return;
    }

    match fs::remove_file(path) {
        Ok(_) => logs.push(format!("Removed {label}: {}", path.display())),
        Err(e) => errs.push(format!("Failed to remove {label} {}: {e}", path.display())),
    }
}

fn remove_dir_if_empty(path: &Path, label: &str, logs: &mut Vec<String>, errs: &mut Vec<String>) {
    if !path.exists() {
        logs.push(format!("{label}: already absent ({})", path.display()));
        return;
    }
    match fs::remove_dir(path) {
        Ok(_) => logs.push(format!("Removed {label}: {}", path.display())),
        Err(e) => logs.push(format!("{label} not removed (likely not empty): {} ({e})", path.display())),
    }
    if path.exists() && let Ok(meta) = fs::metadata(path) {
        if !meta.is_dir() {
            errs.push(format!("{label} path is not a directory: {}", path.display()));
        }
    }
}

fn unregister_menu_entry(desktop_file: &Path) {
    let _ = Command::new("xdg-desktop-menu")
        .args(["uninstall", "--noupdate"])
        .arg(desktop_file)
        .output();
}

fn spawn_deferred_self_cleanup(binary: &Path, install_dir: &Path) {
    let binary_q = shell_quote(binary);
    let dir_q = shell_quote(install_dir);
    let cmd = format!("sleep 1; rm -f {binary_q}; rmdir {dir_q} 2>/dev/null || true");
    let _ = Command::new("/bin/sh")
        .arg("-c")
        .arg(cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

fn shell_quote(path: &Path) -> String {
    let s = path.to_string_lossy();
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn load_texture_from_path(ctx: &egui::Context, path: &Path, texture_name: &str) -> Option<TextureHandle> {
    let img = ImageReader::open(path).ok()?.decode().ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let pixels = img.as_raw();
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels);
    Some(ctx.load_texture(
        texture_name,
        color_image,
        egui::TextureOptions::LINEAR,
    ))
}
