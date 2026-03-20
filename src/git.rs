use std::process::Command;
use anyhow::{Result, Context, anyhow};

/// Represents the extracted components of a Git remote URL.
///
/// This structure is used to categorize different parts of a remote URL
/// to facilitate flexible matching against user-defined profiles.
#[derive(Debug, PartialEq, Default)]
pub struct GitRemoteInfo {
    /// The domain of the Git host (e.g., "github.com", "gitlab.company.com").
    pub domain: String,
    /// The repository path, typically including organization and repo name (e.g., "org/repo").
    pub path: String,
}

/// Parses a Git remote URL into its constituent domain and path components.
///
/// This function supports both SSH and HTTPS formats:
/// - SSH: `git@github.com:Organization/repo.git` or `ssh://git@host:port/path/repo.git`
/// - HTTPS: `https://github.com/Organization/repo.git`
///
/// It gracefully handles URLs with or without the `.git` suffix and attempts to
/// normalize the output for consistent matching.
///
/// # Arguments
/// * `url` - A string slice representing the Git remote URL.
///
/// # Returns
/// * `Some(GitRemoteInfo)` if the URL was successfully parsed.
/// * `None` if the URL format is unrecognized.
pub fn parse_git_url(url: &str) -> Option<GitRemoteInfo> {
    if url.starts_with("git@") {
        // Standard SCP-like SSH format: git@domain:path/to/repo.git
        let parts: Vec<&str> = url.trim_start_matches("git@").splitn(2, ':').collect();
        if parts.len() == 2 {
            return Some(GitRemoteInfo {
                domain: parts[0].to_string(),
                path: parts[1].trim_end_matches(".git").to_string(),
            });
        }
    } else if url.starts_with("ssh://") {
        // Full SSH URI format: ssh://git@host:port/path/to/repo.git
        let clean_url = url.trim_start_matches("ssh://");
        let parts: Vec<&str> = clean_url.splitn(2, '/').collect();
        if parts.len() == 2 {
            // Further split domain/port if necessary
            let domain = parts[0].split(':').next().unwrap_or(parts[0]);
            let host_parts: Vec<&str> = domain.split('@').collect();
            let final_domain = host_parts.last().unwrap_or(&domain);

            return Some(GitRemoteInfo {
                domain: final_domain.to_string(),
                path: parts[1].trim_end_matches(".git").to_string(),
            });
        }
    } else if url.starts_with("https://") {
        // HTTPS format: https://domain/path/to/repo.git
        let clean_url = url.trim_start_matches("https://");
        let parts: Vec<&str> = clean_url.splitn(2, '/').collect();
        if parts.len() == 2 {
            return Some(GitRemoteInfo {
                domain: parts[0].to_string(),
                path: parts[1].trim_end_matches(".git").to_string(),
            });
        }
    }
    None
}

/// Retrieves the remote 'origin' URL for the Git repository in the current directory.
///
/// This function executes `git remote get-url origin` and returns the trimmed output.
///
/// # Errors
/// Returns an error if:
/// - The current directory is not a Git repository.
/// - The 'origin' remote is not defined.
/// - The `git` command fails to execute.
pub fn get_remote_url() -> Result<String> {
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .with_context(|| "Failed to execute git remote get-url origin")?;

    if !output.status.success() {
        return Err(anyhow!("Failed to get git remote URL (not in a git repo or no origin)"));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Retrieves a local Git configuration value by key.
/// Returns `Ok(Some(value))` if found, `Ok(None)` if not found, or an `Err` if the git command fails.
pub fn get_local_config(key: &str) -> Result<Option<String>> {
    let output = Command::new("git")
        .args(["config", "--local", "--get", key])
        .output()
        .with_context(|| format!("Failed to execute git config --local --get {}", key))?;

    if output.status.success() {
        Ok(Some(String::from_utf8_lossy(&output.stdout).trim().to_string()))
    } else {
        // git config returns 1 if the key is not found
        Ok(None)
    }
}

/// Expands the tilde `~` in a path to the user's home directory.
pub fn expand_tilde(path: &str) -> Result<String> {
    if path.starts_with('~') {
        let home = dirs::home_dir().context("Could not find home directory for tilde expansion")?;
        Ok(home
            .join(path.trim_start_matches("~/"))
            .to_string_lossy()
            .to_string())
    } else {
        Ok(path.to_string())
    }
}

/// Applies local Git configuration settings to the current repository.
pub fn apply_git_config(name: &str, email: &str, ssh_key_path: &str) -> Result<()> {
    // Resolve tilde in ssh_key_path
    let expanded_ssh_key_path = expand_tilde(ssh_key_path)?;

    // Set user.name
    Command::new("git")
        .args(["config", "--local", "user.name", name])
        .status()
        .with_context(|| "Failed to set user.name")?;

    // Set user.email
    Command::new("git")
        .args(["config", "--local", "user.email", email])
        .status()
        .with_context(|| "Failed to set user.email")?;

    // Set core.sshCommand
    let ssh_command = format!("ssh -i {} -F /dev/null", expanded_ssh_key_path);
    Command::new("git")
        .args(["config", "--local", "core.sshCommand", &ssh_command])
        .status()
        .with_context(|| "Failed to set core.sshCommand")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ssh_standard() {
        let url = "git@github.com:MyOrg/my-repo.git";
        let info = parse_git_url(url).expect("Should parse standard SSH URL");
        assert_eq!(info.domain, "github.com");
        assert_eq!(info.path, "MyOrg/my-repo");
    }

    #[test]
    fn test_parse_ssh_no_suffix() {
        let url = "git@github.com:MyOrg/my-repo";
        let info = parse_git_url(url).expect("Should parse SSH URL without .git suffix");
        assert_eq!(info.domain, "github.com");
        assert_eq!(info.path, "MyOrg/my-repo");
    }

    #[test]
    fn test_parse_ssh_uri_with_port() {
        let url = "ssh://git@gitlab.company.com:2222/group/project.git";
        let info = parse_git_url(url).expect("Should parse SSH URI with port");
        assert_eq!(info.domain, "gitlab.company.com");
        assert_eq!(info.path, "group/project");
    }

    #[test]
    fn test_parse_https_standard() {
        let url = "https://github.com/MyOrg/my-repo.git";
        let info = parse_git_url(url).expect("Should parse standard HTTPS URL");
        assert_eq!(info.domain, "github.com");
        assert_eq!(info.path, "MyOrg/my-repo");
    }

    #[test]
    fn test_parse_https_no_suffix() {
        let url = "https://github.com/MyOrg/my-repo";
        let info = parse_git_url(url).expect("Should parse HTTPS URL without .git suffix");
        assert_eq!(info.domain, "github.com");
        assert_eq!(info.path, "MyOrg/my-repo");
    }

    #[test]
    fn test_parse_custom_host() {
        let url = "https://my-git.internal.net/team-alpha/service-x.git";
        let info = parse_git_url(url).expect("Should parse internal host HTTPS URL");
        assert_eq!(info.domain, "my-git.internal.net");
        assert_eq!(info.path, "team-alpha/service-x");
    }

    #[test]
    fn test_parse_invalid_url() {
        assert!(parse_git_url("not-a-git-url").is_none());
        assert!(parse_git_url("ftp://server/repo.git").is_none());
        assert!(parse_git_url("http://insecure.com/repo.git").is_none());
    }
}
