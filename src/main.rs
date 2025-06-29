#[allow(unused_imports)]
use iced::{
    alignment, keyboard, mouse,
    widget::{
        button, checkbox, column, container, mouse_area, row, scrollable, scrollable::Viewport,
        text, text_input, Column,
    },
    Alignment, Application, Command, Element, Event, Length, Point, Settings, Size, Subscription,
    Theme,
};
#[allow(unused_imports)]
use std::{env, fs, path::PathBuf, time::SystemTime};

mod file_manager;
mod helper;
mod popup;

fn main() -> iced::Result {
    file_manager::FileManager::run(Settings {
        window: iced::window::Settings {
            size: Size::new(800.0, 600.0),
            min_size: Some(Size::new(400.0, 300.0)),
            ..Default::default()
        },
        ..Default::default()
    })
}