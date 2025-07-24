
use super::helper::{self, PathExt, Columns, FileEntry, copy_dir_all, get_file_display_info};
use super::popup::{Popup, PopupMessage, PopupState, OverlayStyle, calculate_popup_position};
use super::navigation::NavigationState;
use iced::{
	alignment, keyboard, mouse, mouse::Button,
	widget::{
		scrollable,
		scrollable::Viewport,
		button, checkbox, column, container, mouse_area, row, text, text_input, Column,
	},
	Alignment, Application, Command, Element, Event, Length, Point, Size, Subscription, Theme,
};
use std::{fs, path::PathBuf, time::SystemTime};

pub struct FileManager {
	navigation: NavigationState,
	ui_state: UIState,
	clipboard: Option<ClipboardItem>,
	files: FileCache,
}

#[derive(Clone)]
struct UIState {
	selected_file: Option<PathBuf>,
	hovered_file: Option<PathBuf>,
	error_message: Option<String>,
	show_hidden: bool,
	columns: Columns,
	scroll_offset: f32,
	popup: Option<Popup>,
	mouse_position: Point,
	loading: bool,
	window_size: Size,
}

#[derive(Clone)]
struct ClipboardItem {
	path: PathBuf,
	is_cut: bool,
}

#[derive(Clone)]
struct FileCache {
	cached_files: Option<(PathBuf, Vec<FileEntry>, SystemTime)>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Message {
	// Navigation messages
	PathInputChanged(String),
	PathSubmitted,
	NavigateUp,
	NavigateHome,
	NavigateBack,
	NavigateForward,
	BackspacePressed,
	
	// File operations
	FileLeftClicked(PathBuf),
	FileRightClicked(PathBuf, Point),
	FileHovered(PathBuf),
	FileUnhovered,
	DeleteSelected,
	
	// Clipboard operations
	CopySelected,
	CutSelected,
	PasteSelected,
	CopyToClipboard(String),
	
	// UI state
	Refresh,
	ToggleHidden,
	ScrollChanged(Viewport),
	MouseMoved(Point),
	WindowResized(Size),
	OverlayClicked,
	MouseButtonPressed(mouse::Button),
	
	// Async operations
	FilesLoaded(Result<Vec<FileEntry>, String>),
	
	// Popup
	PopupMessage(PopupMessage),
}

impl UIState {
	fn new() -> Self {
		Self {
			selected_file: None,
			hovered_file: None,
			error_message: None,
			show_hidden: false,
			columns: Columns::new(),
			scroll_offset: 0.0,
			popup: None,
			mouse_position: Point::ORIGIN,
			loading: true,
			window_size: Size::new(800.0, 600.0),
		}
	}

	fn clear_transient_state(&mut self) {
		self.popup = None;
		self.selected_file = None;
		self.error_message = None;
		self.scroll_offset = 0.0;
	}

	fn set_error(&mut self, message: String) {
		self.error_message = Some(message);
	}
}

impl FileCache {
	fn new() -> Self {
		Self {
			cached_files: None,
		}
	}

	fn get_files(&self) -> Option<&Vec<FileEntry>> {
		self.cached_files.as_ref().map(|(_, files, _)| files)
	}

	fn update_cache(&mut self, path: PathBuf, files: Vec<FileEntry>) {
		self.cached_files = Some((path, files, SystemTime::now()));
	}

	fn clear(&mut self) {
		self.cached_files = None;
	}
}

impl Application for FileManager {
	type Message = Message;
	type Theme = Theme;
	type Executor = iced::executor::Default;
	type Flags = ();

