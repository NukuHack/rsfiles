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

use super::*;

pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    const UNIT_SIZE: f64 = 1024.0;

    if bytes == 0 {
        return "0 B".to_string();
    }

    let i = (bytes as f64).log(UNIT_SIZE).floor() as usize;
    let size = bytes as f64 / UNIT_SIZE.powi(i as i32);

    if i < UNITS.len() {
        format!("{:.1} {}", size, UNITS[i])
    } else {
        let tb_size = bytes as f64 / UNIT_SIZE.powi(4);
        format!("{:.1} TB", tb_size)
    }
}

pub fn format_time_ago(time: SystemTime) -> String {
    let now = SystemTime::now();
    let duration = now.duration_since(time).unwrap_or_default();
    let secs = duration.as_secs();

    let years = secs / 31_536_000;
    let days = (secs % 31_536_000) / 86400;
    let hours = (secs % 86400) / 3600;
    let minutes = (secs % 3600) / 60;

    if years > 0 {
        format!("{}y {}d ago", years, days)
    } else if days > 0 {
        format!("{}d {}h ago", days, hours)
    } else if hours > 0 {
        format!("{}h {}m ago", hours, minutes)
    } else if minutes > 0 {
        format!("{}m ago", minutes)
    } else {
        "Just now".to_string()
    }
}
pub fn format_time(time: SystemTime) -> String {
    match time.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(duration) => {
            let secs = duration.as_secs();
            let minutes = secs / 60;
            let hours = minutes / 60;
            let days = hours / 24;
            
            // This is a simplified calculation - for precise date/time you'd need to handle
            // leap years, months with different days, etc. (which is why chrono is better)
            let year = 1970 + (days / 365) as i32;
            let month = ((days % 365) / 30 + 1) as u32;
            let day = (days % 30 + 1) as u32;
            let hour = (hours % 24) as u32;
            let minute = (minutes % 60) as u32;
            
            format!("{:04}.{:02}.{:02} {:02}:{:02}", year, month, day, hour, minute)
        }
        Err(_) => "Invalid time".to_string(),
    }
}


// Synchronous file loading for better performance on small directories
pub fn load_files_sync(path: PathBuf, show_hidden: bool) -> Command<super::Message> {
    Command::perform(
        async move {
            load_directory_contents(&path, show_hidden)
        },
        Message::FilesLoaded,
    )
}

// Optimized directory loading
pub fn load_directory_contents(path: &PathBuf, show_hidden: bool) -> Result<Vec<super::FileEntry>, String> {
    let mut files = Vec::new();
    
    let entries = fs::read_dir(path)
        .map_err(|e| format!("Error reading directory: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Error reading directory entry: {}", e))?;
        let path = entry.path();
        
        let display_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        
        let is_hidden = display_name.starts_with('.');
        
        // Skip hidden files if not showing them
        if !show_hidden && is_hidden {
            continue;
        }

        let metadata = entry.metadata()
            .map_err(|e| format!("Error reading metadata for {}: {}", display_name, e))?;

        let modified_str = metadata
            .modified()
            .map(helper::format_time)
            .unwrap_or_else(|_| "Unknown".to_string());

        let size_str = if metadata.is_dir() {
            String::new()
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

    // Sort files: directories first, then by name
    files.sort_by(|a, b| {
        match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()),
        }
    });

    Ok(files)
}