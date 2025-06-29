// main.rs
use crate::helper::Columns;
use crate::helper::FileEntry;
use super::*;
use iced::widget::scrollable;
use iced::widget::scrollable::Viewport;
use iced::{alignment, keyboard, mouse};
use iced::{
    widget::{
        button, checkbox, column, container, mouse_area, row, text, text_input, Column,
    },
    Alignment, Application, Command, Element, Event, Length, Point, Size, Subscription, Theme,
};
use iced::mouse::Button;
use std::{fs, path::PathBuf, time::SystemTime};
use crate::helper::copy_dir_all;

use super::popup::{Popup, PopupMessage, PopupState, OverlayStyle, calculate_popup_position};

pub struct FileManager {
    current_path: PathBuf,
    selected_file: Option<PathBuf>,
    error_message: Option<String>,
    path_input: String,
    show_hidden: bool,
    cached_files: Option<(PathBuf, Vec<FileEntry>, SystemTime)>,
    columns: Columns,
    scroll_offset: f32,
    popup: Option<Popup>,
    hovered_file: Option<PathBuf>,
    mouse_position: Point,
    loading: bool,
    window_size: Size,
    clipboard: Option<(PathBuf, bool)>, // (path, is_cut)
    history: Vec<PathBuf>,
    history_index: usize,
    max_history: usize,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Message {
    PathInputChanged(String),
    PathSubmitted,
    Refresh,
    NavigateUp,
    NavigateHome,
    ToggleHidden,
    FileLeftClicked(PathBuf),
    FileRightClicked(PathBuf, Point),
    FileHovered(PathBuf),
    CopyToClipboard(String),
    CopySelected,
    CutSelected,
    PasteSelected,
    FileUnhovered,
    DeleteSelected,
    BackspacePressed,
    ScrollChanged(Viewport),
    MouseMoved(Point),
    FilesLoaded(Result<Vec<FileEntry>, String>),
    WindowResized(Size),
    OverlayClicked,
    PopupMessage(PopupMessage),
    NavigateBack,
    NavigateForward,
    MouseButtonPressed(mouse::Button),
}

impl Application for FileManager {
    type Message = Message;
    type Theme = Theme;
    type Executor = iced::executor::Default;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        let current_path = env::current_dir()
            .unwrap_or_else(|_| dirs::home_dir().unwrap_or_else(|| PathBuf::from("/")));
        let path_input = current_path.to_string_lossy().to_string();

        let load_command = helper::load_files_sync(current_path.clone(), false);

        (
            Self {
                current_path: current_path.clone(),
                selected_file: None,
                error_message: None,
                path_input,
                show_hidden: false,
                cached_files: None,
                columns: Columns::new(),
                scroll_offset: 0.0,
                popup: None,
                hovered_file: None,
                mouse_position: Point::ORIGIN,
                loading: true,
                window_size: Size::new(800.0, 600.0),
                clipboard: None,
                history: vec![current_path],
                history_index: 0,
                max_history: 50,
            },
            load_command,
        )
    }