	fn new(_flags: ()) -> (Self, Command<Message>) {
		let navigation = NavigationState::new();
		let load_command = helper::load_files_sync(navigation.current_path.clone());

		(
			Self {
				navigation,
				ui_state: UIState::new(),
				clipboard: None,
				files: FileCache::new(),
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
			// Navigation messages
			Message::PathInputChanged(input) => {
				self.navigation.path_input = input;
				Command::none()
			}
			Message::PathSubmitted => self.handle_path_submission(),
			Message::NavigateUp => self.handle_navigate_up(),
			Message::NavigateHome => self.handle_navigate_home(),
			Message::NavigateBack => self.handle_navigate_back(),
			Message::NavigateForward => self.handle_navigate_forward(),
			Message::BackspacePressed => self.handle_backspace(),

			// File operations
			Message::FileLeftClicked(path) => self.handle_file_click(path),
			Message::FileRightClicked(path, position) => self.handle_right_click(path, position),
			Message::FileHovered(path) => {
				self.ui_state.hovered_file = Some(path);
				Command::none()
			}
			Message::FileUnhovered => {
				self.ui_state.hovered_file = None;
				Command::none()
			}
			Message::DeleteSelected => self.handle_delete(),

			// Clipboard operations
			Message::CopySelected => self.handle_copy(),
			Message::CutSelected => self.handle_cut(),
			Message::PasteSelected => self.handle_paste(),
			Message::CopyToClipboard(text) => {
				self.ui_state.popup = None;
				iced::clipboard::write(text)
			}

			// UI state
			Message::Refresh => self.refresh_directory(),
			Message::ToggleHidden => {
				self.ui_state.show_hidden = !self.ui_state.show_hidden;
				Command::none()
			}
			Message::ScrollChanged(viewport) => {
				self.ui_state.popup = None;
				self.ui_state.scroll_offset = viewport.relative_offset().y;
				Command::none()
			}
			Message::MouseMoved(position) => {
				self.ui_state.mouse_position = position;
				Command::none()
			}
			Message::WindowResized(size) => {
				self.ui_state.popup = None;
				self.ui_state.window_size = size;
				Command::none()
			}
			Message::OverlayClicked => {
				self.ui_state.popup = None;
				Command::none()
			}
			Message::MouseButtonPressed(button) => self.handle_mouse_button(button),

			// Async operations
			Message::FilesLoaded(result) => self.handle_files_loaded(result),

			// Popup
			Message::PopupMessage(popup_msg) => self.handle_popup_message(popup_msg),
		}
	}

	fn view(&self) -> Element<Message> {
		let control_panel = self.view_control_panel();
		let file_list = self.view_file_list();

		let main_content = column![control_panel, file_list]
			.width(Length::Fill)
			.height(Length::Fill);

		if let Some(popup) = &self.ui_state.popup {
			let popup_view = popup.view().map(Message::PopupMessage);
			let overlay = container(popup_view)
				.width(Length::Fill)
				.height(Length::Fill)
				.style(iced::theme::Container::Custom(Box::new(OverlayStyle)));

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
			self.keyboard_subscription(),
			self.event_subscription(),
		])
	}
}

impl FileManager {
	// Handler methods for better organization
	fn handle_path_submission(&mut self) -> Command<Message> {
		let new_path = PathBuf::from(&self.navigation.path_input);
		if new_path.exists() && new_path.is_dir() {
			self.navigate_to_path(new_path)
		} else {
			self.ui_state.set_error("Invalid directory path".to_string());
			Command::none()
		}
	}

	fn handle_navigate_up(&mut self) -> Command<Message> {
		if let Some(parent) = self.navigation.current_path.parent() {
			self.navigate_to_path(parent.to_path_buf())
		} else {
			Command::none()
		}
	}

	fn handle_navigate_home(&mut self) -> Command<Message> {
		if let Some(home) = dirs::home_dir() {
			self.navigate_to_path(home)
		} else {
			Command::none()
		}
	}

	fn handle_navigate_back(&mut self) -> Command<Message> {
		if let Some(history) = self.navigation.go_back() {
			self.files.clear();
			self.ui_state.loading = true;
			// Don't set scroll_offset here - wait for files to load
			let msg = helper::load_files_sync(self.navigation.current_path.clone());
			self.ui_state.scroll_offset = history.scroll;
			println!("ff {:?}", self.ui_state.scroll_offset);
			msg
		} else {
			Command::none()
		}
	}

