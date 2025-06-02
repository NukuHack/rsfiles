use iced::widget::{button, mouse_area};
use iced::{
    alignment, keyboard, mouse,
    widget::{
        checkbox, column, container, row, scrollable, scrollable::Viewport, text, text_input,
        Column,
    },
    Alignment, Application, Command, Element, Event, Length, Point, Settings, Subscription, Theme,
};
use std::{env, fs, path::PathBuf, time::SystemTime};

mod helper;

fn main() -> iced::Result {
    FileManager::run(Settings {
        window: iced::window::Settings {
            size: iced::Size::new(800.0, 600.0),
            min_size: Some(iced::Size::new(400.0, 300.0)),
            ..Default::default()
        },
        ..Default::default()
    })
}

struct FileManager {
    current_path: PathBuf,
    selected_file: Option<PathBuf>,
    error_message: Option<String>,
    path_input: String,
    show_hidden: bool,
    cached_files: Option<(PathBuf, Vec<FileEntry>, SystemTime)>,
    columns: Columns,
    scroll_offset: f32,
    popup: Option<PopupState>,
    hovered_file: Option<PathBuf>,
    mouse_position: Point,
}

#[derive(Debug, Clone)]
struct PopupState {
    file_path: PathBuf,
}

#[derive(Debug, Clone)]
enum Message {
    PathInputChanged(String),
    PathSubmitted,
    Refresh,
    NavigateUp,
    NavigateHome,
    ToggleHidden,
    FileLeftClicked(PathBuf),
    FileRightClicked(PathBuf),
    FileHovered(PathBuf),
    CopyToClipboard(String),
    FileUnhovered,
    DeleteSelected,
    BackspacePressed,
    ScrollChanged(Viewport),
    ClosePopup,
    MouseMoved(Point),
}

impl Application for FileManager {
    type Message = Message;
    type Theme = Theme;
    type Executor = iced::executor::Default;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        // Use current working directory instead of home directory
        let current_path = env::current_dir()
            .unwrap_or_else(|_| dirs::home_dir().unwrap_or_else(|| PathBuf::from("/")));
        let path_input = current_path.to_string_lossy().to_string();

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
            },
            Command::none(),
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
                self.update_path(PathBuf::from(&self.path_input));
                Command::none()
            }
            Message::Refresh => {
                self.cached_files = None;
                self.path_input = self.current_path.to_string_lossy().to_string();
                self.error_message = None;
                Command::none()
            }
            Message::NavigateUp => {
                if let Some(parent) = self.current_path.parent() {
                    self.update_path(parent.to_path_buf());
                }
                Command::none()
            }
            Message::NavigateHome => {
                if let Some(home) = dirs::home_dir() {
                    self.update_path(home);
                }
                Command::none()
            }
            Message::ToggleHidden => {
                self.show_hidden = !self.show_hidden;
                self.cached_files = None; // Refresh to show/hide files
                Command::none()
            }
            Message::FileLeftClicked(path) => {
                if self.selected_file.as_ref() == Some(&path) {
                    // Double-click behavior: navigate into directories
                    if path.is_dir() {
                        self.update_path(PathBuf::from(&path));
                    }
                    self.selected_file = None;
                } else {
                    // Single-click behavior: select the file
                    self.selected_file = Some(path);
                }
                Command::none()
            }
            Message::FileRightClicked(path) => {
                self.popup = Some(PopupState { file_path: path });
                Command::none()
            }
            Message::FileHovered(path) => {
                self.hovered_file = Some(path);
                Command::none()
            }
            Message::CopyToClipboard(text) => {
                iced::clipboard::write(text)

                /*match clipboard::write(text) {
                    Ok(_) => Message::CopySuccess,
                    Err(e) => Message::CopyError(format!("{:?}", e)),
                }*/
            }
            Message::FileUnhovered => {
                self.hovered_file = None;
                Command::none()
            }
            Message::ClosePopup => {
                self.popup = None;
                Command::none()
            }
            Message::DeleteSelected => {
                self.delete_selected_file();
                Command::none()
            }
            Message::BackspacePressed => {
                if let Some(parent) = self.current_path.parent() {
                    self.update_path(parent.to_path_buf());
                }
                Command::none()
            }
            Message::ScrollChanged(viewport) => {
                self.scroll_offset = viewport.relative_offset().y;
                Command::none()
            }
            Message::MouseMoved(position) => {
                self.mouse_position = position;
                Command::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let control_panel = self.view_control_panel();
        let file_list = self.view_file_list();

        let main_content = column![control_panel, file_list]
            .width(Length::Fill)
            .height(Length::Fill);

        // Add popup overlay if present
        if let Some(popup_state) = &self.popup {
            let popup = self.view_popup(popup_state);
            // Use a simple overlay approach instead of stack widget
            container(column![main_content, popup])
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            main_content.into()
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            keyboard::on_key_press(|key, _modifiers| {
                if let keyboard::Key::Named(keyboard::key::Named::Backspace) = key {
                    Some(Message::BackspacePressed)
                } else {
                    None
                }
            }),
            iced::event::listen_with(|event, _status| match event {
                Event::Mouse(mouse::Event::CursorMoved { position }) => {
                    Some(Message::MouseMoved(position))
                }
                _ => None,
            }),
        ])
    }
}

