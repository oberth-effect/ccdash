# --- flake-parts/docs/default.nix
# The documentation site: mdBook over docs/ with the module options reference
# generated from the modules themselves (`nix build .#docs`; `nix flake check`
# builds it too and fails on a broken relative link).
{
  config,
  inputs,
  lib,
  self,
  ...
}:
let
  inherit (config) flake-root;
  repo = "https://github.com/tsandrini/ccdash/blob/main/";
in
{
  perSystem =
    { config, pkgs, ... }:
    let
      # NOTE: "Declared by" links into the repository, not the store
      relink =
        opt:
        opt
        // {
          declarations = map (
            decl:
            let
              file = lib.removePrefix "${toString flake-root}/" (toString decl);
            in
            {
              url = repo + file;
              name = file;
            }
          ) opt.declarations;
        };

      reference =
        title: intro: options:
        pkgs.runCommand "ccdash-options.md" { } ''
          {
            printf '# %s\n\n%s\n\n' ${lib.escapeShellArg title} ${lib.escapeShellArg intro}
            cat ${
              (pkgs.nixosOptionsDoc {
                inherit options;
                transformOptions = relink;
              }).optionsCommonMark
            }
          } > $out
        '';

      # NOTE: the options are the same everywhere; a Linux eval also works from macOS
      nixos = inputs.nixpkgs.lib.nixosSystem {
        modules = [
          self.nixosModules.hub
          self.nixosModules.default
          {
            nixpkgs.hostPlatform = "x86_64-linux";
            system.stateVersion = "25.05";
          }
        ];
      };

      home = inputs.home-manager.lib.homeManagerConfiguration {
        inherit pkgs;
        modules = [
          self.homeModules.default
          {
            home = {
              username = "me";
              homeDirectory = "/home/me";
              stateVersion = "25.05";
            };
          }
        ];
      };

      hub =
        reference "Hub module options"
          ''
            The NixOS modules: `nixosModules.hub` (`services.ccdash-hub`) runs the
            hub, `nixosModules.default` (`programs.ccdash`) allowlists the channel.
            Generated from the modules; keys the options do not cover go into
            `settings` ([configuration](../configuration.md#3-hub-keys)).
          ''
          {
            services.ccdash-hub = nixos.options.services.ccdash-hub;
            programs.ccdash = nixos.options.programs.ccdash;
          };

      homeManager = reference "Home Manager module options" ''
        `homeModules.default` (`services.ccdash`) runs the collector on a
        workstation and wires Claude Code to it. Generated from the module;
        keys the options do not cover go into `settings`
        ([configuration](../configuration.md#4-collector-keys)).
      '' { services.ccdash = home.options.services.ccdash; };
    in
    {
      packages.docs = pkgs.stdenvNoCC.mkDerivation {
        name = "ccdash-docs";
        src = flake-root + "/docs";
        nativeBuildInputs = [
          pkgs.mdbook
          pkgs.python3
        ];
        buildPhase = ''
          runHook preBuild
          mkdir -p options
          cp ${hub} options/hub.md
          cp ${homeManager} options/home-manager.md
          mdbook build --dest-dir "$out" .
          python3 ${./check-links.py} "$out"
          runHook postBuild
        '';
        dontInstall = true;
      };

      checks.docs = config.packages.docs;
    };
}
