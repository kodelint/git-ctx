use anyhow::{anyhow, Context, Result};
use git2::Repository;

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

/// Retrieves all remote URLs for the Git repository in the current directory.
///
/// # Errors
/// Returns an error if:
/// - The current directory is not a Git repository.
/// - The `git2` library fails to open the repository or retrieve remotes.
pub fn get_remote_urls() -> Result<Vec<String>> {
    let repo = match Repository::open(".") {
        Ok(r) => r,
        Err(e) if e.code() == git2::ErrorCode::NotFound => {
            return Err(anyhow!("Not in a Git repository"));
        }
        Err(e) => return Err(anyhow::Error::from(e).context("Failed to open Git repository")),
    };
    let remotes = repo
        .remotes()
        .with_context(|| "Failed to get Git remotes")?;

    let mut urls = Vec::new();
    for name in remotes.iter().flatten() {
        if let Ok(remote) = repo.find_remote(name) {
            if let Some(url) = remote.url() {
                urls.push(url.to_string());
            }
        }
    }

    if urls.is_empty() {
        return Err(anyhow!("No remotes found in the Git repository"));
    }

    Ok(urls)
}

/// Expands the tilde `~` in a path to the user's home directory.
pub fn expand_tilde(path: &str) -> Result<String> {
    Ok(shellexpand::tilde(path).into_owned())
}

/// Applies local Git configuration settings to the current repository.
pub fn apply_git_config(name: &str, email: &str, ssh_key_path: &str) -> Result<()> {
    let repo = Repository::open(".").with_context(|| "Failed to open Git repository")?;
    let mut config = repo
        .config()
        .with_context(|| "Failed to get Git config")?
        .open_level(git2::ConfigLevel::Local)
        .with_context(|| "Failed to open local Git config")?;

    // Resolve tilde in ssh_key_path
    let expanded_ssh_key_path = expand_tilde(ssh_key_path)?;
    let ssh_command = format!("ssh -i {}", expanded_ssh_key_path);

    // Check current values before writing
    let current_name = config.get_string("user.name").ok();
    let current_email = config.get_string("user.email").ok();
    let current_ssh = config.get_string("core.sshCommand").ok();

    if current_name.as_deref() != Some(name) {
        config
            .set_str("user.name", name)
            .with_context(|| "Failed to set user.name")?;
    }

    if current_email.as_deref() != Some(email) {
        config
            .set_str("user.email", email)
            .with_context(|| "Failed to set user.email")?;
    }

    if current_ssh.as_deref() != Some(&ssh_command) {
        config
            .set_str("core.sshCommand", &ssh_command)
            .with_context(|| "Failed to set core.sshCommand")?;
    }

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
