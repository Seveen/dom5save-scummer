use std::path::PathBuf;

use druid::commands::{self, SHOW_OPEN_PANEL};
use druid::im::Vector;
use druid::widget::{
    Button, Either, Flex, Label, List, MainAxisAlignment, Painter, Scroll, Spinner,
};
use druid::{
    AppDelegate, Color, Command, Data, DelegateCtx, Env,
    FileDialogOptions, Handled, Insets, Lens, RenderContext, Selector, Target, Widget, WidgetExt,
};

use crate::actions::*;
use crate::file_system::*;

pub const FINISH_ARCHIVING: Selector<bool> = Selector::new("finish_archiving");
pub const FINISH_RESTORING: Selector<bool> = Selector::new("finish_restoring");

#[derive(Clone, Data, Debug, Lens)]
pub struct State {
    pub saved_games_path: Option<String>,
    pub archived_games_path: String,
    pub saved_games: Vector<String>,
    pub archived_games: Vector<String>,
    pub selected_saved_game: Option<String>,
    pub selected_archived_game: Option<String>,
    pub archiving: bool,
    pub restoring: bool,
}

pub struct Delegate;

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
                save_saved_games_dir_in_config(path)
                    .expect("Failed to save savedgames dir to config file");
                start_saved_games_watcher(ctx.get_external_handle(), String::from(path));
                data.saved_games =
                    list_games(&PathBuf::from(path)).expect("Failed to list saved games");
                data.archived_games = list_games(&PathBuf::from(&data.archived_games_path))
                    .expect("Failed to list archived games");
            }
            Handled::Yes
        } else {
            Handled::No
        }
    }
}

pub fn build_ui() -> impl Widget<State> {
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

    let mut button_column = Flex::column().main_axis_alignment(MainAxisAlignment::SpaceEvenly);
    let mut sub_column = Flex::column();
    sub_column.add_flex_child(either_archiving.padding(Insets::uniform_xy(32.0, 8.0)), 1.0);
    sub_column.add_flex_child(either_restoring.padding(Insets::uniform_xy(32.0, 8.0)), 1.0);
    button_column.add_flex_child(sub_column, 1.0);
    button_column.add_flex_child(select_saved_games_dir(), 1.0);

    container.add_flex_child(button_column.expand(), 1.0);

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

pub fn select_saved_games_dir() -> impl Widget<State> {
    Button::new("Select saved games directory")
        .on_click(|ctx, _, _| {
            let options = FileDialogOptions::new().select_directories();

            ctx.submit_command(SHOW_OPEN_PANEL.with(options));
        })
        .expand_width()
        .padding(Insets::uniform_xy(32.0, 8.0))
}

pub fn list(lens: impl Lens<State, (Option<String>, Vector<String>)>) -> impl Widget<State> {
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