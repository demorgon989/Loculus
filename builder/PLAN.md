# Builder Packaging Rewrite Plan (Rust/egui GUI builder)

1. **Proposed function/module breakdown**
   1. Create a new `builder/src/packaging.rs` module dedicated to packaging flow only.
   2. Define `PackagingInput` (owned, validated data extracted from `AppState`) and `PackagingOutput` (paths + status details).
   3. Define `ManifestJson` as a `#[derive(Serialize)]` struct with contract-locked fields only.
   4. Define `BuildLogger` helper interface shape (or callbacks) so packaging steps can stream human-readable progress lines into existing UI log state.
   5. Implement orchestrator function `run_packaging(input, logger) -> Result<PackagingOutput, PackagingError>` that executes the end-to-end flow.
   6. Implement small focused helpers:
      - `safe_name(...)`
      - `prepare_output_dir(...)`
      - `create_appdir_tree(...)`
      - `copy_shell_binary(...)`
      - `copy_payload(...)`
      - `copy_optional_assets(...)`
      - `write_manifest(...)`
      - `write_desktop_entry(...)`
      - `write_apprun(...)`
      - `set_executable_bit(...)`
      - `run_appimagetool(...)`
   7. Keep UI wiring separate: `main.rs` should call packaging through a thin adapter from Build page action only, not embed filesystem/process logic directly in UI code.

2. **Python `build_installer` flow → Rust equivalent + crate mapping**
   1. Resolve and normalize input paths (`expanduser`, `resolve`) in Python → Rust with `std::path::PathBuf`, plus tilde expansion helper (for GUI-entered strings) and `std::fs::canonicalize` where applicable.
   2. Ensure output dir exists (`mkdir(parents=True, exist_ok=True)`) → `std::fs::create_dir_all`.
   3. Derive app defaults (`app_name`, `version`, `safe_name`) → pure Rust helper functions.
   4. Validate required files exist (`shell_binary`, `payload`, `icon`) and optional files if provided → `Path::exists` checks with explicit error context.
   5. Remove old AppDir if present (`shutil.rmtree`) → `std::fs::remove_dir_all`.
   6. Create AppDir subdirs (`payload`, `assets`) → `std::fs::create_dir_all`.
   7. Copy shell binary to `<AppDir>/installer-shell` and make executable → `std::fs::copy` + unix perms update.
   8. Copy payload into `<AppDir>/payload/<filename>` and make executable → `std::fs::copy` + unix perms update.
   9. Build manifest object and write pretty JSON (`json.dumps(..., indent=2)`) → `serde_json::to_string_pretty` + `std::fs::write`.
   10. Copy optional `logo/banner/watermark` into `assets/*.png` and set corresponding manifest fields to relative paths.
   11. Copy installer icon to `<AppDir>/<safename>.png`.
   12. Write `<safename>.desktop` with `Exec=AppRun`.
   13. Write `AppRun` script with canonical line `exec "$HERE/installer-shell" "$@"` and chmod +x.
   14. Remove pre-existing output AppImage if present (`unlink`) → `std::fs::remove_file` when exists.
   15. Spawn `appimagetool <AppDir> <output>.AppImage` with `ARCH=x86_64` env.
   16. Return output path on success.
   17. **Not 1:1 by design:** no CLI argument parsing in Rust builder packaging module; inputs come from GUI `AppState` mapping instead.

3. **Executable bit strategy (shell binary, payload, AppRun)**
   1. Use Unix permission APIs: `std::os::unix::fs::PermissionsExt`.
   2. Read current mode via `std::fs::metadata(path)?.permissions().mode()`.
   3. Set exec bit with bitwise OR `mode | 0o111` (equivalent intent to Python `st_mode | stat.S_IEXEC`, but applies executable to user/group/other consistently for packaged artifacts).
   4. Apply with `std::fs::set_permissions(path, Permissions::from_mode(new_mode))`.
   5. Use same helper for all three files to prevent drift.

