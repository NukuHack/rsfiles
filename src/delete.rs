use std::{fs, path::PathBuf};
use std::process::Command;
use std::os::windows::process::CommandExt;
use winapi::um::winbase::CREATE_NO_WINDOW;
use crate::file_manager::{Message, FileManager};

impl FileManager {
	// Replace your delete_file method with this:
	pub fn delete_file(&mut self, path: PathBuf) -> iced::Command<Message> {
		self.ui_state.popup = None;
		self.ui_state.error_message = None;
		
		let result = if path.is_dir() {
			self.delete_with_elevation(&path, true)
		} else {
			self.delete_with_elevation(&path, false)
		};
		
		match result {
			Ok(_) => {
				self.ui_state.selected_file = None;
				self.refresh_directory()
			}
			Err(e) => {
				let error = format!("Error deleting {}: {}", if path.is_dir() { "folder" } else { "file" }, e);
				self.ui_state.set_error(error.clone());
				println!("{:?}", error);
				iced::Command::none()
			}
		}
	}

	// Add this new method to handle elevation
	pub fn delete_with_elevation(&self, path: &PathBuf, is_dir: bool) -> Result<(), String> {
		// First try normal deletion
		let normal_result = if is_dir {
			fs::remove_dir_all(path)
		} else {
			fs::remove_file(path)
		};
		
		if normal_result.is_ok() {
			return Ok(());
		}

		// If normal deletion fails, try with elevation using PowerShell
		let path_str = path.to_string_lossy().to_string();
		
		// Use PowerShell's Remove-Item with proper path handling
		let ps_script = if is_dir {
			format!("Remove-Item -LiteralPath '{}' -Recurse -Force -ErrorAction Stop", path_str.replace("'", "''"))
		} else {
			format!("Remove-Item -LiteralPath '{}' -Force -ErrorAction Stop", path_str.replace("'", "''"))
		};

		// Run PowerShell with elevation
		let output = Command::new("powershell")
			.args(&[
				"-Command", 
				&format!("Start-Process powershell -ArgumentList '-Command', '{}' -Verb RunAs -WindowStyle Hidden -Wait", ps_script.replace("'", "''"))
			])
			.creation_flags(CREATE_NO_WINDOW)
			.output();

		match output {
			Ok(result) => {
				if result.status.success() && !path.exists() {
					Ok(())
				} else {
					// If PowerShell elevation fails, try alternative method
					self.force_delete_alternative(path, is_dir)
				}
			}
			Err(_) => {
				// If PowerShell fails, try alternative method
				self.force_delete_alternative(path, is_dir)
			}
		}
	}
	
	// Alternative force delete method with better command construction
	pub fn force_delete_alternative(&self, path: &PathBuf, is_dir: bool) -> Result<(), String> {
		let path_str = path.to_string_lossy().to_string();
		
		// Method 1: Try elevated cmd commands with proper escaping
		let result = self.try_cmd_delete(&path_str, is_dir);
		if result.is_ok() && !path.exists() {
			return Ok(());
		}

		// Method 2: Try PowerShell direct execution with elevation
		let result = self.try_powershell_direct(&path_str, is_dir);
		if result.is_ok() && !path.exists() {
			return Ok(());
		}

		// Final check
		if path.exists() {
			Err("File/folder still exists after all deletion attempts".to_string())
		} else {
			Ok(())
		}
	}

	fn try_cmd_delete(&self, path_str: &str, is_dir: bool) -> Result<(), String> {
		// Use cmd with proper elevation request
		let script_content = if is_dir {
			format!(
				"@echo off\ntakeown /f \"{}\" /r /d y >nul 2>&1\nicacls \"{}\" /grant administrators:F /t >nul 2>&1\nrmdir /s /q \"{}\"",
				path_str, path_str, path_str
			)
		} else {
			format!(
				"@echo off\ntakeown /f \"{}\" >nul 2>&1\nicacls \"{}\" /grant administrators:F >nul 2>&1\ndel /f /q \"{}\"",
				path_str, path_str, path_str
			)
		};

		// Create a temporary batch file
		let temp_dir = std::env::temp_dir();
		let batch_file = temp_dir.join("delete_temp.bat");
		
		if let Err(e) = fs::write(&batch_file, script_content) {
			return Err(format!("Failed to create batch file: {}", e));
		}

		let batch_path = batch_file.to_string_lossy().to_string();
		
		// Execute with elevation
		let output = Command::new("powershell")
			.args(&[
				"-Command", 
				&format!("Start-Process cmd -ArgumentList '/c', '\"{}\"' -Verb RunAs -WindowStyle Hidden -Wait", batch_path)
			])
			.creation_flags(CREATE_NO_WINDOW)
			.output();

		// Clean up batch file
		let _ = fs::remove_file(&batch_file);

		match output {
			Ok(result) => {
				if result.status.success() {
					Ok(())
				} else {
					Err("Batch command failed".to_string())
				}
			}
			Err(e) => Err(format!("Failed to execute batch command: {}", e))
		}
	}

	fn try_powershell_direct(&self, path_str: &str, is_dir: bool) -> Result<(), String> {
		// Try direct PowerShell execution as administrator
		let ps_command = if is_dir {
			format!("Remove-Item -Path '{}' -Recurse -Force", path_str.replace("'", "''"))
		} else {
			format!("Remove-Item -Path '{}' -Force", path_str.replace("'", "''"))
		};

		let output = Command::new("powershell")
			.args(&[
				"-Command",
				&format!("Start-Process powershell -ArgumentList '-ExecutionPolicy', 'Bypass', '-Command', '{}' -Verb RunAs -WindowStyle Hidden -Wait", ps_command.replace("'", "''"))
			])
			.creation_flags(CREATE_NO_WINDOW)
			.output();

		match output {
			Ok(result) => {
				if result.status.success() {
					Ok(())
				} else {
					Err("Direct PowerShell command failed".to_string())
				}
			}
			Err(e) => Err(format!("Failed to execute PowerShell command: {}", e))
		}
	}
}
