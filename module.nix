{ config, lib, pkgs, ... }:

with lib;

let
  cfg = config.services.fyrnwita;
  user = "fyrnwita";
  group = user;
  settingsFormat = pkgs.formats.json { };
  configFile = settingsFormat.generate "config.json" cfg.settings;
in
{
  options = {
    services.fyrnwita = {
      enable = mkEnableOption (lib.mdDoc "Fyrnwita, a discord quote bot.");

      environmentFile = mkOption {
        type = types.path;
        description = lib.mdDoc "Path to environment variable file to use with service";
      };

      settings = mkOptions {
        inherit (settingsFormat) type;
        description = lib.mdDoc "Settings";
        example = {
          hordPath = "/var/lib/fyrnwita/hord.sl3";
          expungedMessage = "< Quote Expunged >";
          adminUsers = [ ];
          reactions = {
            "hello world" = "👋";
          };
        };
      };
    };
  };

  config = mkIf cfg.enable {
    users.users.${user} = {
      inherit group;
      description = "Fyrnwita system user";
      isSystemUser = true;
    };

    users.groups = {
      fyrnwita = { };
    };

    systemd.services = {
      fyrnwita = {
        description = "Fyrnwita service";
        after = [ "network.target" ];
        wantedBy = [ "multi-user.target" ];
        serviceConfig = {
          Restart = "on-failure";
          User = user;
          Group = group;
          ExecStart = "${pkgs.fyrnwita}/bin/fyrnwita ${configFile}";
          EnvironmentFile = cfg.environmentFile;
        };
      };
    };
  };
}
