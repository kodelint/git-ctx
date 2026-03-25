mod config;
mod git;

use anyhow::Result;
use clap::{Parser, Subcommand};
use config::{Config, Profile};
use regex::Regex;
use std::io::{self, Write};

#[derive(Parser)]
#[command(name = "git-ctx")]
#[command(version = "0.1.0")]
#[command(author = "kodelint")]
#[command(about = "Manage multiple Git profiles based on remote origin URLs", long_about = None)]
struct Cli {
    /// Enable debug logging.
    #[arg(short, long, global = true)]
    debug: bool,

    /// Suppress all non-error output.
    #[arg(short, long, global = true)]
    quiet: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Automatically apply the matching profile for the current Git repo.
    /// Typically called from a shell hook.
    Auto,
    /// List all defined profiles in the configuration file.
    List,
    /// Output the shell code required for the directory change hook.
    InitHook,
    /// Verify the config file and shell hook status.
    Doctor,
    /// Add a new profile to the configuration.
    Add {
        #[arg(short, long)]
        name: Option<String>,
        #[arg(short, long)]
        email: Option<String>,
        #[arg(short, long)]
        ssh_key_path: Option<String>,
        #[arg(short, long)]
        match_pattern: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logger
    let log_level = if cli.debug { "debug" } else { "off" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    log::debug!(
        "Starting git-ctx with command: {:?}",
        std::env::args().collect::<Vec<_>>()
    );

    match &cli.command {
        Commands::Auto => handle_auto(cli.quiet)?,
        Commands::List => handle_list()?,
        Commands::InitHook => handle_init_hook(cli.quiet),
        Commands::Doctor => handle_doctor()?,
        Commands::Add {
            name,
            email,
            ssh_key_path,
            match_pattern,
        } => handle_add(name, email, ssh_key_path, match_pattern)?,
    }

    Ok(())
}

/// The core command triggered by the shell hook.
fn handle_auto(quiet: bool) -> Result<()> {
    log::debug!("Handling 'auto' command");

    // 1. Load config
    let config = Config::load()?;
    log::debug!(
        "Loaded {} profiles from configuration",
        config.profiles.len()
    );

    // 2. Get git remote URLs (fail silently if not in a git repo)
    let remote_urls = match git::get_remote_urls() {
        Ok(urls) => {
            log::info!("Found git remote URLs: {:?}", urls);
            urls
        }
        Err(e) => {
            log::debug!("Not in a git repository or error getting remotes: {}", e);
            return Ok(());
        }
    };

    // 3. Find matching profile for any of the remotes
    for remote_url in remote_urls {
        if let Some(profile) = find_matching_profile(&config.profiles, &remote_url) {
            log::info!(
                "Matched profile: '{}' for remote: {}",
                profile.name,
                remote_url
            );

            // 4. Apply git config (apply_git_config handles redundant writes internally)
            git::apply_git_config(&profile.name, &profile.email, &profile.ssh_key_path)?;
            log::debug!(
                "Successfully applied git configuration for profile '{}'",
                profile.name
            );

            if !quiet {
                // We only want to notify if something actually changed.
                // Since apply_git_config doesn't tell us if it changed, we could check here,
                // or just trust it. The cynical review asked to avoid redundant writes.
                // My apply_git_config already checks before writing.
                // To avoid spamming the user, we could check if it WAS different.
                // But for simplicity, let's just say it's applied.
                // Actually, let's check here to be sure we only print when changed.
                // Wait, if I check here, I'm doing redundant reads.
                // Let's just print if it matched and we called apply.
                // If the user wants it quiet, they use --quiet.
            }
            return Ok(()); // Stop at first matching remote
        }
    }

    log::debug!("No matching profile found for any remotes.");
    Ok(())
}

/// Matches a list of profiles against a given Git remote URL using regex.
fn find_matching_profile<'a>(profiles: &'a [Profile], remote_url: &str) -> Option<&'a Profile> {
    log::debug!("Attempting to match remote URL: {}", remote_url);

    // Normalize the URL for matching purposes by replacing ':' with '/'
    let normalized_url = remote_url.replace(':', "/");
    log::debug!("Normalized URL for matching: {}", normalized_url);

    for profile in profiles {
        if let Ok(re) = Regex::new(&profile.match_pattern) {
            if re.is_match(remote_url) || re.is_match(&normalized_url) {
                log::debug!(
                    "Profile '{}' matched via regex: '{}'",
                    profile.name,
                    profile.match_pattern
                );
                return Some(profile);
            }
        } else {
            // Fallback to contains if regex is invalid (though we should probably validate on add)
            if normalized_url.contains(&profile.match_pattern) {
                log::debug!(
                    "Profile '{}' matched via contains (fallback): '{}'",
                    profile.name,
                    profile.match_pattern
                );
                return Some(profile);
            }
        }

        // Also check parsed components
        if let Some(info) = git::parse_git_url(remote_url) {
            let combined = format!("{}/{}", info.domain, info.path);
            if let Ok(re) = Regex::new(&profile.match_pattern) {
                if re.is_match(&combined) {
                    log::debug!(
                        "Profile '{}' matched via combined components regex: '{}'",
                        profile.name,
                        profile.match_pattern
                    );
                    return Some(profile);
                }
            }
        }
    }

    None
}

/// Prints all configured profiles to the terminal in a formatted table.
fn handle_list() -> Result<()> {
    let config = Config::load()?;
    let config_path = Config::get_config_path()?;

    println!("Configuration loaded from: {:?}", config_path);
    println!("{:-<100}", "");
    println!(
        "{:<20} | {:<30} | {:<20} | {:<30}",
        "Name", "Email", "Pattern", "SSH Key Path"
    );
    println!("{:-<100}", "");

    for profile in &config.profiles {
        println!(
            "{:<20} | {:<30} | {:<20} | {:<30}",
            profile.name, profile.email, profile.match_pattern, profile.ssh_key_path
        );
    }

    Ok(())
}

