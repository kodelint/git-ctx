# git-ctx

![git-ctx](./git-ctx.png)

`git-ctx` is a CLI tool designed to automatically manage Git profiles based on the repository's remote origin URL. It uses a shell hook to silently apply local repository configurations (`user.name`, `user.email`, and `core.sshCommand`) whenever you change directories.

## Features

- **Automated Profile Switching:** Instantly applies the correct Git profile when entering a repository.
- **Support for SSH and HTTPS:** Matches profiles based on your remote URL patterns.
- **Shell Hook Support:** Seamlessly integrates with Zsh and Bash.

## Installation

1.  **Clone and Build:**
    ```bash
    git clone <repository_url>
    cd git-ctx
    cargo build --release
    ```
2.  **Move to Path:**
    ```bash
    cp target/release/git-ctx /usr/local/bin/
    ```

## Configuration

By default, `git-ctx` looks for the configuration file at `~/.config/git-ctx/profiles.toml`.

### Environment Variable Override

You can override the default configuration path by setting the `GIT_CTX_CONFIG` environment variable:

```bash
export GIT_CTX_CONFIG="/path/to/your/profiles.toml"
```

### Example `profiles.toml`

```toml
[[profiles]]
name = "Personal User"
email = "personal@email.com"
ssh_key_path = "~/.ssh/id_rsa_personal"
match_pattern = "github.com/PersonalOrg"

[[profiles]]
name = "Work User"
email = "work@company.com"
ssh_key_path = "~/.ssh/id_ed25519_work"
match_pattern = "gitlab.work.com"
```

## Setup Shell Hook

Add the following to your shell configuration file (`.zshrc` or `.bashrc`):

```bash
eval "$(git-ctx init-hook)"
```

Alternatively, you can manually add the output of `git-ctx init-hook` into your config file.

## Usage Examples

### 1. Interactive Setup
You can easily add a new profile using the interactive `add` command:

```bash
git-ctx add
# Follow the prompts:
# Profile Name: Jane Doe
# Email: jane.doe@work.com
# SSH Key Path: ~/.ssh/id_ed25519_work
# Match Pattern (Regex): github\.com/work-org/.*
```

### 2. Automatic Switching in Action
Once the shell hook is installed, the tool works silently in the background.

```bash
# Enter a personal repository
cd ~/projects/personal-repo
# git-ctx automatically sets user.name="Personal User" and core.sshCommand="ssh -i ~/.ssh/id_rsa_personal"

# Switch to a work repository
cd ~/projects/work-repo
# [git-ctx] Switched to profile 'Work User' (jane.doe@work.com)
```

### 3. Verify Your Setup
Use the `doctor` command to ensure everything is configured correctly:

```bash
git-ctx doctor
# Checking configuration...
# ✅ Config file found at: "/Users/user/.config/git-ctx/profiles.toml"
# ✅ Config is valid (2 profiles)
#
# Checking shell hook status...
# ℹ️  $GIT_CTX_CONFIG is not set (optional)
```

## Commands

### `git-ctx auto`
The core command used by the shell hook. It scans all remotes in the current repository and applies the first matching profile. It is optimized for performance using `git2` to minimize shell latency.

### `git-ctx add`
Add a new profile via CLI arguments or interactive prompts.
```bash
git-ctx add --name "Work" --email "work@corp.com" --ssh-key-path "~/.ssh/id_work" --match-pattern "corp\.com"
```

### `git-ctx list`
Displays all configured profiles in a formatted table.

### `git-ctx doctor`
Diagnoses common issues with the configuration file or shell environment.

### `git-ctx init-hook`
Generates the shell script required for automatic profile switching.

## Configuration

By default, `git-ctx` looks for the configuration file at `~/.config/git-ctx/profiles.toml` (following XDG standards where applicable).


### Run Tests

```bash
cargo test
```
