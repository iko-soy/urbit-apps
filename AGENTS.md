# Agent guidance

## Project

`urbit-apps` is a Nix flake that builds Urbit apps' desks into `.deskjam`
files, the upload format of [deskjam](https://github.com/iko-soy/deskjam),
with each app's frontend glob next to it. `jammer/` is the Rust CLI that packs
them. `README.md` documents the apps for users: read it before changing what
the flake builds.

## Code map

- `flake.nix`: one input per app, plus any dependencies an app's own build
  pins. Each app is an `<app>-desk` built with `mkDeskjam` and exposed as
  `packages.<system>.<app>`; frontends are `packages.<system>.<app>-glob`.
  - `mkDeskjam { desk, src, glob ? null }` packs the desk folder `src` into
    `$out/<desk>.deskjam` and copies `glob` next to it as `$out/<desk>-glob`.
  - `names-glob` reads a `desk.docket-0` with `urbit eval`, as the docket mark
    does. `mkDeskjam` fails the build when the docket names a glob (a
    `glob-http` or `glob-ames` clause, and no `site` clause) but has no
    `glob`, when it has a `glob` for a docket that names none, and when the
    docket isn't valid Hoon.
  - `mkGlob` builds a frontend with `buildNpmPackage` (`npm run build`, then
    its `dist/`), for apps whose own flakes don't build their globs.
  - `harness-git` and `harness-src` run harness' own `zig build` offline; see
    the README's "How the desks are built".
- `flake.lock`: the revision of every app and dependency.
- `jammer/`: the packer. Its build takes only `Cargo.toml`, `Cargo.lock`,
  `src/` and `tests/`, and runs its tests.
- `README.md`: the apps table, installing and updating with deskjam,
  frontends, and how each desk is built.

## Working conventions

- Use jj, not git, and pass `--git` to `jj diff` and `jj show`. Commit or push
  only when asked. Nix only sees files jj tracks. Never write to `/nix` by
  hand.
- Don't change the app repos from here. When one needs a change, write a
  prompt the user can give an agent in that repo: the problem, with evidence,
  the change, and how to check it.
- Add an app by the first route its repo allows:
  1. Its flake follows the convention: `packages.<system>.desk` is the full
     desk, `.glob` is the built frontend if its docket names a glob, and
     `default` is `desk`. Add it as a flake input, with
     `src = app.packages.${system}.desk` and
     `glob = app.packages.${system}.glob`. Have its `nixpkgs` follow this
     flake's only if its flake builds no frontend; otherwise keep the nixpkgs
     it pins, so its glob is the same build as in its own repo.
  2. It has no flake, and its `desk/` is the whole desk: a `flake = false`
     input, with `src = "${app}/desk"`.
  3. Its own build tool assembles the desk: run that build in Nix, with its
     fetches pinned as inputs, as for harness. Don't reimplement its steps,
     and make the build fail loudly when upstream pins something new.
- Before adding an app, check:
  - that its `sys.kelvin` lists the current kernel, `[%zuse 408]`. Kernel
    numbers count down, so 411 is older than 408. Don't package an app that
    only supports older kernels.
  - its desk name, from its README or its docket's `base`.
  - whether its docket names a glob, and where else its frontend could come
    from, such as files its own agent serves from the desk.
  - that `jammer jam -v` reports no missing marks, and that the desk has the
    libraries its Hoon imports.
- The flake supports `x86_64-linux`, `aarch64-linux` and `aarch64-darwin`;
  nixpkgs 26.11 dropped `x86_64-darwin`. An app's flake must provide its
  outputs on all three.
- Keep `README.md` in step with the flake: the app's row in the Apps table
  (description, source link, `nix build github:iko-soy/urbit-apps#<app>`), the
  glob and build-all commands, the Frontends section for an app with a glob,
  and its bullet in "How the desks are built". Write in its voice: short,
  plain sentences. In a table cell, escape `|` as `\|`, even inside backticks.
- Format `flake.nix` with `nix run --inputs-from . nixpkgs#nixfmt -- flake.nix`.
- `nix flake update` moves every input except those pinned by a commit in
  their URL, such as `harness-grubbery` and `harness-tlon`; move those by
  editing the URL.
- When a frontend's `package-lock.json` changes, its build stops on a hash
  mismatch. Set its hash (`npmDepsHash`, or the `hash` of harness' `npmDeps`)
  to `pkgs.lib.fakeHash`, build, and use the hash shown after `got:`.

## Validation

Run from the repository root:

```sh
nix run --inputs-from . nixpkgs#nixfmt -- --check flake.nix
nix flake check --all-systems --no-build
nix build --no-link --print-out-paths .#<app>
```

For a change that shouldn't change a desk, `cmp` each app's new `.deskjam`
with its old one. For one that should, `jammer unjam` both, `diff -r` them,
and check that only the expected files changed. `nix build --rebuild` of a
glob checks that it's reproducible.

For a new app, also compare its deskjam with what the app's own procedure
makes. deskjam's `tests/fakezod/pack.sh` packs a desk folder with
`urbit eval`, byte for byte as jammer does. Then render the README with
`nix run --inputs-from . nixpkgs#pandoc -- -f gfm -t html README.md`, and
check that every table row has four cells.

To test an app on a ship, use deskjam's fake-ship harness, `tests/fakezod` in
iko-soy/deskjam. `bootstrap.sh` boots a fake `~zod` with deskjam, and
`smoke-test.sh` shows how to drive deskjam's pages with curl. Set `HTTP_PORT`
and `AMES_PORT`, and don't pick 31337 plus a galaxy's number as the Ames
port: the fake ship sends that galaxy's packets there, so they loop back.
Upload a glob at `/docket/upload` as a browser does: a `desk` part, then one
`glob` part per file, named `<folder>/<path>`, with its content type. Stop
the ship with `stop.sh` when done.

Quirks:

- `urbit eval` colors its output, ends lines with CRLF, and exits 0 even when
  the Hoon crashes. Check its output, not its exit status.
- In the build sandbox, npm packages' `#!/usr/bin/env node` scripts only run
  after `patchShebangs`, which `npmHooks.npmConfigHook` does once. Another
  `npm ci` puts the unpatched scripts back.
- `nix build` of several targets writes `result`, then `result-1` and so on.
  Nix reuses a `github:` flake it fetched within the last hour, unless given
  `--refresh`.

In the final report, summarize the change, the checks run, and anything left
unchecked.
