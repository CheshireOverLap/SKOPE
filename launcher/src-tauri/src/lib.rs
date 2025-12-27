// SKOPE Launcher - Rust Backend

use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{BufReader, Write};
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc;
use tauri_plugin_dialog::DialogExt;
use tar::Archive;
use xz2::read::XzDecoder;

// ============ Data Structures ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub path: String,
    #[serde(rename = "lastOpened")]
    pub last_opened: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    #[serde(rename = "blenderPath")]
    pub blender_path: String,
    #[serde(rename = "codeEditor")]
    pub code_editor: String,
    #[serde(rename = "customEditorPath")]
    pub custom_editor_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LauncherData {
    pub projects: Vec<Project>,
    pub settings: Settings,
}

// ============ Helper Functions ============

fn get_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("skope-launcher")
}

fn get_data_file() -> PathBuf {
    get_data_dir().join("launcher.json")
}

fn load_data() -> LauncherData {
    let path = get_data_file();
    if path.exists() {
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        LauncherData::default()
    }
}

fn save_data(data: &LauncherData) -> Result<(), String> {
    let dir = get_data_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let path = get_data_file();
    let json = serde_json::to_string_pretty(data).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())?;

    Ok(())
}

fn get_current_time() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

fn get_skope_blender_path() -> PathBuf {
    get_data_dir().join("blender").join("blender")
}

fn find_blender() -> Option<String> {
    // 1. Check SKOPE's own Blender first
    let skope_blender = get_skope_blender_path();
    if skope_blender.exists() {
        return Some(skope_blender.to_string_lossy().to_string());
    }

    // 2. Common Blender paths on Linux
    let paths = [
        "/snap/bin/blender",
        "/usr/bin/blender",
        "/usr/local/bin/blender",
    ];

    for path in paths {
        if std::path::Path::new(path).exists() {
            return Some(path.to_string());
        }
    }

    // 3. Try to find via which command
    if let Ok(output) = Command::new("which").arg("blender").output() {
        if output.status.success() {
            if let Ok(path) = String::from_utf8(output.stdout) {
                let path = path.trim();
                if !path.is_empty() {
                    return Some(path.to_string());
                }
            }
        }
    }

    None
}

// Blender version to download
const BLENDER_VERSION: &str = "4.2.4";
const BLENDER_DOWNLOAD_URL: &str = "https://mirror.clarkson.edu/blender/release/Blender4.2/blender-4.2.4-linux-x64.tar.xz";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlenderStatus {
    pub installed: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    pub is_skope_managed: bool,
}

fn check_blender_status() -> BlenderStatus {
    let skope_blender = get_skope_blender_path();

    if skope_blender.exists() {
        return BlenderStatus {
            installed: true,
            path: Some(skope_blender.to_string_lossy().to_string()),
            version: Some(BLENDER_VERSION.to_string()),
            is_skope_managed: true,
        };
    }

    if let Some(path) = find_blender() {
        // Try to get version from system blender
        let version = Command::new(&path)
            .arg("--version")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| s.lines().next().map(|l| l.replace("Blender ", "")));

        return BlenderStatus {
            installed: true,
            path: Some(path),
            version,
            is_skope_managed: false,
        };
    }

    BlenderStatus {
        installed: false,
        path: None,
        version: None,
        is_skope_managed: false,
    }
}

