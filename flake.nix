{
  description = "Urbit apps";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";

    mcp = {
      url = "github:tloncorp/mcp";
      flake = false;
    };
    # Desk dependencies pinned by mcp's build.zig.
    urbit = {
      url = "github:urbit/urbit/0f94550b941dfe046d9dff4a541330bd084e8cd1";
      flake = false;
    };
    tlon-apps = {
      url = "github:tloncorp/tlon-apps/9f0c94771e4773567a2f55a727ffa31b0f6e8e9f";
      flake = false;
    };

    # Builds its full desk itself, with the dependencies mortar.yaml pins.
    deskjam = {
      url = "github:iko-soy/deskjam";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
    };

    # Its packages.<system>.default is the full desk: it vendors its lib/,
    # mar/ and sur/ files.
    shelf = {
      url = "github:iko-soy/shelf";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    # Its packages.<system>.desk is the full desk: desk/ laid over the marks
    # |new-desk copies from %base, from the kernel its own flake pins.
    hits = {
      url = "github:iko-soy/hits";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    silo = {
      url = "github:arthyn/silo";
      flake = false;
    };
    # Desk dependencies, as silo's .github/helpers/deploy.sh takes them. It
    # takes urbit at a tag from a repo variable; 408k-2 is the 408 kernel,
    # the oldest silo's sys.kelvin lists.
    silo-urbit = {
      url = "github:urbit/urbit/408k-2";
      flake = false;
    };
    silo-landscape = {
      url = "github:tloncorp/landscape/master";
      flake = false;
    };
  };

  outputs =
    {
      nixpkgs,
      flake-utils,
      mcp,
      urbit,
      tlon-apps,
      deskjam,
      shelf,
      hits,
      silo,
      silo-urbit,
      silo-landscape,
      ...
    }:
    # nixpkgs 26.11 dropped x86_64-darwin, and the apps' own flakes build their
    # desks on these same three systems.
    flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" ] (
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

        # Pack a desk into $out/<desk>.deskjam. `src` is copied as the base of
        # the desk, then each entry of `imports` copies `paths` (relative to
        # the desk root) from the directory `from`.
        #
        # When the desk's docket names a glob, its frontend (glob-ames or
        # glob-http), `glob` must be the built frontend, which goes next to the
        # deskjam as $out/<desk>-glob, the folder to upload at /docket/upload.
        # The build fails without it, so a deskjam never leaves here without
        # its glob.
        mkDeskjam =
          {
            desk,
            src,
            imports ? [ ],
            glob ? null,
          }:
          pkgs.runCommand "${desk}.deskjam" { nativeBuildInputs = [ jammer ]; } ''
            cp -r --no-preserve=mode ${src} ${desk}
            ${pkgs.lib.concatMapStrings (
              { from, paths }:
              pkgs.lib.concatMapStrings (path: ''
                install -D -m 644 ${from}/${path} ${desk}/${path}
              '') paths
            ) imports}
            glob=${toString glob}
            if grep -Eqs '(^|[[:space:]])glob-(ames|http)\+' ${desk}/desk.docket-0; then
              if [ -z "$glob" ]; then
                echo "The docket of %${desk} names a glob: give mkDeskjam the built frontend as glob" >&2
                exit 1
              fi
            elif [ -n "$glob" ]; then
              echo "The docket of %${desk} names no glob, but mkDeskjam got one" >&2
              exit 1
            fi
            mkdir $out
            jammer jam ${desk} -d ${desk} -o $out/${desk}.deskjam
            if [ -n "$glob" ]; then
              cp -r --no-preserve=mode "$glob" $out/${desk}-glob
            fi
          '';

        # Mirrors the dependency list in mcp's build.zig.
        mcp-desk = mkDeskjam {
          desk = "mcp";
          src = "${mcp}/desk";
          imports = [
            {
              from = "${urbit}/pkg/arvo";
              paths = [ "lib/pretty-file.hoon" ];
            }
            {
              from = "${tlon-apps}/desk";
              paths = [ "lib/test-agent.hoon" ];
            }
            {
              from = "${urbit}/pkg/base-dev";
              paths = [
                "lib/dbug.hoon"
                "lib/default-agent.hoon"
                "lib/server.hoon"
                "lib/skeleton.hoon"
                "lib/strand.hoon"
                "lib/strandio.hoon"
                "lib/test.hoon"
                "lib/verb.hoon"
                "mar/bill.hoon"
                "mar/hoon.hoon"
                "mar/json.hoon"
                "mar/kelvin.hoon"
                "mar/mime.hoon"
                "mar/noun.hoon"
                "mar/ship.hoon"
                "mar/sole/action.hoon"
                "mar/sole/effect.hoon"
                "mar/txt.hoon"
                "sur/sole.hoon"
                "sur/spider.hoon"
                "sur/verb.hoon"
              ];
            }
          ];
        };

        deskjam-desk = mkDeskjam {
          desk = "deskjam";
          src = deskjam.packages.${system}.deskjam;
        };

        shelf-desk = mkDeskjam {
          desk = "shelf";
          src = shelf.packages.${system}.default;
        };

        # Build a frontend: `npm run build` in its npm project `src`, whose
        # dist/ is the folder of files a glob holds.
        mkGlob =
          {
            desk,
            src,
            npmDepsHash,
          }:
          pkgs.buildNpmPackage {
            name = "${desk}-glob";
            inherit src npmDepsHash;
            installPhase = ''
              runHook preInstall
              cp -r dist $out
              runHook postInstall
            '';
          };

        hits-glob = mkGlob {
          desk = "hits";
          src = "${hits}/ui";
          npmDepsHash = "sha256-oPvqiIG36Dl97R9/G/Op9V/kimsbAjxiZv7RXlPhrgo=";
        };

        hits-desk = mkDeskjam {
          desk = "hits";
          src = hits.packages.${system}.desk;
          glob = hits-glob;
        };

        silo-glob = mkGlob {
          desk = "silo";
          src = "${silo}/ui";
          npmDepsHash = "sha256-kZaVn4+ORa9Cm100f8/VS1sCWwHOC6rxDvLnz4S5+p0=";
        };

        # The desk silo's deploy.sh commits: all of urbit's pkg/base-dev, then
        # landscape's desk-dev, then silo's desk/ over them.
        silo-desk = mkDeskjam {
          desk = "silo";
          src = pkgs.runCommand "silo-desk" { } ''
            mkdir $out
            cp -rL --no-preserve=mode ${silo-urbit}/pkg/base-dev/. $out/
            cp -rL --no-preserve=mode ${silo-landscape}/desk-dev/. $out/
            cp -rL --no-preserve=mode ${silo}/desk/. $out/
          '';
          glob = silo-glob;
        };
      in
      {
        packages = {
          inherit jammer;
          mcp = mcp-desk;
          deskjam = deskjam-desk;
          shelf = shelf-desk;
          hits = hits-desk;
          inherit hits-glob;
          silo = silo-desk;
          inherit silo-glob;
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
