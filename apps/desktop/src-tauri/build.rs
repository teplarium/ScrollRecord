//! Tauri build step: validates `tauri.conf.json`, generates permission
//! schemas and embeds the Windows icon/manifest. Nothing project-specific here.

fn main() {
    tauri_build::build();
}
