# dull fetch

Pull locked dependencies into local storage.

## Usage

```console
dull fetch [options] [images...]
```

## Description

Pulls every dependency pinned in `dull.lock` into this
project's isolated storage, so `dull build` can reference it with
`--pull=never`. Requires `dull lock` to have been run first.

## Options

- `images` — Only fetch these dependencies (default: all).
- `--insecure` — Allow HTTP / untrusted TLS certificates.

Each fetched dependency is printed in canonical `registry/name@digest`
form (no tag) -- usable as-is, e.g. `skopeo inspect docker://<line>`.