4. **Manifest writing with exact keys + guard against misspells/omissions**
   1. Use a dedicated `ManifestJson` struct with `#[derive(Serialize)]` and exact field names:
      - `name`, `version`, `publisher`, `description`, `tagline`, `logo`, `banner`, `watermark`, `appimage`, `install_path`, `default_menu_entry`, `default_desktop_shortcut`, `default_path_symlink`.
   2. Do **not** use ad-hoc `serde_json::json!({ ... })` map construction with free-form string keys.
   3. Optional paths (`logo/banner/watermark`) represented as strings in struct, defaulting to empty string when not provided, matching shell `#[serde(default)]` consumption model.
   4. Set `appimage` explicitly to relative `payload/<filename>` only.
   5. Serialize with `serde_json::to_string_pretty(&manifest_struct)` and write to `<AppDir>/manifest.json`.
   6. Add focused validation/tests in plan execution phase to assert serialized output contains exactly required keys and expected types (booleans for default flags).

5. **`appimagetool` invocation + non-zero exit handling**
   1. Use `std::process::Command` with args: `<AppDir>`, `<output>.AppImage`.
   2. Set env var `ARCH=x86_64` via `.env("ARCH", "x86_64")` while inheriting ambient environment.
   3. Capture stdout/stderr (`.output()`), not fire-and-forget.
   4. If `status.success()` is false, return structured error containing exit code + stderr/stdout snippets for UI log visibility.
   5. Log command start, completion, and output path to Build page log area.

6. **Map GUI `AppState` inputs to packaging inputs (no CLI replication)**
   1. Map existing `AppState` fields:
      - `metadata_app_name` → manifest `name`
      - `metadata_version` → manifest `version`
      - `payload_appimage_path` → payload source file
      - `branding_icon_path` → installer icon source file (`<safename>.png` in AppDir)
      - `branding_banner_path` → optional banner asset source
   2. Packaging inputs required by contract/legacy behavior that GUI does **not yet fully collect** must be flagged:
      - `publisher` (missing UI field currently)
      - `description` (missing UI field currently)
      - `tagline` (missing UI field currently)
      - `branding_logo_path` (icon path exists, but shell contract distinguishes logo and icon roles)
      - `branding_watermark_path` (missing UI field currently)
      - `install_path` default template/value (missing UI field currently)
      - `default_menu_entry`, `default_desktop_shortcut`, `default_path_symlink` booleans (missing UI controls currently)
      - output directory for produced installer (missing UI field currently)
      - source shell binary path (should default to `target/release/installer-shell`, but still needs validation and potential override strategy)
   3. Plan packaging wiring so missing fields block build with explicit UI validation errors rather than silent defaults unless defaults are deliberately chosen.

7. **Contract as module-level doc comment near packaging logic**
   1. At top of `packaging.rs`, include a `//!` module doc block containing the locked contract (canonical naming, manifest keys, AppDir layout, appimagetool invocation).
   2. Use this as maintenance guardrail so any future rename/layout change is intentional and reviewed.

8. **Verification plan (runtime and artifact checks, not compile-only)**
   1. Unit-test `safe_name` behavior with representative names.
   2. Unit-test manifest serialization shape and key names from `ManifestJson`.
   3. Run builder GUI, execute packaging with a known test payload/icon/shell binary.
   4. Inspect generated AppDir tree (`find`/`ls -la`) to confirm exact required layout.
   5. Verify permissions (`stat -c '%A %n'`) on `AppRun`, bundled `installer-shell`, and payload.
   6. Inspect `manifest.json` contents for exact keys and expected values/types.
   7. Inspect `AppRun` file text for exact exec line: `exec "$HERE/installer-shell" "$@"`.
   8. Confirm `appimagetool` success path and output installer file exists.
   9. Negative test: intentionally missing required input (e.g., payload path) should fail fast with clear surfaced error in build logs.
   10. Smoke-run produced installer AppImage to verify it launches shell and reads manifest as expected.
