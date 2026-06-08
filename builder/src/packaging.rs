//! Builder Contract (locked from working shell + legacy builder)
//!
//! Canonical naming — all three MUST match:
//! - source:  target/release/installer-shell
//! - bundled: <AppDir>/installer-shell
//! - AppRun:  exec "$HERE/installer-shell" "$@"
//!
//! Manifest keys (shell consumes via serde, every field #[serde(default)]):
//!   name, version, publisher, description, tagline, logo, banner, watermark,
//!   appimage, install_path, default_menu_entry, default_desktop_shortcut,
//!   default_path_symlink
//! - appimage is RELATIVE: "payload/<filename>"
//! - the three default_* keys are booleans
//!
//! AppDir layout to reproduce exactly:
//!   <AppDir>/manifest.json
//!   <AppDir>/installer-shell
//!   <AppDir>/payload/<payload>.AppImage
//!   <AppDir>/assets/logo.png      (if provided)
//!   <AppDir>/assets/banner.png    (if provided)
//!   <AppDir>/assets/watermark.png (if provided)
//!   <AppDir>/<safename>.png
//!   <AppDir>/<safename>.desktop
//!   <AppDir>/AppRun
//!
//! Build:
//!   appimagetool <AppDir> <output>.AppImage with ARCH=x86_64 in env

use serde::Serialize;
use std::fs;
use std::fs::Permissions;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const BUNDLED_SHELL_NAME: &str = "installer-shell";

#[derive(Debug, Clone)]
pub struct PackagingInput {
    pub app_name: String,
    pub app_version: String,
    pub publisher: String,
    pub description: String,
    pub tagline: String,
    pub payload_appimage_source: String,
    pub install_path_verbatim: String,
    pub default_menu_entry: bool,
    pub default_desktop_shortcut: bool,
    pub default_path_symlink: bool,
    pub icon_source: String,
    pub logo_source: String,
    pub banner_source: String,
    pub watermark_source: String,
    pub output_dir: String,
    pub shell_binary_source: String,
}

#[derive(Debug, Clone)]
pub struct PackagingResult {
    pub output_appimage: PathBuf,
    pub logs: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ManifestJson {
    name: String,
    version: String,
    publisher: String,
    description: String,
    tagline: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    logo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    banner: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    watermark: Option<String>,
    appimage: String,
    install_path: String,
    default_menu_entry: bool,
    default_desktop_shortcut: bool,
    default_path_symlink: bool,
}

pub fn run_packaging(input: PackagingInput) -> Result<PackagingResult, String> {
    let mut logs = Vec::new();

    require_non_empty(&input.app_name, "App name")?;
    require_non_empty(&input.app_version, "Version")?;
    require_non_empty(&input.payload_appimage_source, "Payload AppImage path")?;
    require_non_empty(&input.install_path_verbatim, "Install path")?;
    require_non_empty(&input.icon_source, "Installer icon path")?;
    require_non_empty(&input.output_dir, "Output directory")?;
    require_non_empty(&input.shell_binary_source, "Shell binary source path")?;

    let app_name = input.app_name.trim().to_string();
    let app_version = input.app_version.trim().to_string();
    let safe_name = safe_name(&app_name);

    let shell_src = canonicalize_existing(&input.shell_binary_source, "shell binary")?;
    let payload_src = canonicalize_existing(&input.payload_appimage_source, "payload AppImage")?;
    let icon_src = canonicalize_existing(&input.icon_source, "installer icon")?;

    let logo_src = canonicalize_optional_existing(&input.logo_source, "logo")?;
    let banner_src = canonicalize_optional_existing(&input.banner_source, "banner")?;
    let watermark_src = canonicalize_optional_existing(&input.watermark_source, "watermark")?;

    let output_dir = resolve_output_dir(&input.output_dir)?;
    logs.push(format!("Output directory: {}", output_dir.display()));

    let appdir = output_dir.join(format!("{safe_name}.AppDir"));
    if appdir.exists() {
        logs.push(format!("Removing existing AppDir: {}", appdir.display()));
        fs::remove_dir_all(&appdir)
            .map_err(|e| format!("Failed to remove existing AppDir '{}': {e}", appdir.display()))?;
    }

    fs::create_dir_all(appdir.join("payload"))
        .map_err(|e| format!("Failed to create payload directory: {e}"))?;
    fs::create_dir_all(appdir.join("assets"))
        .map_err(|e| format!("Failed to create assets directory: {e}"))?;

    let shell_dst = appdir.join(BUNDLED_SHELL_NAME);
    fs::copy(&shell_src, &shell_dst)
        .map_err(|e| format!("Failed to copy shell binary to AppDir: {e}"))?;
    set_mode_755(&shell_dst)?;
    logs.push(format!("Bundled shell binary: {}", shell_dst.display()));

    let payload_filename = payload_src
        .file_name()
        .ok_or_else(|| "Payload AppImage path has no filename".to_string())?
        .to_string_lossy()
        .to_string();
    let payload_dst = appdir.join("payload").join(&payload_filename);
    fs::copy(&payload_src, &payload_dst)
        .map_err(|e| format!("Failed to copy payload AppImage: {e}"))?;
    set_mode_755(&payload_dst)?;
    logs.push(format!("Bundled payload: {}", payload_dst.display()));

    let icon_dst = appdir.join(format!("{safe_name}.png"));
    fs::copy(&icon_src, &icon_dst).map_err(|e| format!("Failed to copy installer icon: {e}"))?;
    logs.push(format!("Copied installer icon: {}", icon_dst.display()));

    let mut logo_rel: Option<String> = None;
    let mut banner_rel: Option<String> = None;
    let mut watermark_rel: Option<String> = None;

    if let Some(path) = logo_src {
        let dst = appdir.join("assets/logo.png");
        fs::copy(path, &dst).map_err(|e| format!("Failed to copy logo: {e}"))?;
        logo_rel = Some("assets/logo.png".to_string());
        logs.push(format!("Copied logo asset: {}", dst.display()));
    }

    if let Some(path) = banner_src {
        let dst = appdir.join("assets/banner.png");
        fs::copy(path, &dst).map_err(|e| format!("Failed to copy banner: {e}"))?;
        banner_rel = Some("assets/banner.png".to_string());
        logs.push(format!("Copied banner asset: {}", dst.display()));
    }

    if let Some(path) = watermark_src {
        let dst = appdir.join("assets/watermark.png");
        fs::copy(path, &dst).map_err(|e| format!("Failed to copy watermark: {e}"))?;
        watermark_rel = Some("assets/watermark.png".to_string());
        logs.push(format!("Copied watermark asset: {}", dst.display()));
    }

    let manifest = ManifestJson {
        name: app_name,
        version: app_version,
        publisher: input.publisher,
        description: input.description,
        tagline: input.tagline,
        logo: logo_rel,
        banner: banner_rel,
        watermark: watermark_rel,
        appimage: format!("payload/{payload_filename}"),
        install_path: input.install_path_verbatim,
        default_menu_entry: input.default_menu_entry,
        default_desktop_shortcut: input.default_desktop_shortcut,
        default_path_symlink: input.default_path_symlink,
    };

    let manifest_path = appdir.join("manifest.json");
    let manifest_json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("Failed to serialize manifest JSON: {e}"))?;
    fs::write(&manifest_path, manifest_json)
        .map_err(|e| format!("Failed to write manifest JSON: {e}"))?;
    logs.push(format!("Wrote manifest: {}", manifest_path.display()));

