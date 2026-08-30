# dull rmi

Remove images from local storage and/or tracking.

## Usage

```console
dull rmi [--all | --keep-tracked | --keep-state | --keep-lock | images...]
```

## Description

Operates in one of three mutually exclusive modes:

- `images` — remove specific images by name from `.dull/dull_state`
  and from local storage. A name with no state entry is skipped with
  a warning.
- `--all` — wipe every image in this project's storage and reset
  `.dull/dull_state` entirely.
- `--keep-tracked`/`--keep-state`/`--keep-lock` — garbage collect:
  remove everything in local storage *not* referenced by
  `.dull/dull_state` and/or `dull.lock`, per which of the three is
  given. After collection, any state entry whose image no longer
  exists in storage is pruned from `.dull/dull_state` and the pruned
  count is reported, so state never lies about what's actually built.

## Options

- `images` — Image names (`[images.X]` keys) to remove.
- `--all` — Wipe all images in project storage and reset `.dull/dull_state`.
- `--keep-tracked` — Keep everything tracked by state or lock; remove the rest.
- `--keep-state` — Keep images tracked by state (built images).
- `--keep-lock` — Keep images tracked by lock (fetched base images).
