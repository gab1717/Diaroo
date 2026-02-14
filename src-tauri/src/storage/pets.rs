use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimationDef {
    pub frames: u32,
    pub frame_duration: u32,
    #[serde(rename = "loop")]
    pub looping: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PetManifest {
    pub name: String,
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub sprite_size: u32,
    pub animations: HashMap<String, AnimationDef>,
    pub default_animation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PetInfo {
    #[serde(flatten)]
    pub manifest: PetManifest,
    pub sprite_paths: HashMap<String, String>,
    pub builtin: bool,
}

pub fn user_pets_dir() -> PathBuf {
    super::config::app_data_dir().join("pets")
}

pub fn validate_pet_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("Pet name cannot be empty");
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
    {
        bail!("Pet name must match [a-z0-9_-]+");
    }
    Ok(())
}

pub fn load_manifest(pet_dir: &Path) -> Result<PetManifest> {
    let manifest_path = pet_dir.join("pet.json");
    let contents =
        fs::read_to_string(&manifest_path).context("Failed to read pet.json")?;
    let manifest: PetManifest =
        serde_json::from_str(&contents).context("Failed to parse pet.json")?;
    validate_pet_name(&manifest.name)?;
    Ok(manifest)
}

pub fn pet_info_from_dir(pet_dir: &Path, builtin: bool) -> Result<PetInfo> {
    let manifest = load_manifest(pet_dir)?;
    let sprites_dir = pet_dir.join("sprites");
    let mut sprite_paths = HashMap::new();

    for anim_name in manifest.animations.keys() {
        let png_path = sprites_dir.join(format!("{}.png", anim_name));
        if png_path.exists() {
            sprite_paths.insert(
                anim_name.clone(),
                png_path.to_string_lossy().to_string(),
            );
        }
    }

    Ok(PetInfo {
        manifest,
        sprite_paths,
        builtin,
    })
}

pub fn list_all_pets() -> Result<Vec<PetInfo>> {
    let pets_dir = user_pets_dir();
    if !pets_dir.exists() {
        return Ok(vec![]);
    }
    let mut pets = Vec::new();
    for entry in fs::read_dir(&pets_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if !path.join("pet.json").exists() {
            continue;
        }
        let builtin = path.join(".builtin").exists();
        match pet_info_from_dir(&path, builtin) {
            Ok(info) => pets.push(info),
            Err(e) => log::warn!("Skipping pet at {:?}: {}", path, e),
        }
    }
    pets.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));
    Ok(pets)
}

pub fn get_pet(name: &str) -> Result<PetInfo> {
    validate_pet_name(name)?;
    let pet_dir = user_pets_dir().join(name);
    if !pet_dir.exists() {
        bail!("Pet '{}' not found", name);
    }
    let builtin = pet_dir.join(".builtin").exists();
    pet_info_from_dir(&pet_dir, builtin)
}

pub fn install_dpet(zip_path: &Path) -> Result<PetInfo> {
    let file = fs::File::open(zip_path).context("Failed to open .dpet file")?;
    let mut archive = zip::ZipArchive::new(file).context("Invalid zip archive")?;

    // First pass: find and parse pet.json
    let manifest: PetManifest = {
        let mut pet_json = archive
            .by_name("pet.json")
            .context("Missing pet.json in .dpet archive")?;
        let mut contents = String::new();
        pet_json
            .read_to_string(&mut contents)
            .context("Failed to read pet.json from archive")?;
        serde_json::from_str(&contents).context("Invalid pet.json")?
    };

    validate_pet_name(&manifest.name)?;

    let dest_dir = user_pets_dir().join(&manifest.name);
    if dest_dir.exists() {
        bail!(
            "Pet '{}' already exists. Remove it first.",
            manifest.name
        );
    }

    // Second pass: extract only pet.json and sprites/*.png
    fs::create_dir_all(&dest_dir)?;
    let sprites_dir = dest_dir.join("sprites");
    fs::create_dir_all(&sprites_dir)?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let Some(name) = entry.enclosed_name().map(|p| p.to_path_buf()) else {
            continue; // skip path-traversal entries
        };
        // Normalize to forward slashes (PowerShell zips use backslashes on Windows)
        let name_str = name.to_string_lossy().replace('\\', "/");

        if name_str == "pet.json" {
            let out_path = dest_dir.join("pet.json");
            let mut out_file = fs::File::create(&out_path)?;
            std::io::copy(&mut entry, &mut out_file)?;
        } else if name_str.starts_with("sprites/") && name_str.ends_with(".png") {
            let file_name = name
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if !file_name.is_empty() {
                let out_path = sprites_dir.join(&file_name);
                let mut out_file = fs::File::create(&out_path)?;
                std::io::copy(&mut entry, &mut out_file)?;
            }
        }
    }

    pet_info_from_dir(&dest_dir, false)
}

pub fn remove_pet(name: &str) -> Result<()> {
    validate_pet_name(name)?;
    let pet_dir = user_pets_dir().join(name);
    if !pet_dir.exists() {
        bail!("Pet '{}' not found", name);
    }
    fs::remove_dir_all(&pet_dir).context("Failed to remove pet directory")?;
    Ok(())
}
