use std::{env, path::PathBuf};

#[derive(Clone)]
pub struct NavigationState {
    pub current_path: PathBuf,
    pub path_input: String,
    pub history: Vec<ViewHistory>,
    pub history_index: usize,
    pub max_history: usize,
}

#[derive(Clone)]
pub struct ViewHistory {
    pub path: PathBuf,
    pub scroll: f32,
}

impl ViewHistory {
    pub fn new(path: PathBuf, scroll: f32) -> Self {
        Self { path, scroll }
    }
}
#[allow(dead_code)]
impl NavigationState {
    pub fn new() -> Self {
        let current_path = env::current_dir()
            .unwrap_or_else(|_| dirs::home_dir().unwrap_or_else(|| PathBuf::from("/")));
        let path_input = current_path.to_string_lossy().to_string();

        Self {
            current_path: current_path.clone(),
            path_input,
            history: vec![ViewHistory::new(current_path, 0.0)],
            history_index: 0,
            max_history: 50,
        }
    }

    pub fn navigate_to(&mut self, path: PathBuf) {
        self.add_to_history(path.clone(), self.get_remembered_scroll(&path));
        self.current_path = path.clone();
        self.path_input = path.to_string_lossy().to_string();
    }

    /// Update the scroll position for the current path
    pub fn update_current_scroll(&mut self, scroll: f32) {
        if let Some(current_entry) = self.history.get_mut(self.history_index) {
            current_entry.scroll = scroll;
        }
    }

    fn get_remembered_scroll(&self, path: &PathBuf) -> f32 {
        self.history
            .iter()
            .rev() // Search from most recent
            .find(|entry| entry.path == *path)
            .map(|entry| entry.scroll)
            .unwrap_or(0.0)
    }

    /// Add a new entry to history
    fn add_to_history(&mut self, path: PathBuf, scroll: f32) {
        // If we're not at the end of history, truncate future entries
        if self.history_index + 1 < self.history.len() {
            self.history.truncate(self.history_index + 1);
        }

        // Don't add duplicate consecutive entries
        if let Some(last_entry) = self.history.last() {
            if last_entry.path == path {
                return;
            }
        }

        self.history.push(ViewHistory::new(path, scroll));

        // Maintain max history size
        if self.history.len() > self.max_history {
            self.history.remove(0);
        } else {
            self.history_index = self.history.len() - 1;
        }
    }

    pub fn can_go_back(&self) -> bool {
        self.history_index > 0
    }

    pub fn can_go_forward(&self) -> bool {
        self.history_index + 1 < self.history.len()
    }

    pub fn go_back(&mut self) -> Option<ViewHistory> {
        if self.can_go_back() {
            self.history_index -= 1;
            let history = self.history[self.history_index].clone();
            self.current_path = history.path.clone();
            self.path_input = history.path.to_string_lossy().to_string();
            Some(history)
        } else {
            None
        }
    }

    pub fn go_forward(&mut self) -> Option<ViewHistory> {
        if self.can_go_forward() {
            self.history_index += 1;
            let history = self.history[self.history_index].clone();
            self.current_path = history.path.clone();
            self.path_input = history.path.to_string_lossy().to_string();
            Some(history)
        } else {
            None
        }
    }

    /// Get the current scroll position
    pub fn get_current_scroll(&self) -> f32 {
        self.history
            .get(self.history_index)
            .map(|entry| entry.scroll)
            .unwrap_or(0.0)
    }

    /// Get all visited paths (useful for autocomplete or recent paths)
    pub fn get_visited_paths(&self) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = self.history
            .iter()
            .map(|entry| entry.path.clone())
            .collect();
        paths.dedup();
        paths
    }
}