# Agent guidance

## Project

`urbit-apps` is a Nix flake that builds Urbit apps' desks into `.deskjam`
files, the upload format of [deskjam](https://github.com/iko-soy/deskjam),
with its glob next to each one, for an app whose frontend is one. `jammer/`
is the Rust CLI that packs them. `README.md` documents the apps for users:
read it before changing what the flake builds.

## Code map

- `flake.nix`: nixpkgs, flake-utils, one input per app, and the commits
  harness' build pins. Each app is an `<app>-desk` built with `mkDeskjam` and
  exposed as `packages.<system>.<app>`. Globs are `packages.<system>.<app>-glob`,
  for hits, silo and vita. jammer is `packages.<system>.jammer` and `default`.
  - `mkDeskjam { desk, src, glob ? null }` packs the desk folder `src` into
    `$out/<desk>.deskjam` and copies `glob` next to it as `$out/<desk>-glob`.
  - `names-glob` reads a `desk.docket-0` with `urbit eval`, as the docket mark
    does. `mkDeskjam` fails the build when the docket names a glob (a
    `glob-http` or `glob-ames` clause, and no `site` clause) but has no
    `glob`, when it has a `glob` for a docket that names none, and when Hoon
    can't read the docket as a list of clauses. It doesn't check that the
    docket has the clauses the mark needs, such as `title`.
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
  only when asked. Never write to `/nix` by hand.
- Nix only sees files jj tracks, and jj tracks a new file at its next
  command. After adding a file, run `jj status` before building, not
  `git add`.
- jammer writes `<desk>.deskjam` or `<desk>/` in the current directory, and jj
  would track them in this repo. Run it elsewhere, or give `jam` and `unjam`
  an `-o` outside the repo; `nix shell .` puts it on your PATH.
- Don't change the app repos from here. When one needs a change, write a
  prompt the user can give an agent in that repo: the problem, with evidence,
  the change, and how to check it.
- Add an app by the first route its repo allows:
  1. Its flake builds the full desk, as `packages.<system>.desk`, or as
     `default` if it has no `desk`, like vita's. Add it as a flake input, with
     `src = app.packages.${system}.desk`, or `.default`. If its docket names a
     glob and its flake builds it, use `glob = app.packages.${system}.glob`;
     if not, build it here with `mkGlob`, as for hits. Have its `nixpkgs`
     follow this flake's only if its flake builds no frontend; otherwise keep
     the nixpkgs it pins, so its glob is the same build as in its own repo.
  2. It has no flake, and its `desk/` is the whole desk: a `flake = false`
     input, with `src = "${app}/desk"`. If its docket names a glob, build it
     with `mkGlob`, starting its `npmDepsHash` at `pkgs.lib.fakeHash`.
  3. Its own build tool assembles the desk: run that build in Nix, with its
     fetches pinned as inputs, as for harness. Don't reimplement its steps.
     Replace only a step that can't run in the sandbox, as `flake.nix` does
     for harness' npm steps, with `--replace-fail`, so a change upstream
     stops the build. Make the build fail loudly when upstream pins
     something new.
- Before adding an app, check:
  - that its `sys.kelvin` lists the current kernel, `[%zuse 408]`. Kernel
    numbers count down, so 411 is older than 408. Don't package an app that
    only supports older kernels.
  - its desk name, from its README or its docket's `base`.
  - whether its docket names a glob, and where else its frontend could come
    from, such as files its own agent serves from the desk.
  - that `jammer jam -v` reports no missing marks, and that deskjam's confirm
    page, on a fake ship, finds nothing missing that its agents and marks
    need. Its warning that other files import files the desk lacks, as for
    silo's base-dev libraries, is fine if nothing uses them.
- The flake supports `x86_64-linux`, `aarch64-linux` and `aarch64-darwin`;
  nixpkgs 26.11 dropped `x86_64-darwin`. An app's flake must provide its
  outputs on all three.
- Keep `README.md` in step with the flake. Give the app its row in the Apps
  table (description, source link,
  `nix build github:iko-soy/urbit-apps#<app>`), and add it wherever the README
  lists apps like it: the glob and build-all commands and their counts, the
  Frontends section, which names every app, with a glob or without, and
  "How the desks are built", including its npm hashes. Search the README for
  a similar app's name to find them all, and keep the lists in alphabetical
  order. Write in the README's voice: short, plain sentences.
- In a README table cell, escape `|` as `\|`, even inside backticks. GitHub
  drops the cells an unescaped one adds, so a rendered table won't show it.
- The README says `~donmec-sallud` publishes every app in its table, and
  [shelf.iko.soy](https://shelf.iko.soy) lists the apps it offers. Only the
  user can publish an app there: when you add one that shelf.iko.soy doesn't
  list, say so in your report.
- Format `flake.nix` with `nix run --inputs-from . nixpkgs#nixfmt -- flake.nix`.
- `nix flake update` moves every input except those pinned by a commit in
  their URL, such as `harness-grubbery` and `harness-tlon`; move those by
  editing the URL.
- When a frontend's `package-lock.json` changes, its build stops with a hash
  mismatch, or `npmDepsHash is out of date`. Set its hash (`npmDepsHash`, or
  the `hash` of harness' `npmDeps`) to `pkgs.lib.fakeHash`, build, and use
  the hash shown after `got:`.

## Validation

Run from the repository root:

```sh
nix run --inputs-from . nixpkgs#nixfmt -- --check flake.nix
nix flake check --all-systems --no-build
nix build --no-link --print-out-paths .#<app>
grep '^|' README.md | sed 's/\\|//g' | awk -F'|' 'NF != 6'
```

The last command prints any README table row with a stray `|`; it should
print nothing.

For a change that shouldn't change a desk, build every app before and after
it, and `cmp` each `.deskjam`. Store paths don't tell: they change whenever
jammer is rebuilt. For a change that should, unpack both with
`jammer unjam OLD.deskjam -o old` and `jammer unjam NEW.deskjam -o new`, and
check that `diff -r old new` shows only the expected changes. To check that a
glob is reproducible, build it, then run `nix build --rebuild` on it a few
times; `--rebuild` fails on a glob that isn't built yet.

For a new app, check that its deskjam has what the app's install steps make.
`|new-desk` adds `mar/hoon`, `mar/kelvin`, `mar/noun`, `mar/txt` and
`sys.kelvin` from `%base` to what you copy in. Make that folder, pack it with
deskjam's `tests/fakezod/pack.sh`, which makes the same bytes as jammer with
`urbit eval`, and `cmp` the two. pack.sh stops on a file jammer skips, such
as hits' `LICENSE`, which has no extension: pack a copy without such files.

To test an app on a ship, clone iko-soy/deskjam outside this repo and use its
fake-ship harness, `tests/fakezod`, which keeps its pier in its
`tests/fakezod/.work`:

- `bootstrap.sh` boots a fake `~zod` with deskjam, and a desk `%harness` whose
  `%logtap` agent copies the ship's output into the log. This repo's harness
  app is also `%harness`, and replacing that desk deletes logtap: move
  `%logtap` to a desk of its own before installing it.
- Set `HTTP_PORT`, and an `AMES_PORT` outside 31337 to 31592. A fake ship
  sends galaxy N's packets to port 31337 + N, so a ship on one of those ports
  loops them back to itself. The harness' default, 31483, is in that range.
- `smoke-test.sh` shows how to drive deskjam's pages with curl. It has no
  glob: for an app with one, post the confirm page's **set** form first, as
  the README's Frontends section says.
- Upload a glob at `/docket/upload` as a browser does: a `desk` part, then one
  `glob` part per file, named `<folder>/<path>`, with its content type.
- Stop the ship with `stop.sh` when done.

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