    fn title(&self) -> String {
        String::from("File Manager")
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::PathInputChanged(input) => {
                self.path_input = input;
                Command::none()
            }
            Message::PathSubmitted => {
                let new_path = PathBuf::from(&self.path_input);
                if new_path.exists() && new_path.is_dir() {
                    self.navigate_to_path(new_path)
                } else {
                    self.error_message = Some("Invalid directory path".to_string());
                    Command::none()
                }
            }
            Message::Refresh => self.refresh_current_directory(),
            Message::NavigateUp => {
                if let Some(parent) = self.current_path.parent() {
                    self.navigate_to_path(parent.to_path_buf())
                } else {
                    Command::none()
                }
            }
            Message::NavigateHome => {
                if let Some(home) = dirs::home_dir() {
                    self.navigate_to_path(home)
                } else {
                    Command::none()
                }
            }
            Message::ToggleHidden => {
                self.show_hidden = !self.show_hidden;
                self.refresh_current_directory()
            }
            Message::FileLeftClicked(path) => {
                self.popup = None; // Close popup on any file interaction
                
                if self.selected_file.as_ref() == Some(&path) {
                    if path.is_dir() {
                        return self.navigate_to_path(path);
                    }
                    self.selected_file = None;
                } else {
                    self.selected_file = Some(path);
                }
                Command::none()
            }
            Message::CopySelected => {
                if let Some(selected) = &self.selected_file {
                    self.clipboard = Some((selected.clone(), false));
                    self.popup = None;
                    Command::none()
                } else {
                    Command::none()
                }
            }
            Message::CutSelected => {
                if let Some(selected) = &self.selected_file {
                    self.clipboard = Some((selected.clone(), true));
                    self.popup = None;
                    Command::none()
                } else {
                    Command::none()
                }
            }
            Message::PasteSelected => {
                if let Some((source_path, is_cut)) = &self.clipboard {
                    let dest_path = self.current_path.join(source_path.file_name().unwrap());
                    
                    if *is_cut {
                        // Move operation
                        match fs::rename(source_path, &dest_path) {
                            Ok(_) => {
                                self.clipboard = None;
                                self.refresh_current_directory()
                            }
                            Err(e) => {
                                self.error_message = Some(format!("Error moving file: {}", e));
                                Command::none()
                            }
                        }
                    } else {
                        // Copy operation
                        let result = if source_path.is_dir() {
                            copy_dir_all(source_path, &dest_path)
                        } else {
                            fs::copy(source_path, &dest_path).map(|_| ())
                        };
                        
                        match result {
                            Ok(_) => self.refresh_current_directory(),
                            Err(e) => {
                                self.error_message = Some(format!("Error copying file: {}", e));
                                Command::none()
                            }
                        }
                    }
                } else {
                    Command::none()
                }
            }
            Message::PopupMessage(popup_msg) => {
                if let Some(popup) = &mut self.popup {
                    match popup_msg {
                        PopupMessage::CopyFile => {
                            self.popup = None;
                            return Command::perform(async {}, |_| Message::CopySelected);
                        }
                        PopupMessage::CutFile => {
                            self.popup = None;
                            return Command::perform(async {}, |_| Message::CutSelected);
                        }
                        PopupMessage::PasteFile => {
                            self.popup = None;
                            return Command::perform(async {}, |_| Message::PasteSelected);
                        }
                        PopupMessage::CopyToClipboard(text) => {
                            self.popup = None;
                            return iced::clipboard::write(text);
                        }
                        PopupMessage::ClosePopup => {
                            self.popup = None;
                        }
                        _ => {
                            if let Some(new_path) = popup.update(popup_msg) {
                                self.selected_file = Some(new_path);
                                return self.refresh_current_directory();
                            }
                        }
                    }
                }
                Command::none()
            }
            Message::FileRightClicked(path, position) => {
                let popup_state = PopupState {
                    file_path: path,
                    position: calculate_popup_position(position, self.window_size),
                };
                self.popup = Some(Popup::new(popup_state));
                Command::none()
            }
            Message::FileHovered(path) => {
                self.hovered_file = Some(path);
                Command::none()
            }
            Message::CopyToClipboard(text) => {
                self.popup = None; // Close popup after action
                iced::clipboard::write(text)
            }
            Message::FileUnhovered => {
                self.hovered_file = None;
                Command::none()
            }
            Message::OverlayClicked => {
                self.popup = None;
                Command::none()
            }
            Message::DeleteSelected => {
                if let Some(selected) = &self.selected_file {
                    self.delete_file(selected.clone())
                } else {
                    Command::none()
                }
            }
            Message::BackspacePressed => {
                self.popup = None; // Close popup on keyboard interaction
                if let Some(parent) = self.current_path.parent() {
                    self.navigate_to_path(parent.to_path_buf())
                } else {
                    Command::none()
                }
            }
            Message::ScrollChanged(viewport) => {
                self.popup = None; // Close popup on scroll
                self.scroll_offset = viewport.relative_offset().y;
                Command::none()
            }
            Message::MouseMoved(position) => {
                self.mouse_position = position;
                Command::none()
            }
            Message::WindowResized(size) => {
                self.popup = None; // Close popup on window resize
                self.window_size = size;
                Command::none()
            }
            Message::FilesLoaded(result) => {
                self.loading = false;
                match result {
                    Ok(files) => {
                        self.cached_files =
                            Some((self.current_path.clone(), files, SystemTime::now()));
                        self.error_message = None;
                    }
                    Err(error) => {
                        self.error_message = Some(error);
                    }
                }
                Command::none()
            }
            Message::NavigateBack => self.go_back(),
            Message::NavigateForward => self.go_forward(),
            Message::MouseButtonPressed(button) => {
                match button {
                    Button::Back => self.go_back(),
                    Button::Forward => self.go_forward(),
                    _ => Command::none(),
                }
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let control_panel = self.view_control_panel();
        let file_list = self.view_file_list();

        let main_content = column![control_panel, file_list]
            .width(Length::Fill)
            .height(Length::Fill);

        if let Some(popup) = &self.popup {
            let popup_view = popup.view().map(Message::PopupMessage);
            // Create overlay
            let overlay = container(popup_view)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(iced::theme::Container::Custom(Box::new(OverlayStyle)));

            // Layer the popup over the main content
            container(column![
                main_content,
                mouse_area(overlay).on_press(Message::OverlayClicked)
            ])
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        } else {
            main_content.into()
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            keyboard::on_key_press(|key, modifiers| {
                match key {
                    keyboard::Key::Character(c) if modifiers.command() => match c.as_str() {
                        "c" => Some(Message::CopySelected),
                        "x" => Some(Message::CutSelected),
                        "v" => Some(Message::PasteSelected),
                        _ => None,
                    },
                    keyboard::Key::Named(keyboard::key::Named::Backspace) => {
                        Some(Message::BackspacePressed)
                    }
                    keyboard::Key::Named(keyboard::key::Named::F2) => {
                        Some(Message::PopupMessage(PopupMessage::StartRename))
                    }
                    keyboard::Key::Named(keyboard::key::Named::Escape) => {
                        Some(Message::PopupMessage(PopupMessage::ClosePopup))
                    }
                    keyboard::Key::Named(keyboard::key::Named::F5) => {
                        Some(Message::Refresh)
                    }
                    _ => {None}
                }
            }),
            iced::event::listen_with(|event, _status| match event {
                Event::Mouse(mouse::Event::CursorMoved { position }) => {
                    Some(Message::MouseMoved(position))
                }
                Event::Mouse(mouse::Event::ButtonPressed(button)) => {
                    Some(Message::MouseButtonPressed(button))
                }
                Event::Window(_id, iced::window::Event::Resized { width, height }) => Some(
                    Message::WindowResized(Size::new(width as f32, height as f32)),
                ),
                _ => None,
            }),
        ])
    }
}

