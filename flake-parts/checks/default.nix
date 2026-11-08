# --- flake-parts/checks/default.nix
# NixOS VM tests: the modules on a machine like a user's, not the owner's.
{ inputs, self, ... }:
{
  perSystem =
    { pkgs, lib, ... }:
    {
      checks = lib.optionalAttrs pkgs.stdenv.hostPlatform.isLinux {
        vm-single-node = pkgs.testers.runNixOSTest (import ./vm-single-node.nix { inherit inputs self; });
      };
    };
}
