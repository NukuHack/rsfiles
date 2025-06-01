use eframe::egui;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

mod helper;

#[derive(Default)]
struct FileManagerApp {
    current_path: PathBuf,
    selected_file: Option<PathBuf>,
    error_message: Option<String>,
    path_input: String,
    columns: Columns,
    left_padding: f32,
    show_hidden: bool,
    cached_files: Option<(PathBuf, Vec<FileEntry>, SystemTime)>,
}

#[derive(Default)]
struct Columns {
    name: f32,
    date: f32,
    size: f32,
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

impl FileManagerApp {
    fn new() -> Self {
        let current_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            path_input: current_path.to_string_lossy().to_string(),
            current_path,
            selected_file: None,
            error_message: None,
            columns: Columns {
                name: 0.6,
                date: 0.2,
                size: 0.2,
            },
            left_padding: 8.0,
            show_hidden: false,
            cached_files: None,
        }
    }

    fn update_path(&mut self, new_path: PathBuf) {
        if new_path.exists() && new_path.is_dir() {
            self.current_path = new_path;
            self.selected_file = None;
            self.error_message = None;
            self.path_input = self.current_path.to_string_lossy().to_string();
            self.cached_files = None;
        } else {
            self.error_message = Some("Invalid directory path".to_string());
        }
    }