impl FileManager {
    // Update the navigate_to_path method to use the history
    fn navigate_to_path(&mut self, path: PathBuf) -> Command<Message> {
        self.add_to_history(path.clone());
        self.navigate_to_path_internal(path)
    }

    fn add_to_history(&mut self, path: PathBuf) {
        // If we're not at the end of history, truncate the future history
        if self.history_index + 1 < self.history.len() {
            self.history.truncate(self.history_index + 1);
        }
        
        // Add new path to history
        self.history.push(path);
        
        // Enforce max history size
        if self.history.len() > self.max_history {
            self.history.remove(0);
        } else {
            self.history_index = self.history.len() - 1;
        }
    }
    
    fn can_go_back(&self) -> bool {
        self.history_index > 0
    }
    
    fn can_go_forward(&self) -> bool {
        self.history_index + 1 < self.history.len()
    }
    
    fn go_back(&mut self) -> Command<Message> {
        if self.can_go_back() {
            self.history_index -= 1;
            let path = self.history[self.history_index].clone();
            self.navigate_to_path_internal(path)
        } else {
            Command::none()
        }
    }
    
    fn go_forward(&mut self) -> Command<Message> {
        if self.can_go_forward() {
            self.history_index += 1;
            let path = self.history[self.history_index].clone();
            self.navigate_to_path_internal(path)
        } else {
            Command::none()
        }
    }
    
