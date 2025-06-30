
use std::path::Path;
use std::io;
use std::os::windows::fs::MetadataExt;

// Add this for Windows shortcut resolution
#[cfg(windows)]
use winapi::{
    Interface,
    um::{
        objbase::CoInitialize,
        combaseapi::{CoCreateInstance, CoUninitialize},
        shobjidl_core::{IShellLinkW},
        objidl::IPersistFile
    },
    shared::{
        wtypesbase::CLSCTX_INPROC_SERVER,
        winerror::{S_OK, S_FALSE},
        guiddef::CLSID
    }
};

#[cfg(windows)]
const STGM_READ: u32 = 0x00000000;

// Define CLSID_ShellLink
#[cfg(windows)]
const CLSID_SHELLLINK: CLSID = CLSID {
    Data1: 0x00021401,
    Data2: 0x0000,
    Data3: 0x0000,
    Data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};

impl super::file_manager::FileManager {
    // Function to resolve Windows shortcut (.lnk) files
    #[cfg(windows)]
    pub fn resolve_shortcut(&self, lnk_path: &PathBuf) -> Option<PathBuf> {
        use std::ptr;
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        
        unsafe {
            // Initialize COM
            let hr = CoInitialize(ptr::null_mut());
            if hr != S_OK && hr != S_FALSE { // S_FALSE means already initialized
                return None;
            }
            
            let mut shell_link: *mut IShellLinkW = ptr::null_mut();
            let hr = CoCreateInstance(
                &CLSID_SHELLLINK,
                ptr::null_mut(),
                CLSCTX_INPROC_SERVER,
                &IShellLinkW::uuidof(),
                &mut shell_link as *mut *mut IShellLinkW as *mut *mut _,
            );
            
            if hr != S_OK || shell_link.is_null() {
                CoUninitialize();
                return None;
            }
            
            // Query for IPersistFile interface
            let mut persist_file: *mut IPersistFile = ptr::null_mut();
            let hr = (*shell_link).QueryInterface(
                &IPersistFile::uuidof(),
                &mut persist_file as *mut *mut IPersistFile as *mut *mut _,
            );
            
            if hr != S_OK || persist_file.is_null() {
                (*shell_link).Release();
                CoUninitialize();
                return None;
            }
            
            // Convert path to wide string
            let wide_path: Vec<u16> = OsStr::new(lnk_path)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            
            // Load the shortcut
            let hr = (*persist_file).Load(wide_path.as_ptr(), STGM_READ);
            if hr != S_OK {
                (*persist_file).Release();
                (*shell_link).Release();
                CoUninitialize();
                return None;
            }
            
            // Resolve the link (this is important for shortcuts to folders)
            let _ = (*shell_link).Resolve(ptr::null_mut(), 0);
            
            // Get the target path
            let mut target_path = [0u16; winapi::shared::minwindef::MAX_PATH as usize];
            let hr = (*shell_link).GetPath(target_path.as_mut_ptr(), target_path.len() as i32, ptr::null_mut(), STGM_READ);
            
            // Cleanup
            (*persist_file).Release();
            (*shell_link).Release();
            CoUninitialize();
            
            if hr == S_OK {
                // Convert wide string back to PathBuf
                let len = target_path.iter().position(|&x| x == 0).unwrap_or(0);
                if len > 0 {
                    let target_string = String::from_utf16_lossy(&target_path[..len]);
                    Some(PathBuf::from(target_string))
                } else {
                    None
                }
            } else {
                None
            }
        }
    }
    
    // For non-Windows platforms, return None
    #[cfg(not(windows))]
    pub fn resolve_shortcut(&self, _lnk_path: &PathBuf) -> Option<PathBuf> {
        None
    }
    
    // Helper function to check if a file is a shortcut
    pub fn is_shortcut(&self, path: &PathBuf) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("lnk"))
            .unwrap_or(false)
    }
}


pub struct Columns {
    name: f32,
    date: f32,
    size: f32,
}

impl Columns {
    pub fn new() -> Self {
        Self {
            name: 50.0,
            date: 25.0,
            size: 20.0,
        }
    }

    pub fn name(&self) -> f32 {
        self.name
    }
    pub fn date(&self) -> f32 {
        self.date
    }
    pub fn size(&self) -> f32 {
        self.size
    }
}

#[derive(Clone, Debug)]
pub struct FileEntry {
    path: PathBuf,
    display_name: String,
    is_dir: bool,
    modified: String,
    size: String,
    is_hidden: bool,
}
#[allow(dead_code)]
impl FileEntry {
    pub fn new(
        path: PathBuf,
        display_name: String,
        is_dir: bool,
        modified: String,
        size: String,
        is_hidden: bool,
    ) -> Self {
        Self{
            path,
            display_name,
            is_dir,
            modified,
            size,
            is_hidden,
        }
    }

    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }
    pub fn display_name(&self) -> String {
        self.display_name.clone()
    }
    pub fn is_dir(&self) -> bool {
        self.is_dir
    }
    pub fn modified(&self) -> String {
        self.modified.clone()
    }
    pub fn size(&self) -> String {
        self.size.clone()
    }
    pub fn is_hidden(&self) -> bool {
        self.is_hidden
    }
}


use crate::file_manager::Message;
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

#[allow(dead_code)]
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

pub fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<()> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}

// Synchronous file loading for better performance on small directories
pub fn load_files_sync(path: PathBuf, show_hidden: bool) -> Command<Message> {
    Command::perform(
        async move {
            load_directory_contents(&path, show_hidden)
        },
        Message::FilesLoaded,
    )
}


/// Loads directory contents with proper hidden file checking and sorting
// Optimized directory loading with concise hidden file handling
pub fn load_directory_contents(path: &PathBuf, show_hidden: bool) -> Result<Vec<FileEntry>, String> {
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
        
        // Early continue if file is hidden and we're not showing hidden files
        if !show_hidden && is_file_hidden(&entry)? {
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

        let is_hidden = is_file_hidden(&entry)?;  // We check again since we need it for FileEntry

        files.push(FileEntry::new(
            path,
            display_name,
            metadata.is_dir(),
            modified_str,
            size_str,
            is_hidden,
        ));
    }

    // Sort files: directories first, then by name
    files.sort_by(|a, b| {
        match (a.is_dir(), b.is_dir()) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.display_name().to_lowercase().cmp(&b.display_name().to_lowercase()),
        }
    });

    Ok(files)
}

/// Proper cross-platform hidden file check
fn is_file_hidden(entry: &fs::DirEntry) -> Result<bool, String> {
    #[cfg(unix)]
    {
        // On Unix, check if filename starts with a dot
        let name = entry.file_name();
        let is_hidden = name.to_string_lossy().starts_with('.');
        Ok(is_hidden)
    }
    
    #[cfg(windows)]
    {
        // On Windows, check the hidden file attribute
        let metadata = entry.metadata()
            .map_err(|e| format!("Error reading metadata: {}", e))?;
        Ok(metadata.file_attributes() & 0x2 != 0)
    }
    
    #[cfg(not(any(unix, windows)))]
    {
        // For other platforms, fall back to dot file check
        let name = entry.file_name();
        Ok(name.to_string_lossy().starts_with('.'))
    }
}