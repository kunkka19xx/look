self:
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.programs.lookapp;

  # `ui_theme` values as written by the settings dropdown
  # (src/html/screens/settings.html). The default theme is stored as an empty
  # value, so the friendly name maps to "".
  themeIds = {
    catppuccin = "";
    tokyo-night = "tokyo-night";
    rose-pine = "rose-pine";
    gruvbox = "gruvbox";
    dracula = "dracula";
    kanagawa = "kanagawa";
    kindle = "kindle";
    liquid = "liquid";
    custom = "custom";
  };

  # Colours are derived from `ui_theme` at startup (applyThemePreset), but the
  # opacity keys stay user-owned across theme switches (USER_CONTROLLED_KEYS in
  # settings.js), so the two presets that own them (OPACITY_OWNING_THEMES) have
  # to restate them. Values mirror THEME_PRESETS.
  themeOpacity = {
    kindle = {
      ui_tint_opacity = 0.93;
      ui_font_opacity = 1.0;
      ui_border_opacity = 0.26;
    };
    liquid = {
      ui_tint_opacity = 0.78;
      ui_font_opacity = 1.0;
      ui_border_opacity = 0.1;
    };
  };

  themeSettings =
    if cfg.theme == null then
      { }
    else
      {
        ui_theme = themeIds.${cfg.theme};
      }
      // (themeOpacity.${cfg.theme} or { });

  finalSettings = themeSettings // cfg.settings;

  # core splits `ignored_patterns_*` and `alias_*` on `|` (parse_pattern_values)
  # and every other list key on `,` (parse_csv), which also unescapes `\,`/`\\`.
  isPipeSeparated = name: lib.hasPrefix "ignored_patterns_" name || lib.hasPrefix "alias_" name;

  escapeCsvEntry = entry: lib.replaceStrings [ "\\" "," ] [ "\\\\" "\\," ] (toString entry);

  # `toString 0.93` renders "0.930000". Both parsers accept it, but the file is
  # meant to be read, so trim the padding.
  formatFloat =
    value:
    let
      rendered = toString value;
      trimmed = builtins.match "(-?[0-9]+(\\.[0-9]*[1-9])?)\\.?0*" rendered;
    in
    if trimmed == null then rendered else builtins.head trimmed;

  formatValue =
    name: value:
    if lib.isBool value then
      lib.boolToString value
    else if lib.isFloat value then
      formatFloat value
    else if !lib.isList value then
      toString value
    else if isPipeSeparated name then
      lib.concatStringsSep "|" (map toString value)
    else
      lib.concatStringsSep "," (map escapeCsvEntry value);

  aliasSettings = lib.mapAttrs' (
    name: values: lib.nameValuePair "alias_${name}" (lib.concatStringsSep "|" values)
  ) cfg.aliases;

  managedSettings = finalSettings // aliasSettings;
  managedKeys = lib.attrNames managedSettings;

  managedFile = pkgs.writeText "look-config-managed" (
    lib.concatMapStringsSep "\n" (
      name: "${name}=${formatValue name managedSettings.${name}}"
    ) managedKeys
    + "\n"
  );

  managedKeysFile = pkgs.writeText "look-config-managed-keys" (
    lib.concatStringsSep "\n" managedKeys + "\n"
  );

  stateFile = "${config.xdg.stateHome}/lookapp/home-manager-keys";

  # Look merges its own writes into its config file line by line and keeps keys
  # it does not know about (set_config in src-tauri/src/config.rs), so replacing
  # the whole file on activation would throw away everything the user changed
  # in-app. Merge the same way instead: managed keys win, everything else is
  # left alone, and keys dropped from the Nix config since the last generation
  # are removed so the file cannot go stale.
  #
  # Target whichever file Look reads (core/engine/src/config_path.rs): the
  # legacy ~/.look.config until Look copies it into ~/.look/ on its next launch,
  # which carries these keys across. Writing to it after that applies nothing.
  mergeScript = pkgs.writeShellScript "lookapp-merge-config" ''
    set -eu
    export PATH=${lib.makeBinPath [ pkgs.coreutils ]}

    managed=$1
    managed_keys=$2
    prev_keys_file=$3
    target=$4
    legacy=$5

    if [ ! -e "$target" ] && [ -e "$legacy" ]; then
      case "$(head -n 1 "$legacy")" in
        '# Moved to '*) ;;
        *) target=$legacy ;;
      esac
    fi

    keys=" "
    while IFS= read -r key; do
      [ -n "$key" ] || continue
      keys="$keys$key "
    done < "$managed_keys"

    prev_keys=" "
    if [ -f "$prev_keys_file" ]; then
      while IFS= read -r key; do
        [ -n "$key" ] || continue
        prev_keys="$prev_keys$key "
      done < "$prev_keys_file"
    fi

    # Nothing managed and nothing to clean up: leave the file untouched.
    if [ "$keys" = " " ] && [ "$prev_keys" = " " ]; then
      exit 0
    fi

    if [ ! -e "$target" ]; then
      if [ "$keys" = " " ]; then
        exit 0
      fi
      mkdir -p "$(dirname "$target")"
      {
        printf '# look configuration\n'
        printf '# Keys below are managed by Home Manager. Edit your Nix config, not this file.\n\n'
        cat "$managed"
      } > "$target"
      chmod 0644 "$target"
      exit 0
    fi

    if [ ! -e "$target.hm-backup" ]; then
      cp --no-preserve=mode "$target" "$target.hm-backup"
    fi

    lookup() {
      while IFS= read -r entry; do
        case $entry in
          "$1="*)
            printf '%s\n' "$entry"
            return 0
            ;;
        esac
      done < "$managed"
      return 1
    }

    seen=" "
    {
      while IFS= read -r line || [ -n "$line" ]; do
        trimmed=''${line#"''${line%%[![:space:]]*}"}
        case $trimmed in
          "" | \#*)
            printf '%s\n' "$line"
            continue
            ;;
          *=*) key=''${trimmed%%=*} ;;
          *)
            printf '%s\n' "$line"
            continue
            ;;
        esac
        key=''${key%"''${key##*[![:space:]]}"}

        case $keys in
          *" $key "*)
            # Managed: first occurrence takes the Nix value, later duplicates go
            # away so the last line in the file cannot win at parse time.
            case $seen in
              *" $key "*) continue ;;
            esac
            if entry=$(lookup "$key"); then
              printf '%s\n' "$entry"
            else
              printf '%s\n' "$line"
            fi
            seen="$seen$key "
            ;;
          *)
            # Dropped from the Nix config since the last generation.
            case $prev_keys in
              *" $key "*) continue ;;
            esac
            printf '%s\n' "$line"
            ;;
        esac
      done < "$target"

      while IFS= read -r entry; do
        [ -n "$entry" ] || continue
        key=''${entry%%=*}
        case $seen in
          *" $key "*) continue ;;
        esac
        printf '%s\n' "$entry"
        seen="$seen$key "
      done < "$managed"
    } > "$target.hm-new"

    mv "$target.hm-new" "$target"
    chmod 0644 "$target"
  '';

  conflictingAliases = lib.intersectLists (map (name: "alias_${name}") (lib.attrNames cfg.aliases)) (
    lib.attrNames cfg.settings
  );
