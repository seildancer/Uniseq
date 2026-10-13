use std::{fs, path::Path};

use tauri::AppHandle;

use crate::{CommandResult, ErrorDto, app_storage_dir};

const API_KEY_FILE: &str = "openai-api-key.txt";

fn read_api_key(dir: &Path) -> CommandResult<Option<String>> {
    let path = dir.join(API_KEY_FILE);
    match fs::read_to_string(&path) {
        Ok(key) => Ok(Some(key.trim().to_owned())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(ErrorDto::io(&path, &error)),
    }
}

fn write_api_key(dir: &Path, api_key: &str) -> CommandResult<()> {
    fs::create_dir_all(dir).map_err(|error| ErrorDto::io(dir, &error))?;
    let path = dir.join(API_KEY_FILE);
    // Keep an empty file when removing a key so old WebView storage cannot restore it.
    fs::write(&path, api_key.trim()).map_err(|error| ErrorDto::io(&path, &error))
}

#[tauri::command]
pub(crate) fn get_ai_chat_api_key(app: AppHandle) -> CommandResult<Option<String>> {
    read_api_key(&app_storage_dir(&app)?)
}

#[tauri::command]
pub(crate) fn set_ai_chat_api_key(app: AppHandle, api_key: String) -> CommandResult<()> {
    write_api_key(&app_storage_dir(&app)?, &api_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_key_persists_on_disk_and_can_be_replaced_and_removed() {
        let dir = crate::unique_temp_dir("uniseq-ai-settings");
        assert_eq!(read_api_key(&dir).unwrap(), None);
        write_api_key(&dir, "  test-openai-key  ").unwrap();
        assert_eq!(
            read_api_key(&dir).unwrap().as_deref(),
            Some("test-openai-key")
        );
        write_api_key(&dir, "replacement-key").unwrap();
        assert_eq!(
            read_api_key(&dir).unwrap().as_deref(),
            Some("replacement-key")
        );
        write_api_key(&dir, "").unwrap();
        assert_eq!(read_api_key(&dir).unwrap().as_deref(), Some(""));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn storage_errors_are_returned() {
        let dir = crate::unique_temp_dir("uniseq-ai-settings-error");
        fs::create_dir_all(dir.join(API_KEY_FILE)).unwrap();
        assert!(read_api_key(&dir).is_err());
        assert!(write_api_key(&dir, "test-key").is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}
