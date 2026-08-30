# dull

Container build orchestrator built on `buildah` and `skopeo`.

## Usage

```console
dull <command> [command options]
```

## Description

`dull` manages a declarative `dull.toml` describing a project's
container images, builds them in dependency order with `buildah`, and
pushes/pulls them with `skopeo`. Every command is a thin wrapper
around a real `buildah`/`skopeo` invocation; `dull` does not
introduce its own build language or execution engine.

`dull` gives each project its own isolated container storage under
`.dull/containers/`, instead of sharing the one rootless storage pool
`buildah`/`podman` otherwise use per user. Run `dull venv` once per
project before any other command that touches images.

Every command's context is `dull.toml` — see [dull.toml](dull.toml.md)
for its full format.

## Files

| Path | Contents |
|---|---|
| `dull.toml` | Images, build args/secrets, dependencies, and registry aliases. Checked into git. |
| `dull.lock` | Digests resolved for `dependencies` by `dull lock`. Checked into git. |
| `.dull/dull_state` | Digests of the last successful `dull build`, per image. Local, not checked in. |
| `.dull/containers/` | This project's isolated buildah/skopeo storage (`storage.conf`, graphroot, tmpdir). Local, not checked in. |

## Commands

| Command | What it does |
|---|---|
| [dull init](dull-init.md) | Create a new dull.toml |
| [dull venv](dull-venv.md) | Set up this project's isolated container storage |
| [dull scan](dull-scan.md) | Discover Containerfiles and add them to dull.toml |
| [dull lock](dull-lock.md) | Resolve dependencies to digests, write dull.lock |
| [dull fetch](dull-fetch.md) | Pull locked dependencies into local storage |
| [dull build](dull-build.md) | Build images defined in dull.toml |
| [dull push](dull-push.md) | Push built images to a registry |
| [dull registry](dull-registry.md) | Manage registry aliases used by push |
| [dull login](dull-login.md) | Log in to a container registry |
| [dull images](dull-images.md) | List tracked and local images |
| [dull export](dull-export.md) | Export built images to an OCI layout on disk |
| [dull rmi](dull-rmi.md) | Remove images from local storage and/or tracking |
| [dull shell](dull-shell.md) | Open a shell configured for this project's storage |