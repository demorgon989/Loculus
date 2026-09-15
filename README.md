Loculus turns an existing AppImage into a polished, Windows-style installer for Linux — so end users get a real install experience instead of a bare executable they have to chmod and hunt for.

How it works

Open the builder, point it at your app's AppImage, fill in the name, tagline, and icon.
Loculus bundles it with a lightweight (~7 MB) installer shell and outputs YourApp-installer.AppImage.
Users double-click the installer and get a step-by-step wizard that handles the app menu entry, desktop shortcut, PATH symlink, and a matching GUI uninstaller.

Details

Pure Rust, GUI built with egui/eframe — no Electron or WebKit, so the installer shell stays small and starts instantly.
Cargo workspace with two crates: builder (the wizard you run) and installer-shell (what your users run).
Build with cargo build --release; binaries land in target/release/.
Verified end-to-end: install and uninstall both tested against a real payload.

Roadmap: packaging Loculus with itself as its own installer AppImage, and a build targeting older glibc for wider distro compatibility.