    fn navigate_to_path_internal(&mut self, path: PathBuf) -> Command<Message> {
        self.popup = None;
        self.current_path = path.clone();
        self.selected_file = None;
        self.error_message = None;
        self.path_input = path.to_string_lossy().to_string();
        self.cached_files = None;
        self.scroll_offset = 0.0;
        self.loading = true;
        helper::load_files_sync(path, self.show_hidden)
    }

    fn refresh_current_directory(&mut self) -> Command<Message> {
        self.popup = None;
        self.cached_files = None;
        self.error_message = None;
        self.loading = true;
        helper::load_files_sync(self.current_path.clone(), self.show_hidden)
    }

    fn delete_file(&mut self, path: PathBuf) -> Command<Message> {
        self.popup = None;
        self.error_message = None;
        let result = if path.is_dir() {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_file(&path)
        };

        if let Err(e) = result {
            self.error_message = Some(format!(
                "Error deleting {}: {}",
                if path.is_dir() { "folder" } else { "file" },
                e
            ));
            Command::none()
        } else {
            self.selected_file = None;
            self.refresh_current_directory()
        }
    }

    fn get_files(&self) -> Option<&Vec<FileEntry>> {
        self.cached_files.as_ref().map(|(_, files, _)| files)
    }

    fn view_control_panel(&self) -> Element<Message> {
        let path_input = text_input("Directory path", &self.path_input)
            .on_input(Message::PathInputChanged)
            .on_submit(Message::PathSubmitted)
            .padding(8)
            .width(Length::Fill);

        let refresh_button = button("Refresh").on_press(Message::Refresh).padding(8);

        let path_row = row![path_input, refresh_button]
            .spacing(8)
            .align_items(Alignment::Center);

        let delete_button = if self.selected_file.is_some() {
            button(
                text("Delete").style(iced::theme::Text::Color(iced::Color::from_rgb(
                    0.9, 0.9, 0.9,
                ))),
            )
            .style(iced::theme::Button::Destructive)
            .padding(8)
            .on_press(Message::DeleteSelected)
        } else {
            button(
                text("Delete").style(iced::theme::Text::Color(iced::Color::from_rgb(
                    0.5, 0.5, 0.5,
                ))),
            )
            .style(iced::theme::Button::Secondary)
            .padding(8)
        };

        let up_button = button("Up").on_press(Message::NavigateUp).padding(8);
        let home_button = button("Home").on_press(Message::NavigateHome).padding(8);
        let back_button = button("<")
            .on_press_maybe(self.can_go_back().then_some(Message::NavigateBack))
            .padding(8)
            .style(if self.can_go_back() {
                iced::theme::Button::Primary
            } else {
                iced::theme::Button::Secondary
            });
        let forward_button = button(">")
            .on_press_maybe(self.can_go_forward().then_some(Message::NavigateForward))
            .padding(8)
            .style(if self.can_go_forward() {
                iced::theme::Button::Primary
            } else {
                iced::theme::Button::Secondary
            });

        let hidden_checkbox =
            checkbox("Show hidden", self.show_hidden).on_toggle(|_| Message::ToggleHidden);

        let nav_row = row![
            delete_button,
            up_button,
            home_button,
            back_button,
            forward_button,
            hidden_checkbox]
            .spacing(8)
            .align_items(Alignment::Center);

        let error_or_headers = if let Some(err) = &self.error_message {
            text(err)
                .style(iced::theme::Text::Color(iced::Color::from_rgb8(
                    255, 100, 100,
                )))
                .into()
        } else {
            self.view_table_headers()
        };

        column![path_row, nav_row, error_or_headers]
            .spacing(8)
            .padding(8)
            .into()
    }

    fn view_table_headers(&self) -> Element<Message> {
        let header_color = iced::Color::from_rgb(0.6, 0.6, 0.7);

        let name_header = text("Name")
            .style(iced::theme::Text::Color(header_color))
            .width(Length::FillPortion(self.columns.name() as u16));
        let date_header = text("Modified")
            .style(iced::theme::Text::Color(header_color))
            .width(Length::FillPortion(self.columns.date() as u16))
            .horizontal_alignment(alignment::Horizontal::Center);
        let size_header = text("Size")
            .style(iced::theme::Text::Color(header_color))
            .width(Length::FillPortion(self.columns.size() as u16))
            .horizontal_alignment(alignment::Horizontal::Right);

        row![name_header, date_header, size_header]
            .spacing(8)
            .width(Length::Fill)
            .into()
    }

