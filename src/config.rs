use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Represents a single Git profile configuration.
///
/// Profiles define the identity and credentials to be used when a Git repository's
/// remote URL matches the specified `match_pattern`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Profile {
    /// The user name to be set in `user.name`.
    pub name: String,
    /// The email address to be set in `user.email`.
    pub email: String,
    /// The path to the SSH private key to be used for this profile.
    pub ssh_key_path: String,
    /// A pattern used to match the Git remote URL. This can be a domain
    /// (e.g., "github.com") or a path fragment (e.g., "my-org/my-project").
    pub match_pattern: String,
}

/// The root configuration structure containing multiple profiles.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    /// A list of configured Git profiles.
    pub profiles: Vec<Profile>,
}

impl Config {
    /// Loads the configuration from the filesystem.
    ///
    /// The configuration file path is determined by `get_config_path`.
    /// If the file does not exist, an empty configuration is returned.
    ///
    /// # Errors
    /// Returns an error if the file exists but cannot be read or parsed as valid TOML.
    pub fn load() -> Result<Self> {
        let config_path = Self::get_config_path()?;
        log::debug!("Attempting to load config from: {:?}", config_path);

        if !config_path.exists() {
            log::debug!("Config file not found, using empty default configuration");
            return Ok(Config::default());
        }
        let content = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file at {:?}", config_path))?;
        toml::from_str(&content).with_context(|| "Failed to parse config TOML")
    }

    /// Determines the path to the configuration file based on priority.
    pub fn get_config_path() -> Result<PathBuf> {
        // Priority 1: Environment Variable
        if let Ok(env_path) = std::env::var("GIT_CTX_CONFIG") {
            log::debug!("Using configuration path from GIT_CTX_CONFIG: {}", env_path);
            return Ok(PathBuf::from(env_path));
        }

        // Priority 2: Standard ~/.config/git-ctx/profiles.toml (Preferred for CLI tools)
        if let Some(home) = dirs::home_dir() {
            let xdg_path = home.join(".config").join("git-ctx").join("profiles.toml");
            if xdg_path.exists() {
                log::debug!("Found config at standard XDG path: {:?}", xdg_path);
                return Ok(xdg_path);
            }
        }

        // Priority 3: ProjectDirs (OS-specific standard fallback)
        if let Some(proj_dirs) = directories::ProjectDirs::from("", "", "git-ctx") {
            let config_dir = proj_dirs.config_dir();
            let default_path = config_dir.join("profiles.toml");
            log::debug!("Using ProjectDirs configuration path: {:?}", default_path);
            return Ok(default_path);
        }

        // Priority 4: Default ~/.config/git-ctx/profiles.toml (Fallback if not exists yet)
        let home = dirs::home_dir().context("Could not find home directory")?;
        Ok(home.join(".config").join("git-ctx").join("profiles.toml"))
    }
}