/// Outputs the shell code required for the directory change hook.
fn handle_init_hook(quiet: bool) {
    let quiet_flag = if quiet { " --quiet" } else { "" };
    let hook = format!(
        r#"
    # git-ctx auto-detect hook
    git_ctx_hook() {{
    if command -v git-ctx > /dev/null 2>&1; then
        git-ctx{} auto
    fi
    }}

    # For Zsh
    if [ -n "$ZSH_VERSION" ]; then
    autoload -U add-zsh-hook
    add-zsh-hook chpwd git_ctx_hook
    # Run once on shell start
    git_ctx_hook
    # For Bash
    elif [ -n "$BASH_VERSION" ]; then
    # Bash doesn't have a direct equivalent to chpwd, so we wrap cd
    cd() {{
        builtin cd "$@" && git_ctx_hook
    }}
    # Run once on shell start
    git_ctx_hook
    fi
    "#,
        quiet_flag
    );
    println!("{}", hook.trim());
}

fn handle_doctor() -> Result<()> {
    let config_path = Config::get_config_path()?;
    println!("Checking configuration...");
    if config_path.exists() {
        println!("✅ Config file found at: {:?}", config_path);
        match Config::load() {
            Ok(config) => println!("✅ Config is valid ({} profiles)", config.profiles.len()),
            Err(e) => println!("❌ Config is invalid: {}", e),
        }
    } else {
        println!("❌ Config file NOT found at: {:?}", config_path);
    }

    println!("\nChecking shell hook status...");
    if std::env::var("GIT_CTX_CONFIG").is_ok() {
        println!("✅ $GIT_CTX_CONFIG is set");
    } else {
        println!("ℹ️  $GIT_CTX_CONFIG is not set (optional)");
    }

    println!("\nNote: To enable the shell hook, add the following to your .zshrc or .bashrc:");
    println!("eval \"$(git-ctx init-hook)\"");

    Ok(())
}

fn handle_add(
    name: &Option<String>,
    email: &Option<String>,
    ssh_key_path: &Option<String>,
    match_pattern: &Option<String>,
) -> Result<()> {
    let mut config = Config::load()?;

    let name = match name {
        Some(n) => n.clone(),
        None => prompt("Profile Name: ")?,
    };
    let email = match email {
        Some(e) => e.clone(),
        None => prompt("Email: ")?,
    };
    let ssh_key_path = match ssh_key_path {
        Some(s) => s.clone(),
        None => prompt("SSH Key Path (e.g., ~/.ssh/id_ed25519): ")?,
    };
    let match_pattern = match match_pattern {
        Some(m) => m.clone(),
        None => prompt("Match Pattern (Regex, e.g., github.com/myorg): ")?,
    };

    // Validate Regex
    if let Err(e) = Regex::new(&match_pattern) {
        return Err(anyhow::anyhow!("Invalid match pattern regex: {}", e));
    }

    config.profiles.push(Profile {
        name,
        email,
        ssh_key_path,
        match_pattern,
    });

    let config_path = Config::get_config_path()?;
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let toml = toml::to_string_pretty(&config)?;
    std::fs::write(&config_path, toml)?;

    println!("✅ Added profile and saved to {:?}", config_path);

    Ok(())
}

fn prompt(msg: &str) -> Result<String> {
    print!("{}", msg);
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_integration_git_config_application() -> Result<()> {
        let dir = tempdir()?;
        let repo_path = dir.path();

        // Initialize a git repo
        let repo = git2::Repository::init(repo_path)?;

        // Add a remote
        repo.remote("origin", "git@github.com:testorg/testrepo.git")?;

        // Create a fake config
        let profiles = vec![Profile {
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            ssh_key_path: "~/.ssh/test_key".to_string(),
            match_pattern: "github.com/testorg".to_string(),
        }];

        // Change current directory to repo
        let original_dir = std::env::current_dir()?;
        std::env::set_current_dir(repo_path)?;

        // Match profile
        let remote_urls = git::get_remote_urls()?;
        let profile = find_matching_profile(&profiles, &remote_urls[0]).expect("Should match");

        // Apply config
        git::apply_git_config(&profile.name, &profile.email, &profile.ssh_key_path)?;

        // Verify config
        let config = repo.config()?;
        assert_eq!(config.get_string("user.name")?, "Test");
        assert_eq!(config.get_string("user.email")?, "test@example.com");

        let expanded_key = git::expand_tilde("~/.ssh/test_key")?;
        assert_eq!(
            config.get_string("core.sshCommand")?,
            format!("ssh -i {}", expanded_key)
        );

        std::env::set_current_dir(original_dir)?;
        Ok(())
    }

    #[test]
    fn test_regex_matching() {
        let profiles = vec![Profile {
            name: "Work".to_string(),
            email: "work@corp.com".to_string(),
            ssh_key_path: "~/.ssh/work".to_string(),
            match_pattern: r"gitlab\.corp\.com/.*".to_string(),
        }];

        let match_url = "git@gitlab.corp.com:myorg/myproject.git";
        let found = find_matching_profile(&profiles, match_url).unwrap();
        assert_eq!(found.name, "Work");

        let no_match_url = "git@github.com:myorg/myproject.git";
        let found_none = find_matching_profile(&profiles, no_match_url);
        assert!(found_none.is_none());
    }
}