	fn handle_navigate_forward(&mut self) -> Command<Message> {
		if let Some(history) = self.navigation.go_forward() {
			self.files.clear();
			self.ui_state.loading = true;
			// Don't set scroll_offset here - wait for files to load
			let msg = helper::load_files_sync(self.navigation.current_path.clone());
			self.ui_state.scroll_offset = history.scroll;
			println!("ff {:?}", self.ui_state.scroll_offset);
			msg
		} else {
			Command::none()
		}
	}

	fn handle_backspace(&mut self) -> Command<Message> {
		self.ui_state.popup = None;
		self.handle_navigate_up()
	}

	fn handle_file_click(&mut self, path: PathBuf) -> Command<Message> {
		self.ui_state.popup = None;

		if self.ui_state.selected_file.as_ref() == Some(&path) {
			// Second click - navigate or handle shortcut
			self.handle_double_click(path)
		} else {
			// First click - select file
			self.ui_state.selected_file = Some(path);
			Command::none()
		}
	}

	fn handle_double_click(&mut self, path: PathBuf) -> Command<Message> {
		if path.is_dir() {
			self.navigate_to_path(path)
		} else if path.is_shortcut() {
			self.handle_shortcut_navigation(path)
		} else {
			self.ui_state.selected_file = None;
			Command::none()
		}
	}

	fn handle_shortcut_navigation(&mut self, path: PathBuf) -> Command<Message> {
		if let Some(target_path) = helper::resolve_shortcut(&path) {
			if target_path.exists() {
				if target_path.is_dir() {
					self.navigate_to_path(target_path)
				} else if let Some(parent_dir) = target_path.parent() {
					self.navigation.path_input = target_path.to_string_lossy().to_string();
					self.navigate_to_path(parent_dir.to_path_buf())
				} else {
					Command::none()
				}
			} else {
				self.ui_state.set_error(format!(
					"Shortcut target does not exist: {}",
					target_path.display()
				));
				Command::none()
			}
		} else {
			self.ui_state.set_error("Could not resolve shortcut".to_string());
			Command::none()
		}
	}

	fn handle_right_click(&mut self, path: PathBuf, position: Point) -> Command<Message> {
		let popup_state = PopupState {
			file_path: path,
			position: calculate_popup_position(position, self.ui_state.window_size),
		};
		self.ui_state.popup = Some(Popup::new(popup_state));
		Command::none()
	}

	fn handle_copy(&mut self) -> Command<Message> {
		if let Some(selected) = &self.ui_state.selected_file {
			self.clipboard = Some(ClipboardItem {
				path: selected.clone(),
				is_cut: false,
			});
			self.ui_state.popup = None;
		}
		Command::none()
	}

	fn handle_cut(&mut self) -> Command<Message> {
		if let Some(selected) = &self.ui_state.selected_file {
			self.clipboard = Some(ClipboardItem {
				path: selected.clone(),
				is_cut: true,
			});
			self.ui_state.popup = None;
		}
		Command::none()
	}

	fn handle_paste(&mut self) -> Command<Message> {
		if let Some(clipboard_item) = &self.clipboard {
			let dest_path = self.navigation.current_path.join(
				clipboard_item.path.file_name().unwrap()
			);

			let result = if clipboard_item.is_cut {
				fs::rename(&clipboard_item.path, &dest_path)
					.map_err(|e| format!("Error moving file: {}", e))
			} else {
				self.copy_file_or_dir(&clipboard_item.path, &dest_path)
					.map_err(|e| format!("Error copying file: {}", e))
			};

			match result {
				Ok(_) => {
					if clipboard_item.is_cut {
						self.clipboard = None;
					}
					self.refresh_directory()
				}
				Err(msg) => {
					self.ui_state.set_error(msg);
					Command::none()
				}
			}
		} else {
			Command::none()
		}
	}

	fn handle_delete(&mut self) -> Command<Message> {
		if let Some(selected) = &self.ui_state.selected_file {
			self.delete_file(selected.clone())
		} else {
			Command::none()
		}
	}