struct Columns {
    name: f32,
    date: f32,
    size: f32,
}

impl Columns {
    fn new() -> Self {
        Self {
            name: 50.0, // 50% for name
            date: 25.0, // 25% for date
            size: 20.0, // 20% for size
                        //rest is padding .
        }
    }
}

#[derive(Clone)]
struct FileEntry {
    path: PathBuf,
    display_name: String,
    is_dir: bool,
    modified: String,
    size: String,
    is_hidden: bool,
}

impl FileManager {
    fn update_path(&mut self, new_path: PathBuf) {
        if new_path.exists() && new_path.is_dir() {
            self.current_path = new_path;
            self.selected_file = None;
            self.error_message = None;
            self.path_input = self.current_path.to_string_lossy().to_string();
            self.cached_files = None;
            self.scroll_offset = 0.0;
        } else {
            self.error_message = Some("Invalid directory path".to_string());
        }
    }

    fn get_files(&mut self) -> Option<&Vec<FileEntry>> {
        // Check if cache is still valid
        if let Some((cached_path, _, timestamp)) = &self.cached_files {
            if cached_path == &self.current_path {
                if let Ok(metadata) = fs::metadata(&self.current_path) {
                    if let Ok(modified) = metadata.modified() {
                        if modified <= *timestamp {
                            return self.cached_files.as_ref().map(|(_, files, _)| files);
                        }
                    }
                }
            }
        }

        // Read directory and cache results
        if let Ok(entries) = fs::read_dir(&self.current_path) {
            let mut files = Vec::new();
            let mut latest_modified = SystemTime::UNIX_EPOCH;

            for entry in entries.filter_map(|e| e.ok()) {
                if let Ok(metadata) = entry.metadata() {
                    let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                    if modified > latest_modified {
                        latest_modified = modified;
                    }

                    let path = entry.path();
                    let display_name = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    let is_hidden = display_name.starts_with('.');

                    let modified_str = metadata
                        .modified()
                        .map(helper::format_time)
                        .unwrap_or_else(|_| "Unknown".to_string());

                    let size_str = if metadata.is_dir() {
                        "".to_string()
                    } else {
                        helper::format_size(metadata.len())
                    };

                    files.push(FileEntry {
                        path,
                        display_name,
                        is_dir: metadata.is_dir(),
                        modified: modified_str,
                        size: size_str,
                        is_hidden,
                    });
                }
            }

            files.sort_by(|a, b| helper::compare_paths(&a.path, &b.path));
            self.cached_files = Some((self.current_path.clone(), files, latest_modified));
            self.cached_files.as_ref().map(|(_, files, _)| files)
        } else {
            self.error_message = Some("Could not read directory".to_string());
            None
        }
    }

    fn delete_selected_file(&mut self) {
        if let Some(selected) = &self.selected_file {
            self.error_message = None;
            let result = if selected.is_dir() {
                fs::remove_dir_all(selected)
            } else {
                fs::remove_file(selected)
            };

            if let Err(e) = result {
                self.error_message = Some(format!(
                    "Error deleting {}: {}",
                    if selected.is_dir() { "folder" } else { "file" },
                    e
                ));
            } else {
                self.selected_file = None;
                self.cached_files = None;
            }
        }
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

        let up_button = button("Up").on_press(Message::NavigateUp).padding(8);
        let home_button = button("Home").on_press(Message::NavigateHome).padding(8);
        let hidden_checkbox =
            checkbox("Show hidden", self.show_hidden).on_toggle(|_| Message::ToggleHidden);

        let nav_row = row![up_button, home_button, hidden_checkbox]
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
            .width(Length::FillPortion(self.columns.name as u16));
        let date_header = text("Modified")
            .style(iced::theme::Text::Color(header_color))
            .width(Length::FillPortion(self.columns.date as u16))
            .horizontal_alignment(alignment::Horizontal::Center);
        let size_header = text("Size")
            .style(iced::theme::Text::Color(header_color))
            .width(Length::FillPortion(self.columns.size as u16))
            .horizontal_alignment(alignment::Horizontal::Right);

        row![name_header, date_header, size_header]
            .spacing(8)
            .width(Length::Fill)
            .into()
    }

