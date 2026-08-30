# dull.toml

The manifest every `dull` command reads. Checked into git.

## Usage

```console
$ dull init      # create an empty dull.toml
$ dull scan      # or: populate [images] from Containerfiles on disk
```

## Description

`dull.toml` has four top-level tables, all optional:

```toml
dependencies = []

[registries]

[images]

[build_groups]
```

### `dependencies`

A list of base images, given as Fully Qualified Image Names (FQIN) —
a registry, name, and tag or digest. Short names (`fedora:41`) are
rejected. Resolved to digests in `dull.lock` by
[dull lock](dull-lock.md), and pulled into local storage by
[dull fetch](dull-fetch.md).

```toml
dependencies = [
    "docker.io/library/fedora:41",
    "quay.io/fedora/python-312:latest",
    "ghcr.io/astral-sh/uv:latest",
]
```

### `[registries]`

Maps a short alias to a registry URL, managed by
[dull registry](dull-registry.md) (or edited by hand). `dull push -r`
takes one of these aliases.

```toml
[registries.ghcr]
url = "ghcr.io/myorg"
```

### `[images.X]`

One entry per buildable image; `X` becomes the image's name (used as
`dull build X`, the local tag `localhost/X:...`, etc). Populated by
[dull scan](dull-scan.md), or written by hand.

```toml
[images.app]
path = "Dockerfile"       # required: Containerfile path, relative to the project root
context = "app"             # build context, relative to the project root 
local_tag = "latest"      # tag used for the local build (default: "latest")

[images.app.args]         # --build-arg KEY=VALUE, one per entry
VERSION = "1.2.3"

[images.app.secrets]      # --secret id=KEY,..., one per entry; never the value itself
token = { env = "GITHUB_TOKEN" }     # read from an environment variable at build time
cert = { file = "certs/client.pem" } # read from a file, relative to the project root
```

`args` values are baked into the image and its build history — fine
for ordinary build-time flags, wrong for anything sensitive. `secrets`
entries are never written to `dull.toml`, only a pointer to where the
real value lives; consuming them requires the Containerfile to use
`RUN --mount=type=secret,id=<key> ...`.

### `[build_groups.X]`

A named cluster of images, selectable as a unit with `dull build -g
X`. `base`, if given, adds a dependency edge from that image to every
image listed in `images` — this is the *only* mechanism `dull` has
for expressing build order; there's no separate per-image dependency
field.

```toml
[build_groups.app-stack]
base = "app"
images = ["app-worker", "app-migrate"]
```

## Full example

```toml
dependencies = ["docker.io/library/fedora:41", "quay.io/fedora/python-312:latest"]

[registries.ghcr]
url = "ghcr.io/myorg"

[images.app]
path = "app/Dockerfile"
context = "."
local_tag = "latest"

[images.app.args]
VERSION = "1.2.3"

[images.app.secrets]
token = { env = "GITHUB_TOKEN" }
cert = { file = "certs/client.pem" }

[build_groups.app-stack]
base = "app"
images = ["app-worker", "app-migrate"]
```
