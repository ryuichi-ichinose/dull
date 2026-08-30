# dull build

Build images defined in dull.toml.

## Usage

```console
dull build [options] [--all | --group <name> | images...]
```

## Description

Builds the selected [`[images.X]`](dull.toml.md) entries with
`buildah bud --pull=never`, in the order given by
[`[build_groups.X]`](dull.toml.md) `base` relationships. Successfully built images are recorded in　`.dull/dull_state` with their digest, even if other images in the
same run fail.

Each built image is tagged `localhost/<name>:<local_tag>`, where `<name>` is
the `[images.X]` key and `<local_tag>` is that image's `local_tag` in
`dull.toml` (or `--tag`, for the whole run).

`--pull=never` only stops build from pulling on its own; it does not check
that a `FROM` target was actually resolved through `dependencies` and `dull
lock`/`dull fetch`. If something with that name already sits in project
storage — a leftover from an earlier experiment, a manual `buildah pull`, a
prior build — build happily uses it, pinned or not. For a clean build, wipe
the project's storage first with `dull rmi --all`.

Concurrent invocations against the same project are serialized by a
`.dull/build.lock` file lock.

## Options

- `--all`, `-a` — Build every image defined in `dull.toml`. Conflicts with `images`/`--group`.
- `images` — Image names (`[images.X]` keys) to build. Conflicts with `--group`.
- `--group`, `-g <name>` — Build a named `[build_groups.X]` entry (its `base` plus its
  `images`) instead of individual images. Conflicts with `--all`/`images`.
- `--tag`, `-t <tag>` — Override every built image's tag (default: each image's `local_tag`).
- `--jobs`, `-j <n>` — Max number of images to build concurrently (default: available parallelism).
- `--logs <errors|all>` — After the run, print full per-image logs in addition to the default.
  `errors` dumps only the images that failed; `all` dumps every image, sorted by name.
- `--keep-going` — Don't abort other builds when one fails; run everything to
  completion and report all failures at the end.
- `--ignore-lock` — Pull base images missing from local storage by name, instead of
  requiring them to have been fetched from `dull.lock` first.
