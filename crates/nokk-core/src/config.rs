use std::fmt;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::assets::AnimationId;
use crate::pet::PetMood;

#[derive(Debug)]
pub enum ConfigError {
    MissingHome,
    Io(std::io::Error),
    Ron(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingHome => write!(f, "could not locate a config directory"),
            Self::Io(err) => write!(f, "config io error: {err}"),
            Self::Ron(err) => write!(f, "config parse error: {err}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<std::io::Error> for ConfigError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    pub monitor: Option<String>,
    pub position: Option<(i32, i32)>,
    pub last_pose: AnimationId,
    pub mood: PetMood,
    pub scale: u32,
    pub paused: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            monitor: None,
            position: None,
            last_pose: AnimationId::Idle,
            mood: PetMood::Calm,
            scale: 1,
            paused: false,
        }
    }
}

impl AppConfig {
    pub fn load_or_default() -> Result<Self, ConfigError> {
        let path = config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }

        let input = fs::read_to_string(path)?;
        ron::from_str(&input).map_err(|err| ConfigError::Ron(err.to_string()))
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        let path = config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let serialized =
            ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
                .map_err(|err| ConfigError::Ron(err.to_string()))?;
        fs::write(path, serialized)?;
        Ok(())
    }
}

pub fn config_path() -> Result<PathBuf, ConfigError> {
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var_os("APPDATA").ok_or(ConfigError::MissingHome)?;
        return Ok(PathBuf::from(appdata).join("Nokk").join("config.ron"));
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            return Ok(PathBuf::from(xdg).join("nokk").join("config.ron"));
        }
        let home = std::env::var_os("HOME").ok_or(ConfigError::MissingHome)?;
        Ok(PathBuf::from(home)
            .join(".config")
            .join("nokk")
            .join("config.ron"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_v1_values() {
        let config = AppConfig::default();
        assert_eq!(config.scale, 1);
        assert_eq!(config.last_pose, AnimationId::Idle);
        assert_eq!(config.mood, PetMood::Calm);
        assert!(!config.paused);
    }

    #[test]
    fn config_roundtrips_through_ron() {
        let config = AppConfig {
            monitor: Some("DP-2".into()),
            position: Some((144, 220)),
            last_pose: AnimationId::Dance,
            mood: PetMood::Happy,
            scale: 3,
            paused: true,
        };
        let text = ron::ser::to_string(&config).unwrap();
        let decoded: AppConfig = ron::from_str(&text).unwrap();
        assert_eq!(decoded, config);
    }
}