	fn handle_mouse_button(&mut self, button: Button) -> Command<Message> {
		match button {
			Button::Back => self.handle_navigate_back(),
			Button::Forward => self.handle_navigate_forward(),
			_ => Command::none(),
		}
	}

	fn handle_files_loaded(&mut self, result: Result<Vec<FileEntry>, String>) -> Command<Message> {
		self.ui_state.loading = false;
		match result {
			Ok(files) => {
				self.files.update_cache(self.navigation.current_path.clone(), files);
				self.ui_state.error_message = None;
				// Restore scroll position after files are loaded
				self.ui_state.scroll_offset = self.navigation.get_current_scroll();
			}
			Err(error) => {
				self.ui_state.set_error(error);
			}
		}
		Command::none()
	}

	fn handle_popup_message(&mut self, popup_msg: PopupMessage) -> Command<Message> {
		if self.ui_state.popup.is_some() {
			match popup_msg {
				PopupMessage::CopyToClipboard(text) => {
					self.ui_state.popup = None;
					iced::clipboard::write(text)
				}
				PopupMessage::ClosePopup => {
					self.ui_state.popup = None;
					Command::none()
				}
				_ => {
					if let Some(popup) = &mut self.ui_state.popup {
						if let Some(new_path) = popup.update(popup_msg) {
							self.ui_state.selected_file = Some(new_path);
							return self.refresh_directory();
						}
					}
					Command::none()
				}
			}
		} else {
			Command::none()
		}
	}

	// Utility methods
	fn navigate_to_path(&mut self, path: PathBuf) -> Command<Message> {
		self.navigation.update_current_scroll(self.ui_state.scroll_offset);
		self.navigation.navigate_to(path);
		self.refresh_directory()
	}

	fn refresh_directory(&mut self) -> Command<Message> {
		self.ui_state.clear_transient_state();
		self.files.clear();
		self.ui_state.loading = true;
		helper::load_files_sync(self.navigation.current_path.clone())
	}

	fn copy_file_or_dir(&self, source: &PathBuf, dest: &PathBuf) -> Result<(), std::io::Error> {
		if source.is_dir() {
			copy_dir_all(source, dest)
		} else {
			fs::copy(source, dest).map(|_| ())
		}
	}

	fn delete_file(&mut self, path: PathBuf) -> Command<Message> {
		self.ui_state.popup = None;
		self.ui_state.error_message = None;
		
		let result = if path.is_dir() {
			fs::remove_dir_all(&path)
		} else {
			fs::remove_file(&path)
		};

		match result {
			Ok(_) => {
				self.ui_state.selected_file = None;
				self.refresh_directory()
			}
			Err(e) => {
				self.ui_state.set_error(format!(
					"Error deleting {}: {}",
					if path.is_dir() { "folder" } else { "file" },
					e
				));
				Command::none()
			}
		}
	}

	// Subscription helpers
	fn keyboard_subscription(&self) -> Subscription<Message> {
		keyboard::on_key_press(|key, modifiers| {
			match key {
				keyboard::Key::Character(c) if modifiers.command() => match c.as_str() {
					"c" => Some(Message::CopySelected),
					"x" => Some(Message::CutSelected),
					"v" => Some(Message::PasteSelected),
					_ => None,
				},
				keyboard::Key::Named(named_key) => match named_key {
					keyboard::key::Named::Backspace => Some(Message::BackspacePressed),
					keyboard::key::Named::F2 => Some(Message::PopupMessage(PopupMessage::StartRename)),
					keyboard::key::Named::Escape => Some(Message::PopupMessage(PopupMessage::ClosePopup)),
					keyboard::key::Named::F5 => Some(Message::Refresh),
					_ => None,
				},
				_ => None,
			}
		})
	}

	fn event_subscription(&self) -> Subscription<Message> {
		iced::event::listen_with(|event, _status| match event {
			Event::Mouse(mouse::Event::CursorMoved { position }) => {
				Some(Message::MouseMoved(position))
			}
			Event::Mouse(mouse::Event::ButtonPressed(button)) => {
				Some(Message::MouseButtonPressed(button))
			}
			Event::Window(_id, iced::window::Event::Resized { width, height }) => {
				Some(Message::WindowResized(Size::new(width as f32, height as f32)))
			}
			_ => None,
		})
	}

