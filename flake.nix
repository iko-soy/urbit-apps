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

    # Its packages.<system>.desk is the full desk: it vendors its lib/,
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

    # silo's and vita's flakes build their desks and their globs, with the
    # nixpkgs they pin, so both come out as their own repos build them.
    silo.url = "github:iko-soy/silo";
    vita.url = "github:iko-soy/vita";

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
      vita,
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

        # Reads the docket file it's given as the docket mark does, with
        # `ream` and `slap` in `urbit eval`, so any layout Hoon accepts reads
        # the same. Exits 0 if it names a glob: a glob-http or glob-ames
        # clause, and no site clause, which lib/docket prefers. Exits 1 if
        # it doesn't, and 2 if it isn't a valid docket.
        names-glob = pkgs.writeShellApplication {
          name = "names-glob";
          runtimeInputs = [
            pkgs.urbit
            pkgs.util-linux
          ];
          excludeShellChecks = [ "SC2016" ];
          text = ''
            # the file's bytes as a Hoon @ux literal: an atom, first byte
            # lowest, with a dot between each group of four hex digits
            hex=$(od -An -v -tx1 -w1 "$1" | tac | tr -d ' \n' | sed 's/^0*//')
            atom=0x$(printf '%s' "''${hex:-0}" | rev | sed -E 's/([0-9a-f]{4})/\1./g; s/\.$//' | rev)
            out=$(
              {
                printf '=/  tex  `@t`%s\n' "$atom"
                echo '=/  cls  ;;((list [@tas *]) q:(slap !>(~) (ream tex)))'
                echo '=/  has  |=(t=@tas (lien cls |=([h=@tas *] =(h t))))'
                echo '?&(!(has %site) |((has %glob-http) (has %glob-ames)))'
              } | urbit eval 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | tr -d '\r'
            )
            case $(printf '%s\n' "$out" | tail -n 1) in
              '%.y') exit 0 ;;
              '%.n') exit 1 ;;
              *)
                echo "$1 isn't a valid docket: Hoon can't read it as a list of clauses" >&2
                printf '%s\n' "$out" >&2
                exit 2
                ;;
            esac
          '';
        };

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
              names=
              if [ -f ${desk}/desk.docket-0 ]; then
                if names-glob ${desk}/desk.docket-0; then
                  names=yes
                elif [ $? != 1 ]; then
                  exit 1
                fi
              fi
              if [ -n "$names" ] && [ -z "$glob" ]; then
                echo "The docket of %${desk} names a glob: give mkDeskjam the built frontend as glob" >&2
                exit 1
              elif [ -z "$names" ] && [ -n "$glob" ]; then
                echo "%${desk} has no docket that names a glob, but mkDeskjam got one" >&2
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
          src = deskjam.packages.${system}.desk;
        };

        shelf-desk = mkDeskjam {
          desk = "shelf";
          src = shelf.packages.${system}.desk;
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

        silo-desk = mkDeskjam {
          desk = "silo";
          src = silo.packages.${system}.desk;
          glob = silo.packages.${system}.glob;
        };

        vita-desk = mkDeskjam {
          desk = "vita";
          src = vita.packages.${system}.default;
          glob = vita.packages.${system}.glob;
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
            checkout | fetch) for arg; do :; done; pinned "$arg" ;;
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
            runHook preInstall
            if [ ! -f zig-out/web/index.html ]; then
              echo "harness' desk has no web/index.html: its frontend is missing" >&2
              exit 1
            fi
            cp -r zig-out $out
            runHook postInstall
          '';
          # the output is a desk: leave its files as they are
          dontFixup = true;
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
          silo-glob = silo.packages.${system}.glob;
          vita = vita-desk;
          vita-glob = vita.packages.${system}.glob;
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