fn download_and_install_blender() -> Result<String, String> {
    let blender_dir = get_data_dir().join("blender");

    // Clean up existing installation
    if blender_dir.exists() {
        fs::remove_dir_all(&blender_dir).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(&blender_dir).map_err(|e| e.to_string())?;

    // Download
    let tar_path = blender_dir.join("blender.tar.xz");

    println!("Downloading Blender from {}...", BLENDER_DOWNLOAD_URL);

    let response = reqwest::blocking::get(BLENDER_DOWNLOAD_URL)
        .map_err(|e| format!("다운로드 실패: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("다운로드 실패: HTTP {}", response.status()));
    }

    let bytes = response.bytes().map_err(|e| format!("다운로드 실패: {}", e))?;

    let mut file = File::create(&tar_path).map_err(|e| e.to_string())?;
    file.write_all(&bytes).map_err(|e| e.to_string())?;
    drop(file);

    println!("Extracting Blender...");

    // Extract .tar.xz
    let tar_file = File::open(&tar_path).map_err(|e| e.to_string())?;
    let xz = XzDecoder::new(BufReader::new(tar_file));
    let mut archive = Archive::new(xz);

    archive.unpack(&blender_dir).map_err(|e| format!("압축 해제 실패: {}", e))?;

    // Remove tar file
    let _ = fs::remove_file(&tar_path);

    // Find extracted folder and create symlink
    let entries: Vec<_> = fs::read_dir(&blender_dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();

    if let Some(extracted_dir) = entries.first() {
        let blender_exe = extracted_dir.path().join("blender");
        if blender_exe.exists() {
            // Create symlink to blender executable
            let link_path = blender_dir.join("blender");
            #[cfg(unix)]
            std::os::unix::fs::symlink(&blender_exe, &link_path)
                .map_err(|e| format!("심볼릭 링크 생성 실패: {}", e))?;

            println!("Blender installed successfully!");
            return Ok(link_path.to_string_lossy().to_string());
        }
    }

    Err("Blender 실행 파일을 찾을 수 없습니다".to_string())
}

// ============ Tauri Commands ============

#[tauri::command]
fn get_blender_status() -> BlenderStatus {
    check_blender_status()
}

#[tauri::command]
fn install_blender() -> Result<String, String> {
    download_and_install_blender()
}

#[tauri::command]
fn get_projects() -> Vec<Project> {
    load_data().projects
}

#[tauri::command]
fn add_project(path: String) -> Result<(), String> {
    let mut data = load_data();

    // Check if project already exists
    if data.projects.iter().any(|p| p.path == path) {
        // Update last opened time
        if let Some(project) = data.projects.iter_mut().find(|p| p.path == path) {
            project.last_opened = Some(get_current_time());
        }
    } else {
        // Get project name from path
        let name = PathBuf::from(&path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown")
            .to_string();

        data.projects.push(Project {
            name,
            path,
            last_opened: Some(get_current_time()),
        });
    }

    save_data(&data)
}

#[tauri::command]
fn create_project(name: String, location: String) -> Result<(), String> {
    let project_path = PathBuf::from(&location).join(&name);

    // Create project directory structure
    fs::create_dir_all(&project_path).map_err(|e| e.to_string())?;
    fs::create_dir_all(project_path.join("assets")).map_err(|e| e.to_string())?;
    fs::create_dir_all(project_path.join("levels")).map_err(|e| e.to_string())?;
    fs::create_dir_all(project_path.join("src")).map_err(|e| e.to_string())?;

    // Create basic Cargo.toml
    let cargo_toml = format!(
        r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[dependencies]
# SKOPE engine dependencies will be added here
"#,
        name.to_lowercase().replace(' ', "_")
    );
    fs::write(project_path.join("Cargo.toml"), cargo_toml).map_err(|e| e.to_string())?;

    // Create basic main.rs
    let main_rs = r#"// SKOPE Game Project
fn main() {
    println!("SKOPE Game - Ready to build!");
}
"#;
    fs::write(project_path.join("src").join("main.rs"), main_rs).map_err(|e| e.to_string())?;

    // Add to projects
    add_project(project_path.to_string_lossy().to_string())
}

#[tauri::command]
fn get_settings() -> Settings {
    let mut settings = load_data().settings;

    // Auto-detect Blender if not set
    if settings.blender_path.is_empty() {
        if let Some(path) = find_blender() {
            settings.blender_path = path;
        }
    }

    // Default editor
    if settings.code_editor.is_empty() {
        settings.code_editor = "rustrover".to_string();
    }

    settings
}

#[tauri::command]
fn save_settings(settings: Settings) -> Result<(), String> {
    let mut data = load_data();
    data.settings = settings;
    save_data(&data)
}

#[tauri::command]
async fn open_folder_dialog(app: tauri::AppHandle) -> Option<String> {
    let (tx, rx) = mpsc::channel();

    app.dialog()
        .file()
        .set_title("폴더 선택")
        .pick_folder(move |path| {
            let _ = tx.send(path.map(|p| p.to_string()));
        });

    rx.recv().ok().flatten()
}

#[tauri::command]
async fn open_file_dialog(app: tauri::AppHandle) -> Option<String> {
    let (tx, rx) = mpsc::channel();

    app.dialog()
        .file()
        .set_title("파일 선택")
        .pick_file(move |path| {
            let _ = tx.send(path.map(|p| p.to_string()));
        });

    rx.recv().ok().flatten()
}

#[tauri::command]
fn open_blender(project_path: String, blender_path: String) -> Result<(), String> {
    let blender = if blender_path.is_empty() {
        find_blender().ok_or("Blender를 찾을 수 없습니다")?
    } else {
        blender_path
    };

    // Find .blend files in project
    let blend_files: Vec<_> = fs::read_dir(&project_path)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "blend")
                .unwrap_or(false)
        })
        .collect();

    let mut cmd = Command::new(&blender);

    // If there's a .blend file, open it
    if let Some(blend_file) = blend_files.first() {
        cmd.arg(blend_file.path());
    }

    // Set working directory
    cmd.current_dir(&project_path);

    // Set environment variable for SKOPE addon
    cmd.env("SKOPE_PROJECT_PATH", &project_path);

    cmd.spawn().map_err(|e| format!("Blender 실행 실패: {}", e))?;

    // Update last opened time
    let _ = add_project(project_path);

    Ok(())
}

#[tauri::command]
fn open_code_editor(project_path: String, editor: String, custom_path: String) -> Result<(), String> {
    let editor_cmd = match editor.as_str() {
        "rustrover" => {
            // Try common RustRover locations
            if std::path::Path::new("/snap/bin/rustrover").exists() {
                "/snap/bin/rustrover"
            } else if std::path::Path::new("/usr/local/bin/rustrover").exists() {
                "/usr/local/bin/rustrover"
            } else {
                "rustrover"
            }
        }
        "vscode" => "code",
        "zed" => "zed",
        "custom" => {
            if custom_path.is_empty() {
                return Err("사용자 지정 에디터 경로가 비어있습니다".to_string());
            }
            &custom_path
        }
        _ => return Err(format!("알 수 없는 에디터: {}", editor)),
    };

    Command::new(editor_cmd)
        .arg(&project_path)
        .spawn()
        .map_err(|e| format!("에디터 실행 실패: {}", e))?;

    Ok(())
}

#[tauri::command]
fn play_game(project_path: String) -> Result<(), String> {
    let project = PathBuf::from(&project_path);

    // Check if it's a Cargo project
    if !project.join("Cargo.toml").exists() {
        return Err("Cargo.toml을 찾을 수 없습니다".to_string());
    }

    // Build and run
    Command::new("cargo")
        .args(["run", "--release"])
        .current_dir(&project_path)
        .spawn()
        .map_err(|e| format!("게임 실행 실패: {}", e))?;

    Ok(())
}

// ============ App Entry ============

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            get_blender_status,
            install_blender,
            get_projects,
            add_project,
            create_project,
            get_settings,
            save_settings,
            open_folder_dialog,
            open_file_dialog,
            open_blender,
            open_code_editor,
            play_game,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