    let desktop_path = appdir.join(format!("{safe_name}.desktop"));
    let desktop_contents = format!(
        "[Desktop Entry]\nName={} Installer\nExec=AppRun\nIcon={}\nType=Application\nCategories=Utility;\nTerminal=false\n",
        manifest.name, safe_name
    );
    fs::write(&desktop_path, desktop_contents)
        .map_err(|e| format!("Failed to write desktop file: {e}"))?;
    logs.push(format!("Wrote desktop entry: {}", desktop_path.display()));

    let apprun_path = appdir.join("AppRun");
    let apprun_script = "#!/bin/bash\nset -euo pipefail\nHERE=\"$(dirname \"$0\")\"\nexec \"$HERE/installer-shell\" \"$@\"\n";
    fs::write(&apprun_path, apprun_script).map_err(|e| format!("Failed to write AppRun: {e}"))?;
    set_mode_755(&apprun_path)?;
    logs.push(format!("Wrote AppRun: {}", apprun_path.display()));

    let output_appimage = output_dir.join(format!("{safe_name}-installer.AppImage"));
    if output_appimage.exists() {
        fs::remove_file(&output_appimage).map_err(|e| {
            format!(
                "Failed to remove existing output AppImage '{}': {e}",
                output_appimage.display()
            )
        })?;
    }

    logs.push(format!(
        "Running appimagetool: appimagetool {} {}",
        appdir.display(),
        output_appimage.display()
    ));

    let output = Command::new("appimagetool")
        .arg(&appdir)
        .arg(&output_appimage)
        .env("ARCH", "x86_64")
        .output()
        .map_err(|e| format!("Failed to execute appimagetool: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if !stdout.is_empty() {
        logs.push(format!("appimagetool stdout:\n{stdout}"));
    }
    if !stderr.is_empty() {
        logs.push(format!("appimagetool stderr:\n{stderr}"));
    }

    if !output.status.success() {
        return Err(format!(
            "appimagetool failed with status {:?}",
            output.status.code()
        ));
    }

    logs.push(format!("Built installer AppImage: {}", output_appimage.display()));

    Ok(PackagingResult {
        output_appimage,
        logs,
    })
}

fn require_non_empty(value: &str, label: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{label} is required."))
    } else {
        Ok(())
    }
}

fn safe_name(raw: &str) -> String {
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

pub(crate) fn safe_name_for_display(raw: &str) -> String {
    safe_name(raw)
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    if path == "~" {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home);
        }
    }
    PathBuf::from(path)
}

fn canonicalize_existing(raw_path: &str, label: &str) -> Result<PathBuf, String> {
    let expanded = expand_tilde(raw_path.trim());
    if !expanded.exists() {
        return Err(format!("{} does not exist: {}", label, expanded.display()));
    }
    expanded
        .canonicalize()
        .map_err(|e| format!("Failed to canonicalize {} '{}': {e}", label, expanded.display()))
}

fn canonicalize_optional_existing(raw_path: &str, label: &str) -> Result<Option<PathBuf>, String> {
    if raw_path.trim().is_empty() {
        return Ok(None);
    }
    canonicalize_existing(raw_path, label).map(Some)
}

fn resolve_output_dir(raw_path: &str) -> Result<PathBuf, String> {
    let expanded = expand_tilde(raw_path.trim());
    fs::create_dir_all(&expanded).map_err(|e| {
        format!(
            "Failed to create output directory '{}': {e}",
            expanded.display()
        )
    })?;
    Ok(expanded)
}

fn set_mode_755(path: &Path) -> Result<(), String> {
    let permissions = Permissions::from_mode(0o755);
    fs::set_permissions(path, permissions)
        .map_err(|e| format!("Failed to set mode 755 on '{}': {e}", path.display()))
}
