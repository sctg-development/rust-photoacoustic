// Copyright (c) 2024 Ronan LE MEILLAT for SCTG Development
//
// This file is part of the SCTGDesk project.
//
// SCTGDesk is free software: you can redistribute it and/or modify
// it under the terms of the Affero General Public License version 3 as
// published by the Free Software Foundation.
//
// SCTGDesk is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// Affero General Public License for more details.
//
// You should have received a copy of the Affero General Public License
// along with SCTGDesk. If not, see <https://www.gnu.org/licenses/agpl-3.0.html>.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::str;
use std::time::{SystemTime, UNIX_EPOCH};

// Vérifie si des fichiers source web sont plus récents que les fichiers compilés
fn is_web_source_newer_than_dist(dist_path: &PathBuf) -> bool {
    // Obtenir la date de modification la plus récente des fichiers dans dist
    let dist_latest_mod = get_latest_modification_time(dist_path)
        .unwrap_or_else(|| SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() - 86400);
    
    // Obtenir la date de modification la plus récente des fichiers source
    let src_paths = [
        PathBuf::from("./web/src"),
        PathBuf::from("./web/public"),
        PathBuf::from("./web/index.html"),
        PathBuf::from("./web/package.json"),
        PathBuf::from("./web/tsconfig.json"),
        PathBuf::from("./web/vite.config.ts"),
        // Ajoutez d'autres fichiers/dossiers à surveiller au besoin
    ];
    
    for path in &src_paths {
        if let Some(mod_time) = get_latest_modification_time(path) {
            if mod_time > dist_latest_mod {
                println!("cargo:warning=Fichier modifié détecté: {:?}", path);
                return true;
            }
        }
    }
    
    false
}

// Récupère la date de modification la plus récente dans un répertoire (récursivement)
// ou pour un fichier unique
fn get_latest_modification_time(path: &PathBuf) -> Option<u64> {
    if !path.exists() {
        return None;
    }
    
    let mut latest = 0;
    
    if path.is_file() {
        if let Ok(metadata) = fs::metadata(path) {
            if let Ok(modified) = metadata.modified() {
                if let Ok(since_epoch) = modified.duration_since(UNIX_EPOCH) {
                    return Some(since_epoch.as_secs());
                }
            }
        }
        return None;
    }
    
    // Fonction récursive pour parcourir les dossiers
    fn visit_dir(dir: &PathBuf, latest: &mut u64) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Ok(metadata) = fs::metadata(&path) {
                        if let Ok(modified) = metadata.modified() {
                            if let Ok(since_epoch) = modified.duration_since(UNIX_EPOCH) {
                                let secs = since_epoch.as_secs();
                                if secs > *latest {
                                    *latest = secs;
                                }
                            }
                        }
                    }
                } else if path.is_dir() {
                    visit_dir(&path, latest);
                }
            }
        }
    }
    
    visit_dir(path, &mut latest);
    
    if latest > 0 {
        Some(latest)
    } else {
        None
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackageJson {
    name: String,
    private: Option<bool>,
    version: String,
    #[serde(rename = "type")]
    type_: Option<String>,
    scripts: HashMap<String, String>,
    dependencies: HashMap<String, String>,
    #[serde(rename = "devDependencies")]
    dev_dependencies: HashMap<String, String>,
}

impl PackageJson {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            private: None,
            version: String::new(),
            type_: None,
            scripts: HashMap::new(),
            dependencies: HashMap::new(),
            dev_dependencies: HashMap::new(),
        }
    }

    pub fn set_version(&mut self, version: &str) {
        self.version = version.to_string();
    }
}

#[tokio::main]
async fn main() {
    // Indique à Cargo de relancer le script build.rs si des fichiers du dossier web changent
    println!("cargo:rerun-if-changed=web");

    // Vérifie si les fichiers dist existent déjà pour éviter une reconstruction inutile
    let dist_path = PathBuf::from("./web/dist");
    let needs_build = !dist_path.exists() || is_web_source_newer_than_dist(&dist_path);

    let data = fs::read_to_string("./web/package.json").unwrap();
    let mut package: PackageJson = serde_json::from_str(&data).unwrap();

    // Construit le chemin du fichier dans le répertoire temporaire
    let tmp_dir = env::var("TMP")
        .or_else(|_| env::var("TEMP"))
        .or_else(|_| env::var("TMPDIR"))
        .unwrap_or_else(|_| "/tmp".to_string());
    let mut path = PathBuf::from(tmp_dir);
    path.push("version-1B282C00-C9CC-4C5F-890E-952D88623718.txt");
    // Lit la version à partir du fichier
    let version =
        fs::read_to_string(&path).unwrap_or_else(|_| env::var("CARGO_PKG_VERSION").unwrap());

    // Vérifie si la version a changé
    let version_changed = package.version != version;
    if version_changed {
        package.set_version(&version);
        let serialized = serde_json::to_string_pretty(&package).unwrap();
        fs::write("./web/package.json", serialized).unwrap();
    }

    // Si aucune reconstruction n'est nécessaire, on quitte tôt
    if !needs_build && !version_changed {
        println!("cargo:warning=Aucun changement détecté dans les fichiers web, compilation ignorée");
        return;
    }

    let is_windows = cfg!(target_os = "windows");

    let (command, install_args, build_args) = if is_windows {
        ("cmd.exe", &["/C", "npm install --force"], &["/C", "npm run build"])
    } else {
        ("npm", &["install", "--force"], &["run", "build"])
    };

    // Install npm dependencies for webconsole
    let output = Command::new(command)
        .current_dir("web")
        .args(install_args)
        .output()
        .expect("Failed to execute command");
    assert!(
        output.status.success(),
        "Failed to install npm dependencies: {}{}",
        str::from_utf8(&output.stdout).unwrap_or(""),
        str::from_utf8(&output.stderr).unwrap_or("")
    );

    // Build webconsole
    let output = Command::new(command)
        .current_dir("web")
        .args(build_args)
        .output()
        .expect("Failed to execute command");
    assert!(
        output.status.success(),
        "Failed to build web: {}{}",
        str::from_utf8(&output.stdout).unwrap_or(""),
        str::from_utf8(&output.stderr).unwrap_or("")
    );

    // // Install npm dependencies for rapidoc
    //     let output = Command::new(command)
    //     .current_dir("rapidoc")
    //     .args(install_args)
    //     .output()
    //     .expect("Failed to execute command");
    // assert!(
    //     output.status.success(),
    //     "Failed to install npm dependencies: {}{}",
    //     str::from_utf8(&output.stdout).unwrap_or(""),
    //     str::from_utf8(&output.stderr).unwrap_or("")
    // );

    // // Build rapidoc
    // let output = Command::new(command)
    //     .current_dir("rapidoc")
    //     .args(build_args)
    //     .output()
    //     .expect("Failed to execute command");
    // assert!(
    //     output.status.success(),
    //     "Failed to build rapidoc: {}{}",
    //     str::from_utf8(&output.stdout).unwrap_or(""),
    //     str::from_utf8(&output.stderr).unwrap_or("")
    // );
}