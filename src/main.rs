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

mod helper;

fn main() -> iced::Result {
    FileManager::run(Settings {
        window: iced::window::Settings {
            size: Size::new(800.0, 600.0),
            min_size: Some(Size::new(400.0, 300.0)),
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
    loading: bool,
    window_size: Size,
}

#[derive(Debug, Clone)]
struct PopupState {
    file_path: PathBuf,
    position: Point,
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
    FileRightClicked(PathBuf, Point),
    FileHovered(PathBuf),
    CopyToClipboard(String),
    FileUnhovered,
    DeleteSelected,
    BackspacePressed,
    ScrollChanged(Viewport),
    ClosePopup,
    MouseMoved(Point),
    FilesLoaded(Result<Vec<FileEntry>, String>),
    WindowResized(Size),
    OverlayClicked,
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
                current_path,
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
            Message::FileRightClicked(path, position) => {
                self.popup = Some(PopupState {
                    file_path: path,
                    position: self.calculate_popup_position(position),
                });
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
            Message::ClosePopup | Message::OverlayClicked => {
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
                self.window_size = size;
                self.popup = None; // Close popup on window resize
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
        }
    }

    fn view(&self) -> Element<Message> {
        let control_panel = self.view_control_panel();
        let file_list = self.view_file_list();

        let main_content = column![control_panel, file_list]
            .width(Length::Fill)
            .height(Length::Fill);

        if let Some(popup_state) = &self.popup {
            // Create an overlay by wrapping main content with popup
            let overlay = self.view_popup_overlay(popup_state);

            // Use container to layer the popup over the main content
            container(column![main_content, overlay])
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
                } else if let keyboard::Key::Named(keyboard::key::Named::Escape) = key {
                    Some(Message::ClosePopup)
                } else {
                    None
                }
            }),
            iced::event::listen_with(|event, _status| match event {
                Event::Mouse(mouse::Event::CursorMoved { position }) => {
                    Some(Message::MouseMoved(position))
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
    fn navigate_to_path(&mut self, path: PathBuf) -> Command<Message> {
        self.current_path = path;
        self.selected_file = None;
        self.error_message = None;
        self.path_input = self.current_path.to_string_lossy().to_string();
        self.cached_files = None;
        self.scroll_offset = 0.0;
        self.loading = true;
        self.popup = None;
        helper::load_files_sync(self.current_path.clone(), self.show_hidden)
    }

    fn refresh_current_directory(&mut self) -> Command<Message> {
        self.cached_files = None;
        self.error_message = None;
        self.loading = true;
        self.popup = None;
        helper::load_files_sync(self.current_path.clone(), self.show_hidden)
    }

    fn delete_file(&mut self, path: PathBuf) -> Command<Message> {
        self.error_message = None;
        self.popup = None;
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
        /*let back_button = button("Back").on_press(Message::NavigateUp).padding(8);
        let forw_button = button("Forward").on_press(Message::NavigateUp).padding(8);*/
        let home_button = button("Home").on_press(Message::NavigateHome).padding(8);
        let hidden_checkbox =
            checkbox("Show hidden", self.show_hidden).on_toggle(|_| Message::ToggleHidden);

        let nav_row = row![delete_button, up_button, home_button, hidden_checkbox]
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
                    files.iter().filter(|f| !f.is_hidden).collect()
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

        let modified = text(&file.modified)
            .style(iced::theme::Text::Color(iced::Color::from_rgb(
                0.6, 0.6, 0.7,
            )))
            .width(Length::FillPortion(self.columns.date as u16))
            .horizontal_alignment(alignment::Horizontal::Center);

        let size = text(&file.size)
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

    fn view_popup_overlay(&self, popup_state: &PopupState) -> Element<Message> {
        let path = popup_state.file_path.to_string_lossy().to_string();
        let is_dir = popup_state.file_path.is_dir();

        // Create the popup content
        let popup_content = container(
            column![
                text(format!("{}:", if is_dir { "Folder" } else { "File" }))
                    .style(iced::theme::Text::Color(iced::Color::from_rgb(
                        0.9, 0.9, 1.0
                    )))
                    .size(14),
                text(
                    &popup_state
                        .file_path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                )
                .style(iced::theme::Text::Color(iced::Color::from_rgb(
                    0.7, 0.7, 0.8
                )))
                .size(12),
                row![
                    button("Copy Path")
                        .on_press(Message::CopyToClipboard(path))
                        .padding([4, 8])
                        .style(iced::theme::Button::Secondary),
                    button("Close")
                        .on_press(Message::ClosePopup)
                        .padding([4, 8])
                        .style(iced::theme::Button::Secondary)
                ]
                .spacing(8)
            ]
            .spacing(8)
            .padding(12),
        )
        .style(iced::theme::Container::Custom(Box::new(PopupStyle)))
        .width(Length::Shrink)
        .height(Length::Shrink);

        // Position the popup using padding (as you were doing)
        let positioned_popup = container(popup_content)
            .width(Length::Shrink)
            .height(Length::Shrink)
            .style(iced::theme::Container::Transparent)
            .padding([
                popup_state.position.y as u16,
                0,
                0,
                popup_state.position.x as u16,
            ]);

        // Create the overlay
        let overlay = container(positioned_popup)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(iced::theme::Container::Custom(Box::new(OverlayStyle)));

        // Make the overlay clickable to dismiss
        mouse_area(overlay).on_press(Message::OverlayClicked).into()
    }

    fn calculate_popup_position(&self, click_position: Point) -> Point {
        const POPUP_WIDTH: f32 = 200.0;
        const POPUP_HEIGHT: f32 = 120.0;
        const MARGIN: f32 = 10.0;

        let mut x = click_position.x;
        let mut y = click_position.y;

        // Adjust X position to keep popup within window bounds
        if x + POPUP_WIDTH > self.window_size.width {
            x = self.window_size.width - POPUP_WIDTH - MARGIN;
        }
        if x < MARGIN {
            x = MARGIN;
        }

        // Adjust Y position to keep popup within window bounds
        if y + POPUP_HEIGHT > self.window_size.height {
            y = self.window_size.height - POPUP_HEIGHT - MARGIN;
        }
        if y < MARGIN {
            y = MARGIN;
        }

        Point::new(x, y - self.window_size.height / 2.0) // Add back the control panel height for final position
    }
}

// Custom styles for the popup
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
struct OverlayStyle;

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

struct Columns {
    name: f32,
    date: f32,
    size: f32,
}

impl Columns {
    fn new() -> Self {
        Self {
            name: 50.0,
            date: 25.0,
            size: 20.0,
        }
    }
}

#[derive(Clone, Debug)]
struct FileEntry {
    path: PathBuf,
    display_name: String,
    is_dir: bool,
    modified: String,
    size: String,
    is_hidden: bool,
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
        }
    }
}
