# dull scan

Discover Containerfiles and add them to dull.toml.

## Usage

```console
dull scan [options] [directories...]
```

## Description

Walks the project (respecting `.gitignore`) looking for files named
`Dockerfile`, `Containerfile`, `*.Dockerfile`, `*.Containerfile`,
`Dockerfile.*`, or `Containerfile.*`, and adds an
[`[images.X]`](dull.toml.md) entry to `dull.toml` for each one found.

The image name is derived from the file's directory, relative to the
project root (slashes preserved, e.g. `services/api/Containerfile`
becomes `services/api`); the root Containerfile is named after the
project directory itself. A `Dockerfile.<suffix>`/`<prefix>.Dockerfile`
file appends that suffix/prefix to the derived name.

Existing `[images.X]` entries are matched by the Containerfile's
*path*, not by name — renaming an entry by hand in `dull.toml` and
rescanning does not re-add it under the scanned name. Rescanning
fails if a scanned name would collide with an existing entry
that points at a different file.

Entries whose Containerfile no longer exists on disk are removed.

`scan` is a convenience for keeping `dull.toml` in sync with the directory
layout as a project grows; it writes nothing you could not write yourself.
Every `[images.X]` entry is plain TOML (see [dull.toml](dull.toml.md)), and
adding or editing one by hand is just as valid as scanning for it.

## Options

- `directories` — Restrict the scan to these directories (default: the whole project).
- `--context <path>` — Override the build context for every image found this run
  (default: each Containerfile's own parent directory).
