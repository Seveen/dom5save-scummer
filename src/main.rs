use anyhow::Result;
use druid::commands::{SHOW_OPEN_PANEL, self};
use druid::im::{Vector, vector};
use druid::widget::{Button, Either, Flex, Label, List, Painter, Scroll, Spinner};
use druid::{
    AppDelegate, AppLauncher, Color, Command, Data, DelegateCtx, Env, ExtEventSink, Handled, Lens,
    RenderContext, Selector, Target, UnitPoint, Widget, WidgetExt, WindowDesc, FileDialogOptions,
};
use file_system::*;
use std::path::PathBuf;
use std::thread;

mod file_system;

const FINISH_ARCHIVING: Selector<bool> = Selector::new("finish_archiving");
const FINISH_RESTORING: Selector<bool> = Selector::new("finish_restoring");

#[derive(Clone, Data, Debug, Lens)]
pub struct State {
    saved_games_path: Option<String>,
    archived_games_path: String,
    saved_games: Vector<String>,
    archived_games: Vector<String>,
    selected_saved_game: Option<String>,
    selected_archived_game: Option<String>,
    archiving: bool,
    restoring: bool,
}

struct Delegate;

impl AppDelegate<State> for Delegate {
    fn command(
        &mut self,
        ctx: &mut DelegateCtx,
        _target: Target,
        cmd: &Command,
        data: &mut State,
        _env: &Env,
    ) -> Handled {
        if let Some(&archiving) = cmd.get(FINISH_ARCHIVING) {
            data.archiving = archiving;
            Handled::Yes
        } else if let Some(&restoring) = cmd.get(FINISH_RESTORING) {
            data.restoring = restoring;
            Handled::Yes
        } else if let Some(file_info) = cmd.get(commands::OPEN_FILE) {
            if let Some(path) = file_info.path().to_str() {
                data.saved_games_path = Some(String::from(path));
                save_saved_games_dir_in_config(path).expect("Failed to save savedgames dir to config file");
                start_saved_games_watcher(ctx.get_external_handle(), String::from(path));
                data.saved_games = list_games(&PathBuf::from(path)).expect("Failed to list saved games");
                data.archived_games = list_games(&PathBuf::from(&data.archived_games_path)).expect("Failed to list archived games");
            }
            Handled::Yes
        } else {
            Handled::No
        }
    }
}

fn main() {
    let window = WindowDesc::new(build_ui()).title("Dom5SaveScummer");
    let launcher = AppLauncher::<State>::with_window(window).delegate(Delegate {});

    match get_initial_state() {
        Ok(state) => {
            match state.saved_games_path.clone() {
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
                },
            }
        }
        Err(_) => {
            println!("Failed to load initial state");
        }
    }
}

fn start_saved_games_watcher(event_sink: ExtEventSink, saved_games_path: String) {
    thread::spawn(move || {
        watch_filesystem(event_sink, &saved_games_path, move |data: &mut State| {
            if let Some(saved_games_path) = &data.saved_games_path {
                data.saved_games = list_games(&PathBuf::from(saved_games_path))
                    .expect("Failed to list saved games in watcher");
            }
        })
    });
}

fn start_archived_games_watcher(event_sink: ExtEventSink, archived_games_path: String) {
    thread::spawn(move || {
        watch_filesystem(event_sink, &archived_games_path, move |data: &mut State| {
            data.archived_games =
                list_games(&PathBuf::from(data.archived_games_path.clone()))
                    .expect("Failed to list archived games in watcher");
        })
    });
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

fn start_archiving(
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

fn start_restoring(
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

fn build_ui() -> impl Widget<State> {
    let mut container = Flex::row();

    container.add_flex_child(
        list((State::selected_saved_game, State::saved_games)).expand_width(),
        1.0,
    );

    let either_archiving = Either::new(
        |data, _| data.archiving,
        Flex::column()
            .with_child(Label::new("Processing...").padding(5.0))
            .with_child(Spinner::new()),
        Button::new("Archive")
            .on_click(|ctx, data: &mut State, _| {
                data.archiving = true;
                if let Some(saved) = &data.saved_games_path {
                    if let Some(selected) = &data.selected_saved_game {
                        let selected = selected.clone();
                        let saved = saved.clone();
                        let archived = data.archived_games_path.clone();
                        start_archiving(ctx.get_external_handle(), selected, saved, archived);
                    }
                }
            })
            .expand_width(),
    );

    let either_restoring = Either::new(
        |data, _| data.restoring,
        Flex::column()
            .with_child(Label::new("Processing...").padding(5.0))
            .with_child(Spinner::new()),
        Button::new("Restore")
            .on_click(|ctx, data: &mut State, _| {
                data.restoring = true;
                if let Some(saved) = &data.saved_games_path {
                    if let Some(selected) = &data.selected_archived_game {
                        let selected = selected.clone();
                        let saved = saved.clone();
                        let archived = data.archived_games_path.clone();
                        start_restoring(ctx.get_external_handle(), selected, saved, archived);
                    }
                }
            })
            .expand_width(),
    );

    let mut button_column = Flex::column();
    button_column.add_flex_child(either_archiving, 1.0);
    button_column.add_flex_child(either_restoring, 1.0);

    container.add_flex_child(
        button_column
            .align_vertical(UnitPoint::CENTER)
            .expand_height(),
        1.0,
    );

    container.add_flex_child(
        list((State::selected_archived_game, State::archived_games)).expand_width(),
        1.0,
    );

    Either::new(
        |data, _| data.saved_games_path.is_some(),
        container,
        select_saved_games_dir(),
    )
}

fn select_saved_games_dir() -> impl Widget<State> {
    Button::new("Select saved games directory").on_click(|ctx, _, _| {
        let options = FileDialogOptions::new().select_directories();

        ctx.submit_command(SHOW_OPEN_PANEL.with(options));
    })
}

fn list(lens: impl Lens<State, (Option<String>, Vector<String>)>) -> impl Widget<State> {
    Scroll::new(
        List::new(|| {
            Label::dynamic(|data: &(Option<String>, String), _| data.1.to_string())
                .on_click(|_, data: &mut (Option<String>, String), _| data.0 = Some(data.1.clone()))
                .background(Painter::new(|ctx, data: &(Option<String>, String), _| {
                    if let Some(selected) = &data.0 {
                        if selected.same(&data.1) {
                            let size = ctx.size().to_rect();
                            ctx.fill(size, &Color::BLUE)
                        }
                    }
                }))
                .expand_width()
        })
        .expand_width()
        .lens(lens),
    )
    .vertical()
}
