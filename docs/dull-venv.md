# dull venv

Set up this project's isolated container storage.

## Usage

```console
dull venv [options]
```

## Description

Creates `.dull/containers/storage.conf` and the backing storage
directory for the current project. Every other command that touches
images (`build`, `push`, `lock`, `fetch`, `rmi`, `images`, `shell`) requires
this to exist first, and fails with a pointer back to `dull venv`
otherwise.

By default this storage is project-local and separate from the host's
own rootless `buildah`/`podman` storage, using the `overlay` driver
with `mountopt=nodev` (deliberately without `metacopy=on`, which
breaks Native Overlay Diff under rootless).

If `.dull/` already exists, `dull venv` does nothing.

## Options

- `--global` — Use the host's global/default container storage instead of an
  isolated project-local one (writes an empty `storage.conf`).
  Conflicts with every other flag below.
- `--driver <driver>` — `storage.driver`. Default: `overlay`.
- `--mountopt <opts>` — `storage.options.overlay.mountopt`. Default: `nodev`.
- `--composefs <true|false>` — `storage.options.overlay.use_composefs`. Default: `false`.
- `--size <size>` — `storage.options.overlay.size` — max size of a read/write layer (e.g. `10G`).
- `--inodes <count>` — `storage.options.overlay.inodes` — max inodes in a read/write layer.
- `--convert-images <true|false>` — `storage.options.pull_options.convert_images`. Default: `true`.
- `--enable-partial-images <true|false>` — `storage.options.pull_options.enable_partial_images`. Default: `true`.
- `--use-hard-links <true|false>` — `storage.options.pull_options.use_hard_links`. Default: `true`.
