# dull images

List tracked and local images.

## Usage

```console
dull images [options]
```

## Description

Lists images from `.dull/dull_state` (source `State`) and
`dull.lock` (source `Lock`) by default, cross-referenced against
what's actually present in local storage — an entry present in
state/lock but missing from storage is flagged `(Missing)`.

## Options

- `--state` — Show ONLY images from `.dull/dull_state` (built images). Conflicts with `--lock`.
- `--lock` — Show ONLY images from `dull.lock` (fetched base images). Conflicts with `--state`.
- `--all`, `-a` — Also show images present in local storage but not tracked by state or
  lock (source `Untracked`).
- `--json` — Print as JSON instead of a table.
