{ pkgs, lib, config, options, ... }:

let
  inherit (lib)
    mkEnableOption
    mkIf
    mkOption
    mkPackageOption
    types;

  cfg = config.gridder;
in

{
  options.gridder = {
    enable = mkEnableOption "daily gridder generation";

    # NOTE: we expect the user to apply the overlay themselves
    package = mkPackageOption pkgs "Gridder" {
      default = [ "gridder" ];
    };

    spreadsheetID = mkOption {
      type = types.str;
      description = "Spreadsheet ID of the gridder spreadsheet.";
    };

    serviceAccountPath = mkOption {
      type = types.path;
      description = "Path to the service account file to use for authentication.";
    };

    minuteDelay = mkOption {
      type = types.ints.between 0 60;
      description = ''
        Number of minutes after the hour to run task.

        Note that the new page may not be available exactly when your machine clock hits the hour mark.
      '';
      default = 2;
    };

    username = mkOption {
      type = types.str;
      description = "Username of gridder service user";
      default = "gridder";
    };

    group = mkOption {
      type = types.str;
      description = "Name of gridder service user's primary group";
      default = "gridder";
    };
  };

  config = mkIf cfg.enable {
    users.users.gridder = {
      name = cfg.username;
      group = cfg.group;
      isSystemUser = true;
    };

    users.groups.gridder = {
      name = cfg.group;
    };

    systemd = {
      services.gridder = {
        enable = true;
        environment = {
          GRIDDER_SPREADSHEET_ID = cfg.spreadsheetID;
          GRIDDER_SERVICE_ACCOUNT_FILE = cfg.serviceAccountPath;
        };
        unitConfig.description = "Gridder generation task";
        serviceConfig = {
          ExecStart = "${cfg.package}/bin/gridder";
          User = cfg.username;
        };
      };

      timers.gridder = {
        enable = true;
        unitConfig.Description = "Run gridder generation task daily";
        timerConfig = {
          Unit = "gridder.service";
          OnCalendar =
            let
              padding = if cfg.minuteDelay < 10 then "0" else "";
              mins = "${padding}${toString cfg.minuteDelay}";
            in
            "*-*-* 03:${mins}:00 America/New_York";
        };
      };
    };
  };
}

