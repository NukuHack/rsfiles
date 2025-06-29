// popup.rs
use crate::file_manager::Message;
use iced::widget::{button, column, container, row, text, text_input};
use iced::{Point, Size};

use iced::{Command, Element};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct PopupState {
    pub file_path: PathBuf,
    pub position: Point,
}

#[derive(Debug, Clone)]
pub enum PopupMessage {
    CopyToClipboard(String),
    ClosePopup,
    StartRename(PathBuf),
    RenameInputChanged(String),
    ConfirmRename,
    CancelRename,
}

pub struct Popup {
    state: PopupState,
    rename_input: String,
    rename_error: Option<String>,
    renaming_file: Option<PathBuf>,
}

impl Popup {
    pub fn new(state: PopupState) -> Self {
        Self {
            state,
            rename_input: String::new(),
            rename_error: None,
            renaming_file: None,
        }
    }

    pub fn update(&mut self, message: PopupMessage) -> (Command<PopupMessage>, Option<Message>) {
        match message {
            PopupMessage::CopyToClipboard(text) => (
                Command::none(),
                Some(Message::CopyToClipboard(text))
            ),
            PopupMessage::ClosePopup => (
                Command::none(),
                Some(Message::ClosePopup)),
            PopupMessage::StartRename(path) => {
                self.start_rename(path);
                (Command::none(), None)
            }
            PopupMessage::RenameInputChanged(input) => {
                self.rename_input = input;
                (Command::none(), None)
            }
            PopupMessage::ConfirmRename => {
                if let Some(old_path) = self.renaming_file.take() {
                    let new_name = self.rename_input.trim();
                    
                    if new_name.is_empty() {
                        self.rename_error = Some("Name cannot be empty".to_string());
                        self.renaming_file = Some(old_path);
                        return (Command::none(), None);
                    }

                    let new_path = if old_path.is_dir() {
                        old_path.parent().unwrap().join(new_name)
                    } else {
                        if let Some(ext) = old_path.extension() {
                            let mut new_name = new_name.to_string();
                            if !new_name.ends_with(&format!(".{}", ext.to_string_lossy())) {
                                new_name.push_str(&format!(".{}", ext.to_string_lossy()));
                            }
                            old_path.parent().unwrap().join(new_name)
                        } else {
                            old_path.parent().unwrap().join(new_name)
                        }
                    };

                    if new_path.exists() {
                        self.rename_error = Some("A file/folder with that name already exists".to_string());
                        self.renaming_file = Some(old_path);
                        return (Command::none(), None);
                    }

                    match std::fs::rename(&old_path, &new_path) {
                        Ok(_) => {
                            self.rename_input.clear();
                            (Command::none(), Some(Message::ConfirmRename))
                        }
                        Err(e) => {
                            self.rename_error = Some(format!("Error renaming: {}", e));
                            self.renaming_file = Some(old_path);
                            (Command::none(), None)
                        }
                    }
                } else {
                    (Command::none(), None)
                }
            }
            PopupMessage::CancelRename => {
                self.cancel_rename();
                (Command::none(), None)
            }
        }
    }

    pub fn view(&self) -> Element<PopupMessage> {
        let path = self.state.file_path.to_string_lossy().to_string();
        let is_dir = self.state.file_path.is_dir();

        let mut popup_buttons = vec![
            button("Copy Path")
                .on_press(PopupMessage::CopyToClipboard(path))
                .padding([4, 8])
                .style(iced::theme::Button::Secondary)
                .into(),
            button("Close")
                .on_press(PopupMessage::ClosePopup)
                .padding([4, 8])
                .style(iced::theme::Button::Secondary)
                .into()
        ];

        if self.renaming_file.as_ref() != Some(&self.state.file_path) {
            popup_buttons.insert(0, 
                button("Rename")
                    .on_press(PopupMessage::StartRename(self.state.file_path.clone()))
                    .padding([4, 8])
                    .style(iced::theme::Button::Secondary)
                    .into()
            );
        }

        let popup_content = container(
            column![
                text(format!("{}:", if is_dir { "Folder" } else { "File" }))
                    .style(iced::theme::Text::Color(iced::Color::from_rgb(0.9, 0.9, 1.0)))
                    .size(14),
                text(
                    &self.state
                        .file_path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                )
                .style(iced::theme::Text::Color(iced::Color::from_rgb(0.7, 0.7, 0.8)))
                .size(12),
                if self.renaming_file.as_ref() == Some(&self.state.file_path) {
                    column![
                        text_input("New name", &self.rename_input)
                            .on_input(PopupMessage::RenameInputChanged)
                            .on_submit(PopupMessage::ConfirmRename)
                            .padding(4),
                        row![
                            button("Confirm")
                                .on_press(PopupMessage::ConfirmRename)
                                .padding([4, 8])
                                .style(iced::theme::Button::Primary),
                            button("Cancel")
                                .on_press(PopupMessage::CancelRename)
                                .padding([4, 8])
                                .style(iced::theme::Button::Secondary)
                        ].spacing(8),
                        if let Some(err) = &self.rename_error {
                            text(err)
                                .style(iced::theme::Text::Color(iced::Color::from_rgb8(255, 100, 100)))
                                .size(12)
                        } else {
                            text("").size(0)
                        }
                    ].spacing(8)
                } else {
                    column![].spacing(0)
                },
                row(popup_buttons).spacing(8)
            ]
            .spacing(8)
            .padding(12),
        )
        .style(iced::theme::Container::Custom(Box::new(PopupStyle)));

        popup_content.into()
    }

    fn start_rename(&mut self, path: PathBuf) {
        self.renaming_file = Some(path.clone());
        self.rename_input = path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        self.rename_error = None;
    }

    fn cancel_rename(&mut self) {
        self.renaming_file = None;
        self.rename_input.clear();
        self.rename_error = None;
    }
}

struct PopupStyle;

impl iced::widget::container::StyleSheet for PopupStyle {
    type Style = iced::Theme;

    fn appearance(&self, _style: &Self::Style) -> iced::widget::container::Appearance {
        iced::widget::container::Appearance {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                0.2, 0.2, 0.3, 0.95,
            ))),
            border: iced::Border {
                color: iced::Color::from_rgb(0.4, 0.4, 0.5),
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow: iced::Shadow {
                color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.3),
                offset: iced::Vector::new(2.0, 2.0),
                blur_radius: 10.0,
            },
            text_color: None,
        }
    }
}

pub fn calculate_popup_position(click_position: Point, window_size: Size) -> Point {
    const POPUP_WIDTH: f32 = 200.0;
    const POPUP_HEIGHT: f32 = 120.0;
    const MARGIN: f32 = 10.0;

    let mut x = click_position.x;
    let mut y = click_position.y;

    // Adjust X position to keep popup within window bounds
    if x + POPUP_WIDTH > window_size.width {
        x = window_size.width - POPUP_WIDTH - MARGIN;
    }
    if x < MARGIN {
        x = MARGIN;
    }

    // Adjust Y position to keep popup within window bounds
    if y + POPUP_HEIGHT > window_size.height {
        y = window_size.height - POPUP_HEIGHT - MARGIN;
    }
    if y < MARGIN {
        y = MARGIN;
    }

    Point::new(x, y)
}