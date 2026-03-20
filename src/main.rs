mod config;
mod git;

use anyhow::Result;
use clap::{Parser, Subcommand};
use config::{Config, Profile};

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

    // 2. Get git remote URL (fail silently if not in a git repo)
    let remote_url = match git::get_remote_url() {
        Ok(url) => {
            log::info!("Found git remote URL: {}", url);
            url
        }
        Err(e) => {
            log::debug!("Not in a git repository or error getting remote: {}", e);
            return Ok(());
        }
    };

    // 3. Find matching profile
    if let Some(profile) = find_matching_profile(&config.profiles, &remote_url) {
        log::info!(
            "Matched profile: '{}' for remote: {}",
            profile.name,
            remote_url
        );

        // 4. Check if we need to apply the profile
        let current_email = git::get_local_config("user.email")?;
        let current_ssh = git::get_local_config("core.sshCommand")?;
        let expanded_ssh_key_path = git::expand_tilde(&profile.ssh_key_path)?;
        let expected_ssh = format!("ssh -i {} -F /dev/null", expanded_ssh_key_path);

        let needs_apply = current_email.as_deref() != Some(&profile.email)
            || current_ssh.as_deref() != Some(&expected_ssh);

        if needs_apply {
            // 5. Apply git config
            git::apply_git_config(&profile.name, &profile.email, &profile.ssh_key_path)?;
            log::info!(
                "Successfully applied git configuration for profile '{}'",
                profile.name
            );

            if !quiet {
                eprintln!(
                    "[git-ctx] Switched to profile '{}' ({})",
                    profile.name, profile.email
                );
            }
        } else {
            log::debug!(
                "Profile '{}' already applied, skipping update.",
                profile.name
            );
        }
    } else {
        log::debug!("No matching profile found for remote: {}", remote_url);
    }

    Ok(())
}

/// Matches a list of profiles against a given Git remote URL.
fn find_matching_profile<'a>(profiles: &'a [Profile], remote_url: &str) -> Option<&'a Profile> {
    log::debug!("Attempting to match remote URL: {}", remote_url);

    // Normalize the URL for matching purposes by replacing ':' with '/'
    // This allows patterns like "github.com/org" to match both:
    // SSH: git@github.com:org/repo.git
    // HTTPS: https://github.com/org/repo.git
    let normalized_url = remote_url.replace(':', "/");
    log::debug!("Normalized URL for matching: {}", normalized_url);

    // 1. Check against normalized URL
    if let Some(profile) = profiles
        .iter()
        .find(|p| normalized_url.contains(&p.match_pattern))
    {
        log::debug!(
            "Profile '{}' matched via normalized URL: '{}'",
            profile.name,
            profile.match_pattern
        );
        return Some(profile);
    }

    // 2. Fallback: Check against parsed components (for more complex URI structures)
    if let Some(info) = git::parse_git_url(remote_url) {
        let combined = format!("{}/{}", info.domain, info.path);
        log::debug!("Combined Domain/Path for matching: {}", combined);
        if let Some(profile) = profiles
            .iter()
            .find(|p| combined.contains(&p.match_pattern))
        {
            log::debug!(
                "Profile '{}' matched via combined components: '{}'",
                profile.name,
                profile.match_pattern
            );
            return Some(profile);
        }
    }

    log::debug!("No matching profile found for remote: {}", remote_url);
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
///
/// The user should pipe this output to their shell configuration file:
/// `eval "$(git-ctx init-hook)"`
fn handle_init_hook(quiet: bool) {
    let quiet_flag = if quiet { " --quiet" } else { "" };
    let hook = format!(
        r#"
# git-ctx auto-detect hook
git_ctx_hook() {
    if command -v git-ctx > /dev/null 2>&1; then
        git-ctx{} auto
    fi
}

# For Zsh
if [ -n "$ZSH_VERSION" ]; then
    autoload -U add-zsh-hook
    add-zsh-hook chpwd git_ctx_hook
    # Run once on shell start
    git_ctx_hook
# For Bash
elif [ -n "$BASH_VERSION" ]; then
    # Bash doesn't have a direct equivalent to chpwd, so we wrap cd
    cd() {
        builtin cd "$@" && git_ctx_hook
    }
    # Run once on shell start
    git_ctx_hook
fi
"#,
        quiet_flag
    );
    println!("{}", hook.trim());
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_matching_profile_domain() {
        let profiles = vec![
            Profile {
                name: "Work".to_string(),
                email: "work@corp.com".to_string(),
                ssh_key_path: "~/.ssh/work".to_string(),
                match_pattern: "corp.com".to_string(),
            },
            Profile {
                name: "Personal".to_string(),
                email: "me@home.com".to_string(),
                ssh_key_path: "~/.ssh/personal".to_string(),
                match_pattern: "github.com/personal".to_string(),
            },
        ];

        let match_url = "git@corp.com:project/repo.git";
        let found = find_matching_profile(&profiles, match_url).unwrap();
        assert_eq!(found.name, "Work");
    }

    #[test]
    fn test_find_matching_profile_full_url() {
        let profiles = vec![Profile {
            name: "Personal".to_string(),
            email: "me@home.com".to_string(),
            ssh_key_path: "~/.ssh/personal".to_string(),
            match_pattern: "github.com/personal-user".to_string(),
        }];

        // Should match HTTPS
        let https_url = "https://github.com/personal-user/my-project.git";
        let found_https = find_matching_profile(&profiles, https_url).unwrap();
        assert_eq!(found_https.name, "Personal");

        // Should also match SSH (even with ':' instead of '/')
        let ssh_url = "git@github.com:personal-user/my-project.git";
        let found_ssh = find_matching_profile(&profiles, ssh_url).unwrap();
        assert_eq!(found_ssh.name, "Personal");
    }

    #[test]
    fn test_find_matching_profile_no_match() {
        let profiles = vec![Profile {
            name: "Work".to_string(),
            email: "work@corp.com".to_string(),
            ssh_key_path: "~/.ssh/work".to_string(),
            match_pattern: "corp.com".to_string(),
        }];

        let match_url = "https://github.com/random/repo.git";
        let found = find_matching_profile(&profiles, match_url);
        assert!(found.is_none());
    }
}