    fn get_files(&mut self) -> Option<&Vec<FileEntry>> {
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
                    let display_name = path.file_name().unwrap().to_string_lossy().to_string();
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
            None
        }
    }

    fn render_path_input(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Current directory:");

            let response = ui.add(
                egui::TextEdit::singleline(&mut self.path_input)
                    .desired_width(f32::INFINITY)
                    .id(egui::Id::new("path_input")),
            );

            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.update_path(PathBuf::from(&self.path_input));
            }

            if ui.button("🔄").on_hover_text("Refresh").clicked() {
                self.cached_files = None;
                self.path_input = self.current_path.to_string_lossy().to_string();
                self.error_message = None;
            }
        });
    }

    fn render_navigation_buttons(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui
                .button("⬆ Up")
                .on_hover_text("Go to parent directory")
                .clicked()
            {
                if let Some(parent) = self.current_path.parent() {
                    self.update_path(parent.to_path_buf());
                }
            }

            if ui
                .button("🏠 Home")
                .on_hover_text("Go to home directory")
                .clicked()
            {
                if let Some(home) = dirs::home_dir() {
                    self.update_path(home);
                }
            }

            let response = ui
                .checkbox(&mut self.show_hidden, "Show hidden")
                .on_hover_text("Toggle hidden files visibility");

            if response.changed() {
                ui.ctx().request_repaint();
            }
        });
    }

    fn render_table_headers(&mut self, ui: &mut egui::Ui, available_width: f32) {
        ui.horizontal(|ui| {
            ui.add_space(ui.available_width() * 0.05);

            let name_response = ui.label(egui::RichText::new("Name").heading());
            if name_response.dragged() {
                self.left_padding += name_response.drag_delta().x;
                self.left_padding = self.left_padding.clamp(0.0, 32.0);
            }

            let date_response = ui.label(egui::RichText::new("Modified").heading());
            if date_response.dragged() {
                self.columns.date += date_response.drag_delta().x / available_width;
                self.columns.date = self.columns.date.clamp(0.1, 0.8);
            }

            let size_response = ui.label(egui::RichText::new("Size").heading());
            if size_response.dragged() {
                self.columns.size += size_response.drag_delta().x / available_width;
                self.columns.size = self.columns.size.clamp(0.1, 0.8);
            }

            self.columns.name = 1.0 - (self.columns.date + self.columns.size);
        });

        ui.separator();
    }

    fn handle_file_click(&mut self, path: PathBuf) {
        if path.is_dir() {
            self.selected_file = Some(path);
        } else {
            if self.selected_file.as_ref() == Some(&path) {
                self.selected_file = None;
            } else {
                self.selected_file = Some(path);
            }
        }
    }

    fn handle_file_double_click(&mut self, path: PathBuf) {
        if path.is_dir() {
            self.update_path(path);
        }
    }

    fn render_file_row(
        &mut self,
        ui: &mut egui::Ui,
        file: &FileEntry,
        available_width: f32,
    ) -> egui::Response {
        let is_selected = self.selected_file.as_ref() == Some(&file.path);

        if !self.show_hidden && file.is_hidden {
            return ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover());
        }

        let (icon, text_color) = if file.is_dir {
            ("\u{1F5C0}", egui::Color32::from_rgb(100, 150, 255))
        } else {
            ("\u{1F4C4}", egui::Color32::from_rgb(200, 200, 200))
        };
        let name_format =
            egui::RichText::new(format!("{} {}", icon, &file.display_name)).color(text_color);

        if is_selected {
            let rect = ui.available_rect_before_wrap();
            ui.painter().rect_filled(
                rect,
                2.0,
                egui::Color32::from_rgba_unmultiplied(50, 50, 50, 50),
            );
        }

        let row_response = ui.horizontal(|ui| {
            ui.set_width(available_width);

            self.render_column(
                ui,
                available_width * self.columns.name,
                self.left_padding,
                |ui| ui.label(name_format),
                egui::Align::LEFT,
            );

            self.render_column(
                ui,
                available_width * self.columns.date,
                0.0,
                |ui| ui.label(&file.modified),
                egui::Align::Center,
            );

            self.render_column(
                ui,
                available_width * self.columns.size,
                0.0,
                |ui| ui.label(&file.size),
                egui::Align::RIGHT,
            );
        });

        let response = row_response.response.interact(egui::Sense::click());

        if response.clicked() {
            self.handle_file_click(file.path.clone());
        }

        if response.double_clicked() {
            self.handle_file_double_click(file.path.clone());
        }

        response.on_hover_text(file.path.to_string_lossy())
    }

    fn render_column(
        &self,
        ui: &mut egui::Ui,
        width: f32,
        padding: f32,
        content: impl FnOnce(&mut egui::Ui) -> egui::Response,
        alignment: egui::Align,
    ) {
        let layout = match alignment {
            egui::Align::LEFT => egui::Layout::left_to_right(egui::Align::LEFT),
            egui::Align::Center => {
                egui::Layout::centered_and_justified(egui::Direction::LeftToRight)
            }
            egui::Align::RIGHT => egui::Layout::right_to_left(egui::Align::RIGHT),
        };

        ui.allocate_ui_with_layout(egui::Vec2::new(width, 0.0), layout, |ui| {
            ui.add_space(padding);
            content(ui);
        });
    }
    fn render_file_list(&mut self, ui: &mut egui::Ui) {
        let available_width = ui.available_width();

        let files: Vec<FileEntry> = match self.get_files() {
            Some(files) => files.iter().cloned().collect(),
            None => {
                ui.colored_label(
                    egui::Color32::RED,
                    format!(
                        "Could not read directory contents: {}",
                        self.current_path.display()
                    ),
                );
                return;
            }
        };

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_right(true)
            .show(ui, |ui| {
                for (_, file) in files.iter().enumerate() {
                    ui.scope(|ui| {
                        let response = self.render_file_row(ui, file, available_width);
                        response.on_hover_text(file.path.to_string_lossy());
                    });
                }
            });
    }

    fn render_delete_button(&mut self, ui: &mut egui::Ui) {
        let delete_enabled = self.selected_file.is_some();
        let button = egui::Button::new(
            egui::RichText::new("🗑 Delete")
                .color(egui::Color32::WHITE)
                .size(16.0),
        )
        .fill(if delete_enabled {
            egui::Color32::from_rgb(200, 80, 80)
        } else {
            egui::Color32::from_rgb(100, 100, 100)
        })
        .min_size(egui::Vec2::new(ui.available_width(), 30.0));

        let response = ui.add_enabled(delete_enabled, button);

        if response.clicked() {
            self.delete_selected_file();
        }

        if delete_enabled {
            response.on_hover_text(format!(
                "Delete '{}'",
                self.selected_file
                    .as_ref()
                    .unwrap()
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
            ));
        } else {
            response.on_disabled_hover_text("Select a file or folder to delete");
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

    fn render_control_panel(&mut self, ui: &mut egui::Ui) {
        self.render_path_input(ui);
        self.render_navigation_buttons(ui);
        ui.separator();

        if let Some(err) = &self.error_message {
            ui.colored_label(egui::Color32::RED, err);
        }
    }
}

impl eframe::App for FileManagerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |_ui| {
            egui::TopBottomPanel::top("control_panel").show(ctx, |ui| {
                self.render_control_panel(ui);
                self.render_table_headers(ui, ui.available_width());
            });

            egui::CentralPanel::default().show(ctx, |ui| {
                let delete_button_height = 30.0;
                let vertical_margin = 8.0;
                let available_height =
                    ui.available_height() - delete_button_height - vertical_margin;

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .max_height(available_height)
                    .show(ui, |ui| {
                        self.render_file_list(ui);
                    });

                ui.add_space(vertical_margin);
                self.render_delete_button(ui);
            });
        });

        ctx.input(|i| {
            if i.key_pressed(egui::Key::Backspace) {
                if let Some(parent) = self.current_path.parent() {
                    self.update_path(parent.to_path_buf());
                }
            }
        });
    }
}

fn main() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_min_inner_size([400.0, 300.0]),
        ..Default::default()
    };

    eframe::run_native(
        "File Manager",
        options,
        Box::new(|_cc| Box::new(FileManagerApp::new())),
    )
    .unwrap();
}