    fn view_file_list(&self) -> Element<Message> {
        if self.loading {
            return container(
                text("Loading...")
                    .style(iced::theme::Text::Color(iced::Color::from_rgb(
                        0.7, 0.7, 0.8,
                    )))
                    .size(16),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .into();
        }

        let files: Vec<&FileEntry> = match self.get_files() {
            Some(files) => {
                if self.show_hidden {
                    files.iter().collect()
                } else {
                    files.iter().filter(|f| !f.is_hidden()).collect()
                }
            }
            None => {
                return container(
                    text(format!(
                        "Could not read directory contents: {}",
                        self.current_path.display()
                    ))
                    .style(iced::theme::Text::Color(iced::Color::from_rgb8(
                        255, 100, 100,
                    ))),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .center_y()
                .center_x()
                .into()
            }
        };

        let file_rows = Column::with_children(
            files
                .into_iter()
                .map(|file| self.view_file_row(file.clone())),
        )
        .spacing(4)
        .width(Length::Fill);

        let scrollable_content = scrollable(file_rows)
            .width(Length::Fill)
            .height(Length::Fill)
            .on_scroll(Message::ScrollChanged);

        column![scrollable_content]
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(8)
            .into()
    }

    fn view_file_row(&self, file: FileEntry) -> Element<Message> {
        let is_selected = self.selected_file.as_ref() == Some(&file.path());

        let (prefix, text_color) = if file.is_dir() {
            ("[DIR]", iced::Color::from_rgb(0.5, 0.7, 1.0))
        } else {
            ("", iced::Color::from_rgb(0.7, 0.7, 0.8))
        };

        let name_text = if file.is_dir() {
            format!("{} {}", prefix, file.display_name())
        } else {
            file.display_name().clone()
        };

        let name = text(name_text)
            .style(iced::theme::Text::Color(text_color))
            .width(Length::FillPortion(self.columns.name() as u16));

        let modified = text(&file.modified())
            .style(iced::theme::Text::Color(iced::Color::from_rgb(
                0.6, 0.6, 0.7,
            )))
            .width(Length::FillPortion(self.columns.date() as u16))
            .horizontal_alignment(alignment::Horizontal::Center);

        let size = text(&file.size())
            .style(iced::theme::Text::Color(iced::Color::from_rgb(
                0.6, 0.6, 0.7,
            )))
            .width(Length::FillPortion(self.columns.size() as u16))
            .horizontal_alignment(alignment::Horizontal::Right);

        let row_content = row![name, modified, size]
            .spacing(8)
            .width(Length::Fill)
            .align_items(Alignment::Center);

        let container_style = if is_selected {
            iced::theme::Container::Box
        } else {
            iced::theme::Container::Transparent
        };

        let file_path = file.path().clone();

        let content_container = container(row_content)
            .style(container_style)
            .padding(4)
            .width(Length::Fill);

        mouse_area(content_container)
            .on_press(Message::FileLeftClicked(file_path.clone()))
            .on_right_press(Message::FileRightClicked(
                file_path.clone(),
                self.mouse_position,
            ))
            .on_enter(Message::FileHovered(file_path))
            .on_exit(Message::FileUnhovered)
            .into()
    }
}

impl Clone for FileManager {
    fn clone(&self) -> Self {
        Self {
            current_path: self.current_path.clone(),
            selected_file: self.selected_file.clone(),
            error_message: self.error_message.clone(),
            path_input: self.path_input.clone(),
            show_hidden: self.show_hidden,
            cached_files: self.cached_files.clone(),
            columns: Columns::new(),
            scroll_offset: self.scroll_offset,
            popup: None,
            hovered_file: None,
            mouse_position: Point::ORIGIN,
            loading: self.loading,
            window_size: self.window_size,
            clipboard: self.clipboard.clone(),
            history: self.history.clone(),
            history_index: self.history_index,
            max_history: self.max_history,
        }
    }
}