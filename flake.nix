{
  description = "Urbit apps";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";

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
    # takes urbit at a tag from a repo variable; 408k-2 is the release of the
    # 408 kernel, the newest silo's sys.kelvin lists.
    silo-urbit = {
      url = "github:urbit/urbit/408k-2";
      flake = false;
    };
    silo-landscape = {
      url = "github:tloncorp/landscape/master";
      flake = false;
    };

    # Its desk/ is the full desk: it vendors its lib/, mar/ and sur/ files.
    accountability = {
      url = "github:sigilante/accountability";
      flake = false;
    };

    harness = {
      url = "github:mopfel-winrux/urbit-agent-harness";
      flake = false;
    };
    # The commits harness' build.zig and scripts/stage-tlon.mjs fetch.
    harness-grubbery = {
      url = "github:gwbtc/grubbery/20ef61108f4ffae0c40ccd90e0ee7ae4d475709f";
      flake = false;
    };
    harness-tlon = {
      url = "github:tloncorp/tlon-apps/938f0c44d693f6f7391cca8107c7b3a40b834a01";
      flake = false;
    };
  };

  outputs =
    {
      nixpkgs,
      flake-utils,
      deskjam,
      shelf,
      hits,
      silo,
      silo-urbit,
      silo-landscape,
      accountability,
      harness,
      harness-grubbery,
      harness-tlon,
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
          # Only what cargo builds and tests, so editing the README doesn't
          # rebuild jammer and every deskjam.
          src = pkgs.lib.fileset.toSource {
            root = ./jammer;
            fileset = pkgs.lib.fileset.unions [
              ./jammer/Cargo.toml
              ./jammer/Cargo.lock
              ./jammer/src
              ./jammer/tests
            ];
          };
          cargoLock.lockFile = ./jammer/Cargo.lock;
          meta = {
            description = cargoToml.package.description;
            mainProgram = "jammer";
          };
        };

        # Exits 0 if the docket file it's given names a glob: a glob-ames or
        # glob-http term outside the docket's cords, tapes and comments,
        # however the docket is laid out.
        names-glob = pkgs.writers.writePerlBin "names-glob" { } ''
          local $/;
          my $docket = <>;
          $docket =~ s/'(?:\\.|[^'\\])*'|"(?:\\.|[^"\\])*"|::[^\n]*//g;
          exit($docket =~ /(?<![\w-])glob-(?:ames|http)(?![\w-])/ ? 0 : 1);
        '';

        # Pack the desk `src` into $out/<desk>.deskjam.
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
            glob ? null,
          }:
          pkgs.runCommand "${desk}.deskjam"
            {
              nativeBuildInputs = [
                jammer
                names-glob
              ];
            }
            ''
              cp -r --no-preserve=mode ${src} ${desk}
              glob=${pkgs.lib.optionalString (glob != null) "${glob}"}
              if [ -f ${desk}/desk.docket-0 ] && names-glob ${desk}/desk.docket-0; then
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

        accountability-desk = mkDeskjam {
          desk = "accountability";
          src = "${accountability}/desk";
        };

        # harness' build.zig fetches grubbery, and its scripts/stage-tlon.mjs
        # Tlon's files, with git, at commits they pin. This git answers from
        # the inputs pinned to the same commits, and fails on any other, so a
        # new pin upstream stops the build until the inputs follow it.
        harness-git = pkgs.writeShellScriptBin "git" ''
          set -eu
          if [ "$1" != -C ]; then
            echo "git: unexpected command: $*" >&2
            exit 1
          fi
          repo=$2
          shift 2
          case $repo in
            desk-deps/grubbery) src=${harness-grubbery} rev=${harness-grubbery.rev} ;;
            desk-deps/tlon) src=${harness-tlon} rev=${harness-tlon.rev} ;;
            *) echo "git: unexpected repo: $repo" >&2; exit 1 ;;
          esac
          pinned() {
            if [ "$1" != "$rev" ]; then
              echo "git: $repo is pinned to $rev, but the build wants $1" >&2
              exit 1
            fi
          }
          case $1 in
            init | remote) ;;
            cat-file) pinned "''${3%"^{commit}"}" ;;
            checkout) for arg; do :; done; pinned "$arg" ;;
            show) pinned "''${2%%:*}"; cat "$src/''${2#*:}" ;;
            *) echo "git: unexpected command: $*" >&2; exit 1 ;;
          esac
        '';

        # The desk harness' build.zig assembles: desk/, with fe/ built into
        # its web/, over a pruned and patched grubbery desk, with Tlon's files.
        # build.zig runs npm itself, which can't work in the sandbox, so fe/ is
        # built here first instead.
        harness-src = pkgs.stdenv.mkDerivation {
          name = "harness-desk";
          src = harness;
          npmRoot = "fe";
          npmDeps = pkgs.fetchNpmDeps {
            name = "harness-fe-npm-deps";
            src = "${harness}/fe";
            hash = "sha256-m5yMZFEUJV4HPrvWRIIyJ7psXy43W+4gZEcrcHHFdu4=";
          };
          nativeBuildInputs = [
            pkgs.zig_0_15
            pkgs.nodejs
            pkgs.npmHooks.npmConfigHook
            harness-git
          ];
          postPatch = ''
            substituteInPlace build.zig \
              --replace-fail 'try run(step, &.{ "npm", "ci", "--prefix", "fe", "--no-audit", "--no-fund" });' "" \
              --replace-fail 'try run(step, &.{ "npm", "run", "build", "--prefix", "fe" });' ""
          '';
          buildPhase = ''
            runHook preBuild
            npm run build --prefix fe
            mkdir -p desk-deps
            cp -r --no-preserve=mode ${harness-grubbery} desk-deps/grubbery
            ZIG_GLOBAL_CACHE_DIR=$TMPDIR/zig zig build
            runHook postBuild
          '';
          installPhase = ''
            if [ ! -f zig-out/web/index.html ]; then
              echo "harness' desk has no web/index.html: its frontend is missing" >&2
              exit 1
            fi
            cp -r zig-out $out
          '';
        };

        harness-desk = mkDeskjam {
          desk = "harness";
          src = harness-src;
        };
      in
      {
        packages = {
          inherit jammer;
          deskjam = deskjam-desk;
          shelf = shelf-desk;
          hits = hits-desk;
          inherit hits-glob;
          silo = silo-desk;
          inherit silo-glob;
          accountability = accountability-desk;
          harness = harness-desk;
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