	// View methods (kept similar but organized better)
	fn view_control_panel(&self) -> Element<Message> {
		let path_input = text_input("Directory path", &self.navigation.path_input)
			.on_input(Message::PathInputChanged)
			.on_submit(Message::PathSubmitted)
			.padding(8)
			.width(Length::Fill);

		let refresh_button = button("Refresh").on_press(Message::Refresh).padding(8);
		let path_row = row![path_input, refresh_button]
			.spacing(8)
			.align_items(Alignment::Center);

		let nav_buttons = self.create_navigation_buttons();
		let hidden_checkbox = checkbox("Show hidden", self.ui_state.show_hidden)
			.on_toggle(|_| Message::ToggleHidden);

		let nav_row = row![nav_buttons, hidden_checkbox]
			.spacing(8)
			.align_items(Alignment::Center);

		let error_or_headers = if let Some(err) = &self.ui_state.error_message {
			text(err)
				.style(iced::theme::Text::Color(iced::Color::from_rgb8(255, 100, 100)))
				.into()
		} else {
			self.view_table_headers()
		};

		column![path_row, nav_row, error_or_headers]
			.spacing(8)
			.padding(8)
			.into()
	}

	fn create_navigation_buttons(&self) -> Element<Message> {
		let delete_button = self.create_delete_button();
		let up_button = button("Up").on_press(Message::NavigateUp).padding(8);
		let home_button = button("Home").on_press(Message::NavigateHome).padding(8);
		
		let (back_button, forward_button) = self.create_history_buttons();

		row![delete_button, up_button, home_button, back_button, forward_button]
			.spacing(8)
			.align_items(Alignment::Center)
			.into()
	}

	fn create_delete_button(&self) -> Element<Message> {
		if self.ui_state.selected_file.is_some() {
			button(text("Delete").style(iced::theme::Text::Color(iced::Color::from_rgb(0.9, 0.9, 0.9))))
				.style(iced::theme::Button::Destructive)
				.padding(8)
				.on_press(Message::DeleteSelected)
				.into() // Add .into() to convert Button to Element
		} else {
			button(text("Delete").style(iced::theme::Text::Color(iced::Color::from_rgb(0.5, 0.5, 0.5))))
				.style(iced::theme::Button::Secondary)
				.padding(8)
				.into() // Add .into() to convert Button to Element
		}
	}

	fn create_history_buttons(&self) -> (Element<Message>, Element<Message>) {
		let back_button = button("<")
			.on_press_maybe(self.navigation.can_go_back().then_some(Message::NavigateBack))
			.padding(8)
			.style(if self.navigation.can_go_back() {
				iced::theme::Button::Primary
			} else {
				iced::theme::Button::Secondary
			})
			.into(); // Convert to Element

		let forward_button = button(">")
			.on_press_maybe(self.navigation.can_go_forward().then_some(Message::NavigateForward))
			.padding(8)
			.style(if self.navigation.can_go_forward() {
				iced::theme::Button::Primary
			} else {
				iced::theme::Button::Secondary
			})
			.into(); // Convert to Element

		(back_button, forward_button)
	}

	fn view_table_headers(&self) -> Element<Message> {
		let header_color = iced::Color::from_rgb(0.6, 0.6, 0.7);

		let name_header = text("Name")
			.style(iced::theme::Text::Color(header_color))
			.width(Length::FillPortion(self.ui_state.columns.name() as u16));
		let date_header = text("Modified")
			.style(iced::theme::Text::Color(header_color))
			.width(Length::FillPortion(self.ui_state.columns.date() as u16))
			.horizontal_alignment(alignment::Horizontal::Center);
		let size_header = text("Size")
			.style(iced::theme::Text::Color(header_color))
			.width(Length::FillPortion(self.ui_state.columns.size() as u16))
			.horizontal_alignment(alignment::Horizontal::Right);

		row![name_header, date_header, size_header]
			.spacing(8)
			.width(Length::Fill)
			.into()
	}

