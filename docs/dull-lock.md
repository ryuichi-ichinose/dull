# dull lock

Resolve dependencies to digests, write dull.lock.

## Usage

```console
dull lock [options] [images...]
```

## Description

Resolves each entry in `dull.toml`'s [`dependencies`](dull.toml.md) to
a concrete digest, and writes the result to
`dull.lock`.

Dependencies already present in `dull.lock` are left alone unless
`--upgrade` is given or the specific dependency is named explicitly.
Entries no longer listed in `dull.toml`'s `dependencies` are pruned
from `dull.lock` on every run, regardless of other flags.

The digest recorded is the **manifest list**'s digest (the
multi-platform index), not the digest of any single platform's image
underneath it — [dull fetch](dull-fetch.md) resolves that separately
when it pulls, and the two digests legitimately differ.

After locking, changed entries are printed with the full digest:

- `+ ref locked at manifest-list digest sha256:...` — newly locked
- `~ ref locked at manifest-list digest sha256:... (was sha256:...)` — changed under `--upgrade`
- `- ref removed (was locked at manifest-list digest sha256:...)` — pruned (printed before locking starts)
- `- ref failed` — `skopeo inspect` failed for this dependency

`dull.lock` is still saved for whatever succeeded even if some
dependencies fail to resolve; the command exits non-zero regardless.

## Options

- `images` — Only (re-)lock these dependencies (default: all).
- `--upgrade`, `-u` — Re-resolve digests even for dependencies that are already locked.
- `--insecure` — Allow HTTP / untrusted TLS certificates.
