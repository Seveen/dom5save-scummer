use druid::{
    ExtEventSink, Target,
};

use std::path::PathBuf;
use std::thread;

use crate::app::*;
use crate::file_system::*;

pub fn start_saved_games_watcher(event_sink: ExtEventSink, saved_games_path: String) {
    thread::spawn(move || {
        watch_filesystem(event_sink, &saved_games_path, move |data: &mut State| {
            if let Some(saved_games_path) = &data.saved_games_path {
                data.saved_games = list_games(&PathBuf::from(saved_games_path))
                    .expect("Failed to list saved games in watcher");
            }
        })
    });
}

pub fn start_archived_games_watcher(event_sink: ExtEventSink, archived_games_path: String) {
    thread::spawn(move || {
        watch_filesystem(event_sink, &archived_games_path, move |data: &mut State| {
            data.archived_games = list_games(&PathBuf::from(data.archived_games_path.clone()))
                .expect("Failed to list archived games in watcher");
        })
    });
}


pub fn start_archiving(
    sink: ExtEventSink,
    selected: String,
    saved_games_path: String,
    archived_games_path: String,
) {
    thread::spawn(move || {
        let save_file = get_file_path_from_name(&selected, &saved_games_path);
        // FIXME : error handling
        let _ = archive_turn_files(
            &PathBuf::from(save_file),
            &PathBuf::from(archived_games_path),
        );
        sink.submit_command(FINISH_ARCHIVING, false, Target::Auto)
            .expect("Command failed to submit");
    });
}

pub fn start_restoring(
    sink: ExtEventSink,
    selected: String,
    saved_games_path: String,
    archived_games_path: String,
) {
    thread::spawn(move || {
        let save_file = get_file_path_from_name(&selected, &archived_games_path);
        // FIXME : error handling
        let _ = restore_turn_files(&PathBuf::from(save_file), &PathBuf::from(saved_games_path));
        sink.submit_command(FINISH_RESTORING, false, Target::Auto)
            .expect("Command failed to submit");
    });
}