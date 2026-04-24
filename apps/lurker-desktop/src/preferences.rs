use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const DEFAULT_UI_SCALE: f32 = 1.0;
const MIN_UI_SCALE: f32 = 0.8;
const MAX_UI_SCALE: f32 = 2.0;
const PREFERENCES_FILE: &str = "desktop-ui.json";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeMode {
    pub fn as_ui_value(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub fn parse_ui_value(value: &str) -> Option<Self> {
        match value {
            "system" => Some(Self::System),
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TaskMode {
    #[default]
    Create,
    Mount,
    Unmount,
}

impl TaskMode {
    pub fn as_ui_value(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Mount => "mount",
            Self::Unmount => "unmount",
        }
    }

    pub fn parse_ui_value(value: &str) -> Option<Self> {
        match value {
            "create" => Some(Self::Create),
            "mount" => Some(Self::Mount),
            "unmount" => Some(Self::Unmount),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UiPreferences {
    #[serde(default = "default_ui_scale")]
    pub ui_scale: f32,
    #[serde(default)]
    pub theme_mode: ThemeMode,
    #[serde(default)]
    pub last_task: TaskMode,
}

impl Default for UiPreferences {
    fn default() -> Self {
        Self {
            ui_scale: DEFAULT_UI_SCALE,
            theme_mode: ThemeMode::System,
            last_task: TaskMode::Create,
        }
    }
}

impl UiPreferences {
    pub fn normalized(mut self) -> Self {
        self.ui_scale = self.ui_scale.clamp(MIN_UI_SCALE, MAX_UI_SCALE);
        self
    }
}

pub fn load_preferences() -> UiPreferences {
    let Some(path) = preferences_path() else {
        return UiPreferences::default();
    };

    read_preferences(&path).unwrap_or_default().normalized()
}

pub fn save_preferences(preferences: &UiPreferences) -> io::Result<()> {
    let Some(path) = preferences_path() else {
        return Ok(());
    };

    write_preferences(&path, preferences)
}

fn default_ui_scale() -> f32 {
    DEFAULT_UI_SCALE
}

fn preferences_path() -> Option<PathBuf> {
    resolve_preferences_path(env::var_os("XDG_CONFIG_HOME"), env::var_os("HOME"))
}

fn resolve_preferences_path(config_home: Option<std::ffi::OsString>, home: Option<std::ffi::OsString>) -> Option<PathBuf> {
    if let Some(config_home) = config_home {
        return Some(PathBuf::from(config_home).join("lurker").join(PREFERENCES_FILE));
    }

    home.map(|home| PathBuf::from(home).join(".config").join("lurker").join(PREFERENCES_FILE))
}

fn read_preferences(path: &Path) -> io::Result<UiPreferences> {
    let contents = fs::read_to_string(path)?;
    let preferences = serde_json::from_str::<UiPreferences>(&contents)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    Ok(preferences.normalized())
}

fn write_preferences(path: &Path, preferences: &UiPreferences) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let payload = serde_json::to_vec_pretty(&preferences.clone().normalized())
        .map_err(io::Error::other)?;
    let temporary_path = path.with_extension("json.tmp");
    fs::write(&temporary_path, payload)?;
    fs::rename(temporary_path, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{read_preferences, resolve_preferences_path, write_preferences, TaskMode, ThemeMode, UiPreferences};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_path() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("lurker-desktop-test-{stamp}"))
    }

    #[test]
    fn resolve_preferences_path_prefers_xdg_home() {
        let path = resolve_preferences_path(Some("/tmp/xdg-test".into()), Some("/tmp/home-test".into()))
            .unwrap();

        assert_eq!(path, PathBuf::from("/tmp/xdg-test/lurker/desktop-ui.json"));
    }

    #[test]
    fn resolve_preferences_path_falls_back_to_home_config() {
        let path = resolve_preferences_path(None, Some("/tmp/home-test".into())).unwrap();

        assert_eq!(path, PathBuf::from("/tmp/home-test/.config/lurker/desktop-ui.json"));
    }

    #[test]
    fn preferences_round_trip_and_normalize_scale() {
        let root = unique_temp_path();
        let file_path = root.join("desktop-ui.json");

        let preferences = UiPreferences {
            ui_scale: 2.8,
            theme_mode: ThemeMode::Dark,
            last_task: TaskMode::Unmount,
        };

        write_preferences(&file_path, &preferences).unwrap();
        let restored = read_preferences(&file_path).unwrap();

        assert_eq!(restored.ui_scale, 2.0);
        assert_eq!(restored.theme_mode, ThemeMode::Dark);
        assert_eq!(restored.last_task, TaskMode::Unmount);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn theme_and_task_modes_match_ui_values() {
        assert_eq!(ThemeMode::parse_ui_value(ThemeMode::Light.as_ui_value()), Some(ThemeMode::Light));
        assert_eq!(TaskMode::parse_ui_value(TaskMode::Mount.as_ui_value()), Some(TaskMode::Mount));
        assert_eq!(ThemeMode::parse_ui_value("unknown"), None);
        assert_eq!(TaskMode::parse_ui_value("other"), None);
    }
}
