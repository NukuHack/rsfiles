// popup.rs
use iced::widget::{button, column, container, row, text, text_input};
use iced::{Element, Length, Point, Size};
use std::{fs, path::PathBuf};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PopupState {
    pub file_path: PathBuf,
    pub position: Point,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum PopupMessage {
    CopyToClipboard(String),
    ClosePopup,
    StartRename,
    RenameInputChanged(String),
    ConfirmRename,
    CancelRename,
    CopyFile,
    CutFile,
    PasteFile,
}

pub struct Popup {
    state: PopupState,
    renaming: bool,
    rename_input: String,
    rename_error: Option<String>,
}

impl Popup {
    pub fn new(state: PopupState) -> Self {
        Self {
            state,
            renaming: false,
            rename_input: String::new(),
            rename_error: None,
        }
    }

    pub fn update(&mut self, message: PopupMessage) -> Option<PathBuf> {
        match message {
            PopupMessage::StartRename => {
                self.renaming = true;
                self.rename_input = self.state.file_path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                None
            }
            PopupMessage::RenameInputChanged(input) => {
                self.rename_input = input;
                None
            }
            PopupMessage::ConfirmRename => {
                let old_path = self.state.file_path.clone();
                let new_name = self.rename_input.trim();
                
                if new_name.is_empty() {
                    self.rename_error = Some("Name cannot be empty".to_string());
                    return None;
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
                    return None;
                }

                match fs::rename(&old_path, &new_path) {
                    Ok(_) => {
                        self.renaming = false;
                        self.rename_input.clear();
                        self.rename_error = None;
                        Some(new_path)
                    }
                    Err(e) => {
                        self.rename_error = Some(format!("Error renaming: {}", e));
                        None
                    }
                }
            }
            PopupMessage::CancelRename => {
                self.renaming = false;
                self.rename_input.clear();
                self.rename_error = None;
                None
            }
            _ => None,
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
                .into(),
        ];

        if !self.renaming {
            popup_buttons.insert(
                0,
                button("Rename")
                    .on_press(PopupMessage::StartRename)
                    .padding([4, 8])
                    .style(iced::theme::Button::Secondary)
                    .into(),
            );
        }

        /*
        // Add new buttons
        popup_buttons.insert(
                0,
            button("Copy")
                .on_press(PopupMessage::CopyFile)
                .padding([4, 8])
                .style(iced::theme::Button::Secondary)
                .into(),
        );
        popup_buttons.insert(
                0,
            button("Cut")
                .on_press(PopupMessage::CutFile)
                .padding([4, 8])
                .style(iced::theme::Button::Secondary)
                .into(),
        );
        popup_buttons.insert(
                0,
            button("Paste")
                .on_press(PopupMessage::PasteFile)
                .padding([4, 8])
                .style(iced::theme::Button::Secondary)
                .into(),
        );
        */
        let popup_content = container(
            column![
                text(format!("{}:", if is_dir { "Folder" } else { "File" }))
                    .style(iced::theme::Text::Color(iced::Color::from_rgb(
                        0.9, 0.9, 1.0
                    )))
                    .size(14),
                text(
                    &self.state.file_path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                )
                .style(iced::theme::Text::Color(iced::Color::from_rgb(
                    0.7, 0.7, 0.8
                )))
                .size(12),
                if self.renaming {
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
                            text("").size(12)  // Changed from size(0) to size(12)
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

        container(popup_content)
            .width(Length::Shrink)
            .height(Length::Shrink)
            .style(iced::theme::Container::Transparent)
            .into()
    }
}

pub fn calculate_popup_position(click_position: Point, window_size: Size) -> Point {
    const POPUP_WIDTH: f32 = 200.0;
    const POPUP_HEIGHT: f32 = 150.0;
    const MARGIN: f32 = 10.0;

    let mut x = click_position.x;
    let mut y = click_position.y;

    if x + POPUP_WIDTH > window_size.width {
        x = window_size.width - POPUP_WIDTH - MARGIN;
    }
    if x < MARGIN {
        x = MARGIN;
    }

    if y + POPUP_HEIGHT > window_size.height {
        y = window_size.height - POPUP_HEIGHT - MARGIN;
    }
    if y < MARGIN {
        y = MARGIN;
    }

    Point::new(x, y)
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



// Custom style for the overlay background
pub struct OverlayStyle;

impl iced::widget::container::StyleSheet for OverlayStyle {
    type Style = iced::Theme;

    fn appearance(&self, _style: &Self::Style) -> iced::widget::container::Appearance {
        iced::widget::container::Appearance {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                0.0, 0.0, 0.0, 0.1,
            ))),
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
            text_color: None,
        }
    }
}