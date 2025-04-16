# Nextcloud Tag Sync

This program synchronizes tags on files between Nextcloud and a local file system.
Tags are Nextcloud collaborative tags on the remote server.
For the local file system, they are the value extended file system attribute `user.xdg.tags` (configurable).
This attribute is integrated into file explorer's like Dolphin (requires enabled `baloosearch`) but can also be changed/checked from the command line:

```bash
# Write tags to a file
setfattr -n user.xdg.tags -v 'tag1,tag2,tag3' path/to/file
# Check tag of a single file
getfattr -n user.xdg.tags path/to/file
# Remove all tags
setfattr -x user.xdg.tags path/to/file
# List all tags in the current directory and its children
getfattr -R . -n user.xdg.tags 2> /dev/null # Ignore errors from untagged files
```

## Limitations 

- No guarantees that it won't garble your tags. Make sure to have backups first. For due diligence, I have some [integration tests running against the Nextcloud All-In-One docker image](./tests) and a [fuzz test to ensure that the tag difference computation works](./fuzz). 
- Only supports Linux. (It might work on Mac too but don't quote me on that.)
- The file synchronization must be performed by an external program (e.g. [official Nextcloud client](https://nextcloud.com/de/features/#clients)).

## Installation

### NixOS / HomeManager

With flakes:

```nix
inputs.nextcloud-tag-sync = {
    url = "github:ede1998/nextcloud-tag-sync";
    inputs = {
        nixpkgs.follows = "nixpkgs";
        home-manager.follows = "home-manager";
    };
};
```

There is a HomeManager module with the same options as listed under configuration, just in `kebap-case` instead of `snake_case`. It also automatically creates a systemd user service to periodically synchronize tags.
A configuration example can be found here:
https://github.com/ede1998/nix-config/blob/9b1d855f1287b3ca2242ecc2f0a13d6e5d9380c4/home-manager/nextcloud.nix#L32

### Other distributions

Build and install it yourself with cargo:

```bash
cargo install --git https://github.com/ede1998/nextcloud-tag-sync.git nextcloud-tag-sync
```

## Configuration

The program searches for a configuration file named `nextcloud-tag-sync.toml` with the following precedence:

1. In the current working directory.
2. In the user's configuration directory, e.g. `~/.config` or `$XDG_CONFIG_HOME`.
3. Recursively up the directory tree starting at the current working directory.

The file should have the following contents:

```toml
# Location for database file.
# Contains mapping of files and tags observed during the last run.
# Required to reliably resolve differences in tags on the same file.
# If missing, i.e. during initial run, configured fallback assumption is used (see keep_side_on_conflict).
tag_database = "/path/to/database/nextcloud-tag-sync.db.json"
# During initial synchronization (no db exists), decides which tags to keep when the same file has a different set of tags.
# Both = Use union of both tag sets
# Left = Use local tags, discard remote tags
# Right = Use remote tags, discard local tags
keep_side_on_conflict = "Both"
# URL to the Nextcloud instance
nextcloud_instance = "https://my.nextcloud.example"
# Nextcloud user name
user = "user"
# Nextcloud user secret
token = "access token (or password)"
# Do not update anything and just compute differences if true.
# Defaults to true.
dry_run = false

# Zero or more blocks of mappings between local filesystem paths and Nextcloud paths.
# Assumes external synchronization program running for these paths, e.g. official Nextcloud client.
[[prefixes]]
# Local filesystem path
local = "/home/erik/Documents"
# Corresponding Nextcloud path
# Must always start with `/remote.php/dav/files/<user>`
remote = "/remote.php/dav/files/user/MyDocuments"

# You probably don't need those options:

# How many in-flight requests to the Nextcloud server are allowed?
max_concurrent_requests=10
# Extended file system attribute to use for tagging files
local_tag_property_name="user.xdg.tags"
```

Note: Individual keys can be overridden by environment variables by prefixing with `NCTS_`, e.g. `NCTS_DRY_RUN=true`.

Logging can be [configured](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#example-syntax) by environment variables, e.g. `RUST_LOG=nextcloud_tag_sync=info`.
Warning: The `debug` log level is very verbose and prints multiple lines per inspected file.

## Automatic periodic synchronization with systemd

(Already included in the home-manager module.)

`~/.config/systemd/user/nextcloud-tag-sync.service`:

```systemd
[Service]
Type=oneshot
# Give some logging information with `systemctl status`
Environment=RUST_LOG=nextcloud_tag_sync=info
ExecStart=/home/user/.cargo/bin/nextcloud-tag-sync

# Security Hardening
CapabilityBoundingSet=
NoNewPrivileges=true
PrivateDevices=true
PrivateTmp=true
ProtectKernelTunables=true
ProtectSystem=strict
ReadOnlyPaths=/

[Unit]
After=network-online.target
Description=Synchronize local file system tags with a Nextcloud server
Wants=network-online.target

[Install]
WantedBy=timers.target
```

`~/.config/systemd/user/nextcloud-tag-sync.timer`:

```systemd
[Timer]
DeferReactivation=true
# Activate once per hour at the minute after the hour (e.g. 01:01, 02:01, etc.)
OnCalendar=*:1

[Unit]
Description=Automatically synchronize local file system tags with a Nextcloud server
```