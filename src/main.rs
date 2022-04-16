use actions::*;
use anyhow::Result;
use app::*;
use druid::im::vector;

use druid::{
    AppLauncher,
    WindowDesc,
};
use file_system::*;

mod file_system;
mod app;
mod actions;

fn main() {
    let window = WindowDesc::new(build_ui()).title("Dom5SaveScummer");
    let launcher = AppLauncher::<State>::with_window(window).delegate(Delegate {});

    match get_initial_state() {
        Ok(state) => match state.saved_games_path.clone() {
            Some(saved_games_path) => {
                let archived_games_path = state.archived_games_path.clone();

                let event_sink = launcher.get_external_handle();
                start_saved_games_watcher(event_sink, saved_games_path);

                let event_sink = launcher.get_external_handle();
                start_archived_games_watcher(event_sink, archived_games_path);

                launcher.log_to_console().launch(state).unwrap();
            }
            None => {
                let archived_games_path = state.archived_games_path.clone();

                let event_sink = launcher.get_external_handle();
                start_archived_games_watcher(event_sink, archived_games_path);

                launcher.log_to_console().launch(state).unwrap();
            }
        },
        Err(_) => {
            println!("Failed to load initial state");
        }
    }
}

fn get_initial_state() -> Result<State> {
    let saved_games_dir = get_saved_games_dir_from_config();

    match saved_games_dir {
        Ok(saved_games_dir) => {
            let archived_games_dir = get_archive_dir()?;

            if saved_games_dir.exists() {
                let saved_games = list_games(saved_games_dir.as_path())?;
                let archived_games = list_games(archived_games_dir.as_path())?;

                let state = State {
                    saved_games_path: Some(saved_games_dir.display().to_string()),
                    archived_games_path: archived_games_dir.display().to_string(),
                    saved_games,
                    archived_games,
                    selected_saved_game: None,
                    selected_archived_game: None,
                    archiving: false,
                    restoring: false,
                };

                Ok(state)
            } else {
                let state = State {
                    saved_games_path: None,
                    archived_games_path: archived_games_dir.display().to_string(),
                    saved_games: vector![],
                    archived_games: vector![],
                    selected_saved_game: None,
                    selected_archived_game: None,
                    archiving: false,
                    restoring: false,
                };

                Ok(state)
            }
        }
        Err(e) => Err(e),
    }
}
