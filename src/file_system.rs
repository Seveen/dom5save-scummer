use std::{
    ffi::OsString,
    fs::{self, File},
    path::{Path, PathBuf},
    sync::mpsc::channel,
    time::Duration, io::Write,
};

use anyhow::{anyhow, Context, Result};
use directories::{ProjectDirs, UserDirs, BaseDirs};
use druid::im::Vector;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use crate::State;

pub fn get_saved_games_dir_from_config() -> Result<PathBuf> {
    let config_dir = get_config_dir()?;
    let config_file = config_dir.join("config");

    if config_file.exists() {
        println!("Config file exists");
        let raw_dir = fs::read_to_string(config_file)?;
        Ok(PathBuf::from(raw_dir))
    } else {
        println!("Config file doesn't exist, getting default");
        get_default_dom5_dir()
    }
}

pub fn save_saved_games_dir_in_config(dir: &str) -> Result<()> {
    let config_dir = get_config_dir()?;
    let config_file = config_dir.join("config");

    if config_file.exists() {
        fs::remove_file(&config_file)?;
    }

    let mut file = File::create(config_file)?;
    file.write_all(dir.as_bytes())?;

    Ok(())
}

pub fn watch_filesystem(
    event_sink: druid::ExtEventSink,
    saved_games: &str,
    callback: impl Fn(&mut State) + Send + Copy + 'static,
) {
    let (tx, rx) = channel();

    let mut watcher: RecommendedWatcher =
        Watcher::new(tx, Duration::from_millis(500)).expect("Could spawn watcher");

    watcher
        .watch(saved_games, RecursiveMode::Recursive)
        .expect("Failed to watch filesystem");

    loop {
        match rx.recv() {
            Ok(_) => {
                event_sink.add_idle_callback(callback);
            }
            Err(e) => println!("watch error: {:?}", e),
        }
    }
}

pub fn list_games(games_dir: &Path) -> Result<Vector<String>> {
    if games_dir.is_dir() {
        let mut games = Vec::new();
        for entry in fs::read_dir(games_dir)
            .with_context(|| "Saved games directory not found".to_string())?
        {
            let entry = entry?;

            if let Some(filename) = entry.path().file_name() {
                if let Ok(filename) = OsString::from(filename).into_string() {
                    games.push(filename);
                }
            }
        }
        return Ok(Vector::from(games));
    }
    Err(anyhow!("Saved games dir is not a directory"))
}

pub fn get_file_path_from_name(name: &str, games_dir: &str) -> String {
    let path = PathBuf::from(games_dir);
    path.join(name).display().to_string()
}

pub fn get_archive_dir() -> Result<PathBuf> {
    match ProjectDirs::from("", "", "Dom5SaveScummer") {
        Some(project_dirs) => {
            let data_dir = project_dirs.data_dir();
            fs::create_dir_all(data_dir)?;
            Ok(PathBuf::from(data_dir))
        }
        None => Err(anyhow!("data dir not found")),
    }
}

pub fn get_config_dir() -> Result<PathBuf> {
    match ProjectDirs::from("", "", "Dom5SaveScummer") {
        Some(project_dirs) => {
            let config_dir = project_dirs.config_dir();
            fs::create_dir_all(config_dir)?;
            Ok(PathBuf::from(config_dir))
        }
        None => Err(anyhow!("data dir not found")),
    }
}

pub fn get_default_dom5_dir() -> Result<PathBuf> {
    if cfg!(target_os = "windows") {
        if let Some(roaming) = BaseDirs::new() {
            let save_dir = PathBuf::from(roaming.config_dir());
            return Ok(save_dir.join("Dominions5trollololol").join("savedgames"));
        }
    } else if cfg!(target_os = "linux") || cfg!(target_os = "macos") {
        if let Some(user_dirs) = UserDirs::new() {
            let save_dir = PathBuf::from(user_dirs.home_dir());
            return Ok(save_dir.join(".dominions5").join("savedgames"));
        }
    }

    Err(anyhow!("OS unknown"))
}

pub fn get_turn_number(save_file_path: &Path) -> Result<u8> {
    let ftherlnd_path = save_file_path.join("ftherlnd");
    if let Ok(file) = fs::read(ftherlnd_path) {
        return Ok(file[0x0e]);
    }

    for entry in fs::read_dir(save_file_path).with_context(|| {
        "Failed to read saved games folder while finding turn number".to_string()
    })? {
        let entry = entry?;
        let path = entry.path();
        if let Some(extension) = path.extension() {
            if extension == "trn" || extension == "2h" {
                if let Ok(file) = fs::read(path) {
                    return Ok(file[0x0e]);
                }
            }
        }
    }

    Err(anyhow!("File not found"))
}

pub fn archive_turn_files(save_file_path: &Path, archive_dir_path: &Path) -> Result<String> {
    match save_file_path.components().last() {
        Some(game_name) => {
            let turn_number = get_turn_number(save_file_path)
                .with_context(|| "Failed to find turn number".to_string())?;
            let mut archive_name = OsString::from(game_name.as_os_str());
            archive_name.push(format!("-{}", turn_number));

            let archive_path = archive_dir_path.join(&archive_name);
            if !archive_path.exists() {
                fs::create_dir_all(&archive_path)
                    .with_context(|| format!("Failed to create dir {:?}", &archive_path))?;
            }

            for entry in fs::read_dir(save_file_path)
                .with_context(|| "Failed to read saved games folder while copying".to_string())?
            {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    let file_name = entry.file_name();
                    let mut archive_path = archive_dir_path.join(&archive_name);
                    archive_path.push(file_name);
                    fs::copy(path, archive_path)
                        .with_context(|| "Failed to copy archive".to_string())?;
                }
            }

            let name = archive_name
                .into_string()
                .expect("Failed to convert osStr name to name");
            Ok(name)
        }
        None => Err(anyhow!("failed to find game name")),
    }
}

pub fn restore_turn_files(archive_file_path: &Path, saved_games_dir_path: &Path) -> Result<String> {
    match archive_file_path.components().last() {
        Some(game_name) => {
            let saved_game_name = OsString::from(game_name.as_os_str())
                .into_string()
                .expect("Failed to convert game name to UTF-8");

            if let Some((saved_game_name_without_turn, _)) = saved_game_name.rsplit_once("-") {
                let restore_path = saved_games_dir_path.join(saved_game_name_without_turn);
                if restore_path.exists() {
                    println!("Game exists, will overwrite");
                    fs::remove_dir_all(&restore_path)?;
                }
                fs::create_dir_all(&restore_path)?;

                for entry in fs::read_dir(archive_file_path).with_context(|| {
                    "Failed to read archive games folder while copying".to_string()
                })? {
                    let entry = entry?;
                    let path = entry.path();
                    if path.is_file() {
                        let file_name = entry.file_name();
                        let mut save_path =
                            saved_games_dir_path.join(&saved_game_name_without_turn);
                        save_path.push(file_name);
                        fs::copy(path, save_path)
                            .with_context(|| "Failed to copy archive".to_string())?;
                    }
                }
            }

            Ok(saved_game_name)
        }
        None => Err(anyhow!("failed to find game name")),
    }
}
