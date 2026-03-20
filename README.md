# git-ctx

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

## Usage

### Global Options

- `-d`, `--debug`: Enable debug logging to see how `git-ctx` matches profiles and applies configurations.
- `-q`, `--quiet`: Suppress all non-error output (hides the profile switch notification).
- `-h`, `--help`: Print help information.
- `-V`, `--version`: Print version information.

### Profile Switch Notifications

`git-ctx` will automatically notify you via `stderr` when it detects a different profile is required for the repository you just entered. It only shows this message if it actually changes the local Git configuration.

Example output:
`[git-ctx] Switched to profile 'Work' (work@company.com)`

To disable these notifications, add the `--quiet` flag to your shell hook initialization:
`eval "$(git-ctx --quiet init-hook)"` (though `init-hook` itself doesn't need it, you can manually edit the hook or we can update `init-hook` to include it).

### Debugging

If the tool is not behaving as expected, you can enable debug logging using the `--debug` flag:

```bash
git-ctx --debug auto
```

Alternatively, you can use the `RUST_LOG` environment variable:

```bash
RUST_LOG=debug git-ctx auto
```

### `git-ctx auto`

This is the command called by the shell hook. It checks for a git repository, parses the remote URL, and applies the matching profile from your configuration. It fails silently and quickly if you are not in a Git repo.

### `git-ctx list`

List all current profiles in a readable format:

```bash
git-ctx list
```

### `git-ctx init-hook`

Outputs the shell code required to initialize the `chpwd` (Zsh) or `cd` wrap (Bash) hook.

## Development and Testing

`git-ctx` is built with Rust and uses unit tests for core logic.

### Run Tests

```bash
cargo test
```