in
{
  options.programs.lookapp = {
    enable = lib.mkEnableOption "Look, a keyboard-first desktop launcher";

    package = lib.mkOption {
      type = lib.types.nullOr lib.types.package;
      default = import ./default-package.nix self pkgs;
      defaultText = lib.literalExpression "inputs.look.packages.\${pkgs.stdenv.hostPlatform.system}.default";
      description = ''
        The Look package to install. `null` installs nothing and manages only
        `~/.look/config`, for when Look is already installed system-wide.
      '';
    };

    theme = lib.mkOption {
      type = lib.types.nullOr (lib.types.enum (lib.attrNames themeIds));
      default = null;
      example = "kindle";
      description = ''
        Built-in Look theme preset to write as `ui_theme`. `null` leaves the
        theme unmanaged. `custom` keeps whatever `ui_*` values are already in
        the config, for setting every colour through {option}`settings`.
      '';
    };

    settings = lib.mkOption {
      type = lib.types.attrsOf (
        lib.types.oneOf [
          lib.types.bool
          lib.types.int
          lib.types.float
          lib.types.str
          (lib.types.listOf lib.types.str)
        ]
      );
      default = { };
      example = lib.literalExpression ''
        {
          ui_tint_opacity = 0.93;
          running_apps_placement = "right";
          file_scan_extra_roots = [ "~/Projects" "/mnt/data" ];
          ai_enabled = false;
        }
      '';
      description = ''
        Settings written to ~/.look/config. Attribute names map directly to
        Look config keys, for example `ui_theme` becomes `ui_theme=...`.
        Lists are serialized as comma-separated values, except for
        `ignored_patterns_*` and `alias_*` keys, which Look parses as
        pipe-separated.
      '';
    };

    aliases = lib.mkOption {
      type = lib.types.attrsOf (lib.types.listOf lib.types.str);
      default = { };
      example = lib.literalExpression ''
        {
          note = [ "Obsidian" "Logseq" ];
          term = [ "Alacritty" "Kitty" ];
        }
      '';
      description = ''
        Search aliases written as `alias_<name>=Term1|Term2`. An empty list
        clears Look's built-in alias of that name.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = conflictingAliases == [ ];
        message = "programs.lookapp: ${lib.concatStringsSep ", " conflictingAliases} set in both settings and aliases. Use one or the other.";
      }
    ];

    home.packages = lib.optional (cfg.package != null) cfg.package;

    # Look writes its config itself on every settings change, so a home.file
    # symlink into the read-only store makes those writes fail and the frontend
    # only logs the error. Merge into a mutable file instead.
    home.activation.lookappConfig = lib.hm.dag.entryAfter [ "writeBoundary" ] ''
      run ${mergeScript} ${managedFile} ${managedKeysFile} ${lib.escapeShellArg stateFile} "$HOME/.look/config" "$HOME/.look.config"
      run install -Dm0644 ${managedKeysFile} ${lib.escapeShellArg stateFile}
    '';
  };
}
