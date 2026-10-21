# urbit-apps

Urbit apps, built with Nix as deskjam files. An app lives on your ship as a
desk, a folder of its code and files, and a deskjam is a whole desk in one
file. You install it in your ship's web interface with
[deskjam](https://github.com/iko-soy/deskjam), without access to the ship's
files.

## Apps

| App | Description | Source | Build |
|---|---|---|---|
| `%accountability` | Watches all 256 galaxies from your ship. Every 10 minutes it checks each one's keys on chain, whether it answers, and which `%kids` OTA it serves, and logs every change. A dashboard at `/apps/accountability` and a public JSON API show the results. It needs `%azimuth` running, as on a livenet ship. | [sigilante/accountability](https://github.com/sigilante/accountability) | `nix build .#accountability` |
| `%deskjam` | Install and update apps, and upload and download desks, as deskjam files, in your ship's web interface at `/apps/deskjam`. A fork of [tinnus-napbus/deskjam](https://github.com/tinnus-napbus/deskjam). | [iko-soy/deskjam](https://github.com/iko-soy/deskjam) | `nix build .#deskjam` |
| `%harness` | Runs AI agents on your ship. Talk to them in its web app at `/apps/harness`, in Tlon, or from an editor, while the ship keeps the conversations, permissions and work. Models come from providers you configure, such as OpenRouter, OpenAI or Anthropic. Its Tlon features and its documents need `%groups`. Installing it takes a few minutes, while Clay builds it. | [mopfel-winrux/urbit-agent-harness](https://github.com/mopfel-winrux/urbit-agent-harness) | `nix build .#harness` |
| `%hits` | Ranks apps by how many installs they have among the ships around you, which gossip their installs to their Pals and the ships they chat with in Tlon. There's no central server. It works without `%pals`, but then hears only from the ships you chat with. It only counts apps that kiln syncs from another ship, as after `|install`: not ones installed from a deskjam file, or ones you stop kiln syncing. Its frontend is a glob: see [Frontends](#frontends). A fork of [urbit/hits](https://github.com/urbit/hits). | [iko-soy/hits](https://github.com/iko-soy/hits) | `nix build .#hits` |
| `%shelf` | A public web page that lists the apps your ship publishes, so anyone can install them on their own ship. It answers at `/shelf`, for a reverse proxy to serve at the root of its own domain, and needs `%landscape`, which fresh ships have. | [iko-soy/shelf](https://github.com/iko-soy/shelf) | `nix build .#shelf` |
| `%silo` | A file manager for the S3 storage your ship uploads to: add a bucket's credentials, then browse its files and upload new ones. It has no agents of its own, and uses `%storage` from `%landscape`, which fresh ships have. Its frontend is a glob: see [Frontends](#frontends). A fork of [arthyn/silo](https://github.com/arthyn/silo). | [iko-soy/silo](https://github.com/iko-soy/silo) | `nix build .#silo` |
| `%vita` | Tracks how your published apps spread: how many ships sync each of your desks, and the daily active users that apps using its `vita-client` library report, with a dashboard for creating and maintaining apps. Its frontend is a glob: see [Frontends](#frontends). A fork of [assemblycapital/vita](https://github.com/assemblycapital/vita), updated for kernel 408. | [iko-soy/vita](https://github.com/iko-soy/vita) | `nix build .#vita` |

Run the build commands in a checkout of this repo, with Nix and flakes
enabled. If flakes aren't enabled, put
`--extra-experimental-features 'nix-command flakes'` after `nix`. Without a
checkout, give the repo's address instead of `.`, such as
`nix build github:iko-soy/urbit-apps#shelf`. Nix fetches each app's source
itself, at the version `flake.lock` pins.

Each build writes `result/<desk>.deskjam`, such as `result/shelf.deskjam`, in
place of the last build's `result`. For an app whose web frontend is a glob,
such as hits, it also writes the glob's files next to it, as
`result/<desk>-glob`, and the build fails if `flake.nix` doesn't build them,
so a deskjam never comes without its glob. `nix build .#hits-glob`,
`.#silo-glob` or `.#vita-glob` builds just a glob.
`nix build .#accountability .#deskjam .#harness .#hits .#shelf .#silo .#vita`
builds all seven, in that order, into `result` and `result-1` to `result-6`.
The steps below assume one app's `result`.

You can install these files on your ship with
[iko-soy/deskjam](https://github.com/iko-soy/deskjam): see
[Installing with deskjam](#installing-with-deskjam).

The first build also compiles [jammer](#jammer), the tool that packs the
desks. No Hoon is compiled: your ship compiles an app's Hoon when you install
it. The flake supports `x86_64-linux`, `aarch64-linux` and `aarch64-darwin`
(Apple silicon Macs).

## Installing with deskjam

[deskjam](https://github.com/iko-soy/deskjam) installs and updates apps from
these files. Only install apps you trust: an app's agents, its programs, run
as your ship, and can read and change anything on it.

If you build on another machine, such as the server your ship runs on, copy
the file, and its `<desk>-glob` folder if it has one, to the computer your
browser runs on first, for example with `scp -r`.

### Getting deskjam

deskjam has to be on your ship first, and it can't install itself on a ship
that doesn't have it yet. The first time, do one of these:

- If you can reach your ship's files, follow deskjam's
  [Installation](https://github.com/iko-soy/deskjam#installation).
- If another ship publishes it, run `|install ~that-ship %deskjam` in the
  dojo, your ship's command line.
- If your ship runs upstream deskjam,
  [tinnus-napbus/deskjam](https://github.com/tinnus-napbus/deskjam), upload
  `result/deskjam.deskjam` through it, following
  [Replacing upstream deskjam](https://github.com/iko-soy/deskjam#replacing-upstream-deskjam)
  from its step 2. Its pages differ from the steps below. First run
  `+vats %deskjam` in the dojo: if its source ship isn't `~`, run
  `|pause %deskjam`, or that ship's next release replaces this version. When
  you upload, choose `deskjam` under **Stage a desk to merge with**, since
  that list starts on whichever desk comes first.

After that, update deskjam like any other app, as below.

### Installing an app

1. Open `https://your-ship/apps/deskjam`.
2. Under **upload deskjam**, choose the file, such as `result/shelf.deskjam`,
   and click **upload**.
3. Under **Install it as a new desk**, keep the name and click **next**.
4. On the confirm page, if the app's frontend is a glob, first click **set**
   in the glob box, as [Frontends](#frontends) says. Then tick
   **start its agents after uploading** (for an app without agents, such as
   silo, **mark it running after uploading**), and click **confirm**.
5. The status page says whether the app runs.

Once it runs, see the app's README for how to set it up.

### Updating an app

Apps installed this way don't update on their own. To update one:

1. Move its pin to the latest commit of its repo, such as with
   `nix flake update shelf`, and build it again. `nix flake update` moves
   every app, and nixpkgs.
2. Upload the new file as above. Under **Update an existing desk**, choose the
   app's desk, select **replace it**, and click **next**. Replace makes the
   desk exactly the upload, so files a new version removed don't linger.
3. The confirm page offers a download of the desk as it is now. Keep it if
   you might want to go back. If your ship serves the app's glob, click
   **set** in the glob box first. Then click **confirm**.

Uploading the kept copy with **replace it** puts the old version back. If the
update changed the state an agent keeps, that usually fails, and nothing
changes. But some agents start afresh from state they can't read, and lose it
instead: deskjam's does. If your ship serves the app's glob, also copy its old
`result/<desk>-glob` somewhere before you build the new version, and upload it
at `/docket/upload` after putting the old version back.

If the app came from another ship with `|install`, kiln, the part of your
ship that installs and updates apps, keeps it in sync with that ship. That
ship's next release would undo your upload. Tick **stop kiln syncing** on the
confirm page to keep yours. That stops all of that ship's updates to the
desk, until you run `|install ~that-ship %desk` again, which replaces your
upload with their latest release.

### Frontends

Some apps' web frontends are globs, which aren't part of their desks.
Landscape's `%docket` agent gets an app's glob as its docket says, and the
build puts the glob's files next to the deskjam, as `result/<desk>-glob`. That
folder is what you upload at `/docket/upload`: choose the app's desk and the
folder, and click **glob!**. Use a browser other than Safari, which can't
upload folders.

hits', silo's and vita's dockets have `%docket` get their globs from
`~donmec-sallud` over Ames, with a placeholder hash. On your ship:

1. When you install the deskjam, click **set** in the glob box on the confirm
   page first, so your ship serves the glob instead. **set** reloads the page
   and clears the boxes below it, so tick **start its agents after uploading**
   after it, then confirm.
2. Once the app runs, upload its `result/<desk>-glob`.

On `~donmec-sallud`, their dockets already name your ship, so skip **set**,
and just upload the glob. Without its glob, an app runs, but has no frontend.
deskjam's
[Serving a glob from your ship](https://github.com/iko-soy/deskjam/blob/main/docs/globs.md#serving-a-glob-from-your-ship)
has the details.

When you update an app whose glob your ship serves, click **set** in the glob
box on the confirm page first, which keeps the glob your ship serves now.
Then upload the new `result/<desk>-glob`, so the frontend matches the new
version. If the app was suspended, as after a kernel update, `%docket`
forgets its glob when the app starts again, even after **set**: upload it
again.

To let other ships install an app from yours, frontend included, publish it:
`:treaty|publish %hits` makes it public and lists it in Landscape, and
`|public %hits` is enough for `|install ~your-ship %hits`.

The other apps have no glob: accountability, deskjam, harness and shelf
serve their pages from their own agents. harness' frontend is in its desk,
so `harness.deskjam` already has it.

### Kernel updates

When Urbit releases a new kernel, your ship suspends every app whose
`sys.kelvin` doesn't list it, and a suspended deskjam can't upload its own
update. So as soon as deskjam's `desk/sys.kelvin` lists the new kernel,
update deskjam as above, before your ship upgrades. Do the same for the other
apps, or they stay suspended until you do. See deskjam's
[Kernel updates](https://github.com/iko-soy/deskjam#kernel-updates).

deskjam's
[Deploying an app](https://github.com/iko-soy/deskjam/blob/main/docs/deploying.md)
has the details, including what to do when an upload goes wrong.

## How the desks are built

- deskjam's, hits', shelf's, silo's and vita's own flakes build their desks,
  so each `.deskjam` file has the files that repo's own build makes. jammer
  leaves out the files a `|commit` would skip too, such as hits' `LICENSE`,
  which has no extension. silo's and vita's flakes build their globs too, and
  this flake takes them as they are, with the nixpkgs those flakes pin: so
  `.#silo-glob` is the same build as `github:iko-soy/silo#glob`. Their desks
  include all of urbit's base-dev, so deskjam's confirm page warns about a
  few of its libraries that import files the desks lack, but the apps don't
  use them.
- accountability has no flake, and its `desk/` is the whole desk, which
  `flake.nix` packs as it is.
- harness' repo builds its desk with `zig build`. It builds `fe/`, its
  frontend, into the desk's `web/`, which harness' own agent serves; fetches
  grubbery and some of Tlon's files with git, at commits it pins; and patches
  grubbery's desk under harness' own. `flake.nix` runs the same `zig build`
  with npm's packages fetched ahead, and a stand-in `git` that answers from
  inputs pinned to the same commits, and fails if the build asks for any
  other. When a new version of harness pins other commits, change the commit
  in `harness-grubbery`'s or `harness-tlon`'s url in `flake.nix` to the one
  the error names: `nix flake update` won't move them. The build also fails
  if the desk has no `web/index.html`, so it can't come out without its
  frontend. Two of grubbery's libraries in it, `lib/verb.hoon` and
  `lib/xray.hoon`, import files the desk lacks, so deskjam's confirm page
  warns about them, but harness doesn't use them.
- A desk whose docket names a glob only builds with its glob, and a glob for
  a desk whose docket names none fails the build too. `flake.nix` reads each
  docket as the docket mark does, with Hoon's own parser in `urbit eval`, so
  this holds however the docket is laid out, and a docket that isn't valid
  fails the build.
- A hash in `flake.nix` pins the npm packages of the frontends it builds
  itself: `hits-glob`'s `npmDepsHash`, and the `hash` of harness' `npmDeps`.
  silo's and vita's are in their own flakes. When moving an app's pin changes
  its `package-lock.json`, its build stops with a hash mismatch, or
  `npmDepsHash is out of date`. Set that hash to `pkgs.lib.fakeHash`, build
  again, and put the hash the error shows after `got:` in its place.

## jammer

[`jammer/`](jammer/README.md) is the command-line tool that packs each desk.
It packs any desk directory into a deskjam, and unpacks deskjams, without a
ship:

```console
$ nix run . -- jam path/to/desk -d my-app     # writes my-app.deskjam
$ nix run . -- unjam result/shelf.deskjam     # writes ./shelf/
$ nix run . -- list result/shelf.deskjam
```

These write into the current directory. To keep their output out of this
repo, run jammer elsewhere: `nix shell .` puts it on your PATH. `nix develop`
gives you a shell with the Rust toolchain for working on it.
