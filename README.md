# urbit-apps

Urbit apps, built with Nix as deskjam files. An app lives on your ship as a
desk, a folder of its code and files, and a deskjam is a whole desk in one
file. You install it in your ship's web interface with
[deskjam](https://github.com/iko-soy/deskjam), without access to the ship's
files.

## Apps

| App | Description | Source | Build |
|---|---|---|---|
| `%deskjam` | Install and update apps, and upload and download desks, as deskjam files, in your ship's web interface at `/apps/deskjam`. A fork of [tinnus-napbus/deskjam](https://github.com/tinnus-napbus/deskjam). | [iko-soy/deskjam](https://github.com/iko-soy/deskjam) | `nix build .#deskjam` |
| `%hits` | Ranks apps by how many installs they have among the ships around you, which gossip their installs to their Pals and the ships they chat with in Tlon. There's no central server. Its frontend is a glob: see [Frontends](#frontends). A fork of [urbit/hits](https://github.com/urbit/hits). | [iko-soy/hits](https://github.com/iko-soy/hits) | `nix build .#hits` |
| `%mcp` | An MCP (Model Context Protocol) server at `/mcp` that lets AI tools use your ship, and a proxy at `/apps/mcp/mcp` that offers its tools together with those of remote MCP and OpenAPI servers. A fork of [gwbtc/urbit-mcp](https://github.com/gwbtc/urbit-mcp). | [tloncorp/mcp](https://github.com/tloncorp/mcp) | `nix build .#mcp` |
| `%shelf` | A public web page that lists the apps your ship publishes, so anyone can install them on their own ship. It answers at `/shelf`, for a reverse proxy to serve at the root of its own domain, and needs `%landscape`, which fresh ships have. | [iko-soy/shelf](https://github.com/iko-soy/shelf) | `nix build .#shelf` |
| `%silo` | A file manager for the S3 storage your ship uploads to: add a bucket's credentials, then browse its files and upload new ones. It has no agents of its own, and uses `%storage` from `%landscape`, which fresh ships have. | [arthyn/silo](https://github.com/arthyn/silo) | `nix build .#silo` |

Run the build commands in a checkout of this repo, with Nix and flakes
enabled. If flakes aren't enabled, put
`--extra-experimental-features 'nix-command flakes'` after `nix`. Without a
checkout, give the repo's address instead of `.`, such as
`nix build github:iko-soy/urbit-apps#shelf`. Nix fetches each app's source
itself, at the version `flake.lock` pins.

Each build writes `result/<desk>.deskjam`, such as `result/shelf.deskjam`, in
place of the last build's `result`. For an app with a web frontend, a glob,
it also writes the glob's files next to it, as `result/<desk>-glob`, and the
build fails if `flake.nix` doesn't build them, so a deskjam never comes
without its glob. `nix build .#hits-glob` or `.#silo-glob` builds just a glob.
`nix build .#deskjam .#hits .#mcp .#shelf .#silo` builds all five, into
`result`, `result-1`, `result-2`, `result-3` and `result-4`.

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
the file to the computer your browser runs on first, for example with `scp`.

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
4. On the confirm page, tick **start its agents after uploading** (for an app
   without agents, such as silo, **mark it running after uploading**), and
   click **confirm**.
5. The status page says whether the app runs.

Once it runs, see the app's README for how to set it up. mcp's README
describes an operator console at `/apps/mcp/`, which this version of mcp
doesn't have. MCP clients send mcp's API key in an `X-Api-Key` header, and
`.^(@t %gx /=mcp-proxy=/client-key/noun)` in the dojo prints it.

### Updating an app

Apps installed this way don't update on their own. To update one:

1. Move its pin to the latest commit of its repo, such as with
   `nix flake update shelf`, and build it again. `nix flake update` moves
   every app, and nixpkgs.
2. Upload the new file as above. Under **Update an existing desk**, choose the
   app's desk, select **replace it**, and click **next**. Replace makes the
   desk exactly the upload, so files a new version removed don't linger.
3. The confirm page offers a download of the desk as it is now. Keep it if
   you might want to go back, then click **confirm**.

Uploading the kept copy with **replace it** puts the old version back. If the
update changed the state an agent keeps, that usually fails, and nothing
changes. But some agents start afresh from state they can't read, and lose it
instead: deskjam's and mcp's do. For mcp, that means a new API key, and its
remote servers and OAuth grants are gone.

If the app came from another ship with `|install`, kiln, the part of your
ship that installs and updates apps, keeps it in sync with that ship. That
ship's next release would undo your upload. Tick **stop kiln syncing** on the
confirm page to keep yours. That stops all of that ship's updates to the
desk, until you run `|install ~that-ship %desk` again, which replaces your
upload with their latest release.

### Frontends

An app's web frontend, its glob, isn't part of its desk. Landscape's
`%docket` agent gets it as the app's docket says, and the build puts it next
to the deskjam, as `result/<desk>-glob`:

- silo's docket has `%docket` download its glob from a url, so `silo.deskjam`
  works on any ship. You need `result/silo-glob` only to serve the frontend
  from your own ship instead, as deskjam's
  [Serving a glob from your ship](https://github.com/iko-soy/deskjam/blob/main/docs/globs.md#serving-a-glob-from-your-ship)
  says.
- hits' docket has `%docket` get its glob from `~donmec-sallud` over Ames,
  with a placeholder hash, so `hits.deskjam` is for that ship. There, install
  it, then open `/docket/upload`, choose the desk `hits` and the folder
  `result/hits-glob`, and click **glob!**. When you update hits with deskjam,
  click **set** in the glob box on the confirm page first, or hits loses its
  frontend until you upload it again. Other ships get hits, frontend
  included, from that ship once it publishes hits:
  `|install ~donmec-sallud %hits`.

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

- deskjam's, hits' and shelf's own flakes build their desks, so each
  `.deskjam` file has the files that repo's own build makes. jammer leaves
  out the files a `|commit` would skip too, such as hits' `LICENSE`, which has
  no extension.
- mcp's repo builds its desk with `zig build`, which copies some files from
  other repos at pinned commits. `flake.nix` copies the same files from the
  same commits. After moving mcp's pin, check that its `build.zig` still lists
  the same files and commits, and change `flake.nix` to match if not: nothing
  checks this for you.
- silo has no flake. Its CI deploys all of urbit's `pkg/base-dev`, then
  landscape's `desk-dev`, then its own `desk/`, and `flake.nix` builds the
  same, with landscape at `master`, as the CI takes it, and urbit at
  `408k-2`, since the CI's tag isn't public. After moving silo's pin, check
  its `.github/helpers/deploy.sh`.
- A desk whose docket names a glob only builds with its glob, which
  `flake.nix` builds from the app's `ui/`. A glob for a desk whose docket
  names none fails the build too.

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