    fn view_file_list(&self) -> Element<Message> {
        let mut files_clone = self.clone();
        let files: Vec<FileEntry> = match files_clone.get_files() {
            Some(files) => files
                .iter()
                .filter(|f| self.show_hidden || !f.is_hidden)
                .cloned()
                .collect(),
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

        let file_rows =
            Column::with_children(files.into_iter().map(|file| self.view_file_row(file)))
                .spacing(4)
                .width(Length::Fill);

        let delete_button = if self.selected_file.is_some() {
            button(
                text("Delete").style(iced::theme::Text::Color(iced::Color::from_rgb(
                    0.9, 0.9, 0.9,
                ))),
            )
            .style(iced::theme::Button::Destructive)
            .width(Length::Fill)
            .padding(8)
            .on_press(Message::DeleteSelected)
        } else {
            button(
                text("Delete").style(iced::theme::Text::Color(iced::Color::from_rgb(
                    0.5, 0.5, 0.5,
                ))),
            )
            .style(iced::theme::Button::Secondary)
            .width(Length::Fill)
            .padding(8)
        };

        let scrollable_content = scrollable(file_rows)
            .width(Length::Fill)
            .height(Length::Fill)
            .on_scroll(Message::ScrollChanged);

        column![scrollable_content, delete_button]
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(8)
            .into()
    }

    fn view_file_row(&self, file: FileEntry) -> Element<Message> {
        let is_selected = self.selected_file.as_ref() == Some(&file.path);

        let (prefix, text_color) = if file.is_dir {
            ("[DIR]", iced::Color::from_rgb(0.5, 0.7, 1.0))
        } else {
            ("", iced::Color::from_rgb(0.7, 0.7, 0.8))
        };

        let name_text = if file.is_dir {
            format!("{} {}", prefix, file.display_name)
        } else {
            file.display_name.clone()
        };

        let name = text(name_text)
            .style(iced::theme::Text::Color(text_color))
            .width(Length::FillPortion(self.columns.name as u16));

        let modified = text(file.modified)
            .style(iced::theme::Text::Color(iced::Color::from_rgb(
                0.6, 0.6, 0.7,
            )))
            .width(Length::FillPortion(self.columns.date as u16))
            .horizontal_alignment(alignment::Horizontal::Center);

        let size = text(file.size)
            .style(iced::theme::Text::Color(iced::Color::from_rgb(
                0.6, 0.6, 0.7,
            )))
            .width(Length::FillPortion(self.columns.size as u16))
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

        let file_path = file.path.clone();

        // Create the content container
        let content_container = container(row_content)
            .style(container_style)
            .padding(4)
            .width(Length::Fill);

        // Use mouse_area to handle both left and right clicks properly
        mouse_area(content_container)
            .on_press(Message::FileLeftClicked(file_path.clone()))
            .on_right_press(Message::FileRightClicked(file_path.clone()))
            .on_enter(Message::FileHovered(file_path))
            .on_exit(Message::FileUnhovered)
            .into()
    }

    fn view_popup(&self, popup_state: &PopupState) -> Element<Message> {
        let path = popup_state.file_path.to_string_lossy().to_string();
        let dir = popup_state.file_path.is_dir();
        let popup_content = container(
            column![
                text(format!(
                    "Right-clicked {}:",
                    if dir { "folder" } else { "file" }
                ))
                .style(iced::theme::Text::Color(iced::Color::from_rgb(
                    0.8, 0.8, 0.9
                ))),
                text(&path)
                    .style(iced::theme::Text::Color(iced::Color::from_rgb(
                        0.9, 0.9, 1.0
                    )))
                    .size(12),
                row![
                    button("Copy Path")
                        .on_press(Message::CopyToClipboard((&path).to_string()))
                        .padding(4),
                    button("Properties").padding(4),
                    button("Close").on_press(Message::ClosePopup).padding(4)
                ]
                .spacing(4)
            ]
            .spacing(8)
            .padding(12),
        )
        .style(iced::theme::Container::Box)
        .padding(2);

        container(popup_content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .into()
    }
}

// Clone implementation for FileManager (needed for mutable borrow in view_file_list)
impl Clone for FileManager {
    fn clone(&self) -> Self {
        Self {
            current_path: self.current_path.clone(),
            selected_file: self.selected_file.clone(),
            error_message: self.error_message.clone(),
            path_input: self.path_input.clone(),
            show_hidden: self.show_hidden,
            cached_files: self.cached_files.clone(),
            columns: Columns::new(), // Reset to default
            scroll_offset: self.scroll_offset,
            popup: None, // Reset popup state in clone
            hovered_file: None,
            mouse_position: Point::ORIGIN,
        }
    }
}
