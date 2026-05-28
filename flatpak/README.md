# Flatpak and Flathub Packaging

This directory contains the upstream packaging files for the Flathub-style Goose Flatpak.

Files in this workflow:

- `io.github.block.Goose.yaml`: `flatpak-builder` manifest used for Flathub submission and local validation
- `flatpak/io.github.block.Goose.desktop`: desktop entry installed by the manifest
- `flatpak/io.github.block.Goose.metainfo.xml`: AppStream metadata
- `flatpak/goose-desktop.sh`: launcher wrapper used inside the sandbox
- `flatpak/cargo-sources.json`: offline Cargo dependency sources generated from `Cargo.lock`
- `flatpak/generated-sources.json`: offline pnpm dependency sources generated from `ui/pnpm-lock.yaml`

## Important distinction

This repo currently has two different Flatpak-related build paths.

1. The long-standing GitHub Actions release workflow builds a `.flatpak` artifact with Electron Forge from `ui/desktop/forge.config.ts`.
2. The files in this directory plus `io.github.block.Goose.yaml` are the separate `flatpak-builder` inputs intended for Flathub submission.

This README is about the second workflow.

## What the manifest does

The manifest in `io.github.block.Goose.yaml` does not reuse the Electron Forge Flatpak maker.

Instead it:

1. Builds `goose-server` from source inside the Flatpak SDK.
2. Copies the resulting `goosed` binary into `ui/desktop/src/bin/goosed`.
3. Installs the `ui/` pnpm workspace offline from `flatpak/generated-sources.json`.
4. Runs `electron-forge package --platform=linux` inside the Flatpak build sandbox.
5. Copies the packaged app into `/app/lib/goose`.
6. Installs the desktop file, metainfo, icons, and wrapper script.

Runtime behavior also differs from the Electron Forge `.flatpak` artifact:

- the Flathub manifest grants `--filesystem=home`
- it grants `--talk-name=org.freedesktop.Flatpak`
- Goose detects `/.flatpak-info` and disables its built-in updater in Flatpak installs
- the wrapper uses `zypak-wrapper` to launch Electron in the sandbox

## Local validation

### Baseline host tools

The most portable local path is to use host `flatpak` and `flatpak-builder` directly.

Required tools:

- `flatpak`
- `flatpak-builder`
- `appstreamcli`
- `desktop-file-validate`

You also need the Flathub remote configured:

```bash
flatpak remote-add --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
```

Build the manifest locally:

```bash
flatpak-builder \
  --user \
  --install \
  --install-deps-from=flathub \
  --force-clean \
  builddir \
  io.github.block.Goose.yaml
```

Run the installed app:

```bash
flatpak run io.github.block.Goose
```

If you want an exported local repo for linting or bundle inspection:

```bash
flatpak-builder \
  --user \
  --install-deps-from=flathub \
  --force-clean \
  --repo=repo \
  builddir \
  io.github.block.Goose.yaml
```

### Using `org.flatpak.Builder`

If you already use the Flathub helper app, the equivalent commands also work through `org.flatpak.Builder`:

```bash
flatpak install -y flathub org.flatpak.Builder
flatpak run --command=flathub-build org.flatpak.Builder --install io.github.block.Goose.yaml
flatpak run io.github.block.Goose
```

## Validation commands

Validate AppStream and desktop metadata:

```bash
appstreamcli validate flatpak/io.github.block.Goose.metainfo.xml
desktop-file-validate flatpak/io.github.block.Goose.desktop
```

If you have `org.flatpak.Builder` installed, run the Flathub linter too:

```bash
flatpak run --command=flatpak-builder-lint org.flatpak.Builder manifest io.github.block.Goose.yaml
flatpak run --command=flatpak-builder-lint org.flatpak.Builder repo repo
```

The current manifest is expected to trigger these lints:

- `finish-args-home-filesystem-access`
- `finish-args-flatpak-spawn-access`
- `appid-url-not-reachable`

Those are expected for the current packaging model:

- `--filesystem=home` is intentional because Goose works against real project directories
- `--talk-name=org.freedesktop.Flatpak` is intentional because Goose needs `flatpak-spawn --host` for host-side shell execution
- `appid-url-not-reachable` currently points at the legacy app ID owner URL and should be fixed before or during Flathub review

## When to regenerate generated sources

Regenerate the dependency manifests whenever any of these change:

- `Cargo.lock`
- `ui/pnpm-lock.yaml`
- manifest build steps that change how dependencies are resolved offline

Do not hand-edit `flatpak/cargo-sources.json` or `flatpak/generated-sources.json`.

## Regenerating Cargo sources

Use the upstream `flatpak-builder-tools` cargo generator.

One reproducible approach:

```bash
git clone https://github.com/flatpak/flatpak-builder-tools.git /tmp/flatpak-builder-tools
python3 -m venv /tmp/flatpak-builder-tools-cargo-venv
. /tmp/flatpak-builder-tools-cargo-venv/bin/activate
python3 -m pip install -r /tmp/flatpak-builder-tools/cargo/requirements.txt
python3 /tmp/flatpak-builder-tools/cargo/flatpak-cargo-generator.py \
  --git-tarballs \
  Cargo.lock \
  -o flatpak/cargo-sources.json
```

If the generator instructions change upstream, prefer upstream over this README.

## Regenerating pnpm sources

Use the upstream node generator from `flatpak-builder-tools`.

One reproducible approach with `pipx`:

```bash
pipx install git+https://github.com/flatpak/flatpak-builder-tools.git#subdirectory=node
flatpak-node-generator pnpm ui/pnpm-lock.yaml -o flatpak/generated-sources.json
```

If `pipx` is not available, install the node generator from the same upstream repo in another isolated Python environment.

## Updating the manifest for a new Goose release

At minimum, update:

1. `io.github.block.Goose.yaml`
2. `flatpak/io.github.block.Goose.metainfo.xml`
3. `flatpak/cargo-sources.json` if `Cargo.lock` changed
4. `flatpak/generated-sources.json` if `ui/pnpm-lock.yaml` changed

Checklist:

1. Update the source tarball URL and SHA256 in `io.github.block.Goose.yaml`.
2. Update release entries in `flatpak/io.github.block.Goose.metainfo.xml`.
3. Regenerate offline dependency manifests if locks changed.
4. Rebuild locally with `flatpak-builder`.
5. Re-run `appstreamcli`, `desktop-file-validate`, and the Flathub linter.

## Flathub submission flow

These files are intended to be copied into a new app directory in `flathub/flathub`.

Typical flow:

1. Merge the upstream packaging files into Goose.
2. Cut a stable Goose release that matches the tarball referenced by `io.github.block.Goose.yaml`.
3. Fork `flathub/flathub`.
4. Create a branch from `new-pr`.
5. Create `io.github.block.Goose/` in that branch.
6. Copy `io.github.block.Goose.yaml`, `flatpak/cargo-sources.json`, and `flatpak/generated-sources.json` into that directory.
7. Open a PR to `flathub/flathub:new-pr` titled `Add io.github.block.Goose`.

Expect review questions about permissions.

## Review notes for Flathub

Goose is a developer tool, so the current manifest intentionally requests broad access compared with a typical consumer desktop app.

The two permissions reviewers will ask about first are:

- `--filesystem=home`
- `--talk-name=org.freedesktop.Flatpak`

Current justification:

- Goose needs direct access to local project directories selected by the user.
- Goose uses `flatpak-spawn --host` to execute host-side shell commands when running sandboxed.

If the app security model changes later, revisit these permissions instead of cargo-culting them forward.
