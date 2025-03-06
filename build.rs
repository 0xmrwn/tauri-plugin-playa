use std::fs;
use std::path::Path;
use tauri_plugin::Builder;

// Define the commands that the plugin exposes
const COMMANDS: &[&str] = &["play", "control", "get_info", "close", "list_presets"];

fn main() {
    // Tell Cargo to rebuild if any of these files change
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=mpv_config");
    println!("cargo:rerun-if-changed=mpv_config/scripts");
    println!("cargo:rerun-if-changed=mpv_config/fonts");
    println!("cargo:rerun-if-changed=mpv_config/script-opts");
    
    // Copy mpv_config to the output directory
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let mpv_config_path = Path::new("mpv_config");
    let dest_path = Path::new(&out_dir).join("mpv_config");
    
    // Create output directory if it doesn't exist
    fs::create_dir_all(&dest_path).expect("Failed to create mpv_config directory in OUT_DIR");
    
    // Copy the mpv_config directory recursively
    copy_dir_recursively(mpv_config_path, &dest_path)
        .expect("Failed to copy mpv_config to OUT_DIR");
    
    // Build the plugin
    Builder::new(COMMANDS).build();
}

fn copy_dir_recursively(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !src.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Source path is not a directory",
        ));
    }
    
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }
    
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let entry_path = entry.path();
        let file_name = entry.file_name();
        let dst_path = dst.join(&file_name);
        
        if entry_path.is_dir() {
            copy_dir_recursively(&entry_path, &dst_path)?;
        } else {
            fs::copy(&entry_path, &dst_path)?;
        }
    }
    
    Ok(())
}
