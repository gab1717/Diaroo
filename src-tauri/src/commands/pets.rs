use base64::Engine;
use crate::storage::pets::{self, PetInfo};

#[tauri::command]
pub async fn list_pets() -> Result<Vec<PetInfo>, String> {
    pets::list_all_pets().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_pet_info(name: String) -> Result<PetInfo, String> {
    pets::get_pet(&name).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn install_pet(path: String) -> Result<PetInfo, String> {
    let zip_path = std::path::PathBuf::from(&path);
    pets::install_dpet(&zip_path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_pet(name: String) -> Result<(), String> {
    pets::remove_pet(&name).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn read_sprite(path: String) -> Result<String, String> {
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&bytes))
}
