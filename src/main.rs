#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use iced::Settings;
use iced::Size;
use iced::Application;
use std::env;

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