{
  description = "Urbit apps";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    { nixpkgs, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        cargoToml = builtins.fromTOML (builtins.readFile ./jammer/Cargo.toml);

        jammer = pkgs.rustPlatform.buildRustPackage {
          pname = cargoToml.package.name;
          version = cargoToml.package.version;
          src = pkgs.lib.cleanSourceWith {
            src = ./jammer;
            filter = path: type: !(type == "directory" && baseNameOf path == "target");
          };
          cargoLock.lockFile = ./jammer/Cargo.lock;
          meta = {
            description = cargoToml.package.description;
            mainProgram = "jammer";
          };
        };
      in
      {
        packages = {
          inherit jammer;
          default = jammer;
        };

        apps.default = flake-utils.lib.mkApp { drv = jammer; };

        devShells.default = pkgs.mkShell {
          inputsFrom = [ jammer ];
          packages = with pkgs; [
            rust-analyzer
            clippy
            rustfmt
          ];
        };
      }
    );
}