	fn view_file_list(&self) -> Element<Message> {
		if self.ui_state.loading {
			return self.create_loading_view();
		}

		let files = self.get_filtered_files();
		match files {
			Some(files) => self.create_file_list_view(files),
			None => self.create_error_view(),
		}
	}

	fn create_loading_view(&self) -> Element<Message> {
		container(
			text("Loading...")
				.style(iced::theme::Text::Color(iced::Color::from_rgb(0.7, 0.7, 0.8)))
				.size(16),
		)
		.width(Length::Fill)
		.height(Length::Fill)
		.center_x()
		.center_y()
		.into()
	}

	fn create_error_view(&self) -> Element<Message> {
		container(
			text(format!(
				"Could not read directory contents: {}",
				self.navigation.current_path.display()
			))
			.style(iced::theme::Text::Color(iced::Color::from_rgb8(255, 100, 100))),
		)
		.width(Length::Fill)
		.height(Length::Fill)
		.center_y()
		.center_x()
		.into()
	}

	fn get_filtered_files(&self) -> Option<Vec<&FileEntry>> {
		self.files.get_files().map(|files| {
			if self.ui_state.show_hidden {
				files.iter().collect()
			} else {
				files.iter().filter(|f| !f.is_hidden()).collect()
			}
		})
	}

	fn create_file_list_view(&self, files: Vec<&FileEntry>) -> Element<Message> {
		let file_rows = Column::with_children(
			files.into_iter().map(|file| self.view_file_row(file.clone()))
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
		let is_selected = self.ui_state.selected_file.as_ref() == Some(&file.path());
		let (prefix, text_color) = get_file_display_info(&file);

		let name_text = if file.is_dir() || file.is_shortcut() {
			format!("{} {}", prefix, file.display_name())
		} else {
			file.display_name().clone()
		};

		let row_content = self.create_file_row_content(name_text, text_color, &file);
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
			.on_right_press(Message::FileRightClicked(file_path.clone(), self.ui_state.mouse_position))
			.on_enter(Message::FileHovered(file_path))
			.on_exit(Message::FileUnhovered)
			.into()
	}

	fn create_file_row_content(&self, name_text: String, text_color: iced::Color, file: &FileEntry) -> Element<Message> {
		let name = text(name_text)
			.style(iced::theme::Text::Color(text_color))
			.width(Length::FillPortion(self.ui_state.columns.name() as u16));

		let modified = text(&file.modified())
			.style(iced::theme::Text::Color(iced::Color::from_rgb(0.6, 0.6, 0.7)))
			.width(Length::FillPortion(self.ui_state.columns.date() as u16))
			.horizontal_alignment(alignment::Horizontal::Center);

		let size = text(&file.size())
			.style(iced::theme::Text::Color(iced::Color::from_rgb(0.6, 0.6, 0.7)))
			.width(Length::FillPortion(self.ui_state.columns.size() as u16))
			.horizontal_alignment(alignment::Horizontal::Right);

		row![name, modified, size]
			.spacing(8)
			.width(Length::Fill)
			.align_items(Alignment::Center)
			.into()
	}
}

impl Clone for FileManager {
	fn clone(&self) -> Self {
		Self {
			navigation: self.navigation.clone(),
			ui_state: UIState {
				selected_file: self.ui_state.selected_file.clone(),
				hovered_file: None, // Don't clone transient hover state
				error_message: self.ui_state.error_message.clone(),
				show_hidden: self.ui_state.show_hidden,
				columns: Columns::new(), // Recreate columns
				scroll_offset: self.ui_state.scroll_offset,
				popup: None, // Don't clone popup state
				mouse_position: Point::ORIGIN, // Reset mouse position
				loading: self.ui_state.loading,
				window_size: self.ui_state.window_size,
			},
			clipboard: self.clipboard.clone(),
			files: self.files.clone(),
		}
	}
}