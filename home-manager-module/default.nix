self:
{
  config,
  lib,
  pkgs,
  ...
}:
with lib;
let
  inherit (pkgs.stdenv.hostPlatform) system;
  nextcloud-tag-sync-pkg = self.packages.${system}.default;
  cfg = config.nextcloud-tag-sync;
  tomlFormat = pkgs.formats.toml { };
in
{
  options.nextcloud-tag-sync = {
    enable = mkOption {
      type = types.bool;
      default = true;
      description = ''
        Whether nextcloud-tag-sync should be active.
      '';
    };

    dry-run = mkOption {
      type = types.bool;
      default = false;
      description = ''
        Whether to actually write the tag changes to the filesystem and Nextcloud.
      '';
    };

    tag-database = mkOption {
      type = types.path;
      default = "${config.xdg.stateHome}/nextcloud-tag-sync.db.json";
      description = "Location to store the tag database used for synchronization";
    };

    frequency = mkOption {
      # type = types.addCheck types.str (
      #   x: lib.runCommandLocal "verifyFrequency" { } "${pkgs.systemd}/bin/systemd-analyze calendar ${x}"
      # );
      type = types.str;
      default = "*:0/5";
      example = "03:15";
      description = ''
        When to run the Nextcloud Tag Sync.

        On Linux this is a string as defined by {manpage}`systemd.time(7)`.

        Defaults to every 5 minutes.
      '';
    };

    keep-on-conflict = mkOption {
      type = types.enum [
        "Both"
        "Left"
        "Right"
      ];
      default = "Both";
      description = ''
        Determines the source of truth during initial sync if tags between files differ.
      '';
    };

    instance-url = mkOption {
      type = types.str;
      description = "URL of the Nextcloud instance to connect to.";
    };

    user = mkOption {
      type = types.str;
      description = "Username to use for login.";
    };

    token = mkOption {
      type = types.str;
      description = "Secret token to use for login";
    };

    prefixes = mkOption {
      type = types.listOf (
        types.submodule {
          options = {
            local = mkOption {
              type = types.path;
              description = "Local path for synchronization";
              example = "/home/user/Documents";
            };
            remote = mkOption {
              type = types.addCheck types.str (lib.hasPrefix "/remote.php/dav/files/");
              description = "Remote path for synchronization";
              example = "/remote.php/dav/files/user/Documents";
            };
          };
        }
      );
      description = "List of prefixes where files are synchronized between a local and a remote path";
    };
  };

  config = mkIf cfg.enable {
    home.packages = [ nextcloud-tag-sync-pkg ];
    xdg.configFile."nextcloud-tag-sync.toml".source = tomlFormat.generate "nextcloud-tag-sync-config" {
      keep_side_on_conflict = cfg.keep-on-conflict;
      nextcloud_instance = cfg.instance-url;
      dry_run = cfg.dry-run;
      tag_database = cfg.tag-database;
      inherit (cfg)
        prefixes
        user
        token
        ;
    };

    systemd.user.timers.nextcloud-tag-sync = {
      Unit.Description = "Automatically synchronize local file system tags with a Nextcloud server";
      Timer = {
        OnCalendar = cfg.frequency;
        DeferReactivation = true;
      };
      Install.WantedBy = [ "timers.target" ];
    };

    systemd.user.services.nextcloud-tag-sync = {
      Unit = {
        Description = "Synchronize local file system tags with a Nextcloud server";
        After = [ "network-online.target" ];
        Wants = [ "network-online.target" ];
      };

      Service = {
        Type = "oneshot";
        Environment = "RUST_LOG=nextcloud_tag_sync=info";
        ExecStart = "${nextcloud-tag-sync-pkg}/bin/nextcloud-tag-sync";
        ReadOnlyPaths = /.;
        # Does not seem to work.
        # ReadWritePaths = lib.concatStringsSep " " (
        #   (builtins.map (prefix: prefix.local) cfg.prefixes)
        #   ++ [
        #     cfg.tag-database
        #   ]
        # );
        ProtectSystem = "strict";
        PrivateTmp = true;
        PrivateDevices = true;
        ProtectKernelTunables = true;
        CapabilityBoundingSet = "";
        NoNewPrivileges = true;
      };
    };
  };
}
