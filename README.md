# dull

`dull` is a container build orchestrator built on top of `buildah` and
`skopeo`. It manages a declarative `dull.toml` describing a project's
images, builds them in dependency order, and keeps each project's
container storage isolated from every other project on the machine.

`dull` does not introduce a new build language — most commands are thin wrappers around real `buildah` or `skopeo` invocations.

## Why isolated storage

Rootless `buildah`/`podman` share one container storage per user
(`~/.local/share/containers/storage`) by default. Working on more than
one project means their images, tags, and cleanup all collide in that
same pool. `dull venv` gives each project its own storage under
`.dull/containers/`, with its own `storage.conf`.

If the storage is corrupted or in a conflicting state, don't try to repair it: remove it and start over.

```console
$ buildah unshare rm -rf .dull
```

Check for leftover locks or mounts under `.dull/containers/storage` first; removing it while something still holds them can leave the mount namespace in a bad state. 
Then run `dull venv` again.

## Let the directory layout decide image names

`dull scan` writes the images it finds into `dull.toml`, the project's configuration file. Each image is named from its path, and its build context is the directory holding its Containerfile:

```
your_project/
├── os/Containerfile           -> os
└── app/
    ├── app1/Containerfile     -> app/app1
    └── app2/Containerfile     -> app/app2
```

The file system decides the name.
The name in `dull.toml` is used as-is in both places: `dull build` tags the image `localhost/<name>:<tag>`, and `dull push` pushes it to `<registry>/<name>:<tag>`. You never rename an image to fit a registry.

`dull scan` exists to make writing `dull.toml` rational, not to be the only way to write it. Every `[images.X]` entry is plain TOML; add, edit, or remove one by hand whenever that is more direct than rescanning.

## Working inside the project storage

`dull shell` starts your `$SHELL` with the environment set to this project's
isolated storage. Inside it, `buildah`, `skopeo`, and `podman` work against
that storage, not the shared one.

`dull` is a build tool, so `dull shell` is meant for testing the built results.

## How the steps connect

- `dull lock` pins each dependency to a digest in `dull.lock`, and `dull fetch`
  pulls exactly those digests. So `lock → fetch` is keyed by the digests in `dull.lock`.
- `dull build` runs with **`--pull=never`** by default. It never pulls on its own,
  so it only sees base images that `dull fetch` already placed in project storage.
  A missing base is an error, not a silent pull — but it doesn't check that
  what's there came from `dull fetch`. Anything already sitting in project
  storage under that name is used as-is. Run `dull rmi --all` first if you
  want a clean build.
- `dull build` records each built image's digest in `.dull/dull_state`, and
  `dull push` pushes from those digests. So `build → push` is keyed by the digests
  in `dull_state`.

## Dullstart

```console
$ dull init                     # create dull.toml
$ dull venv                     # set up this project's isolated storage
$ dull scan                     # find Containerfiles and add them to dull.toml
$ dull lock                     # resolve dependencies (base images) to digests
$ dull fetch                    # pull the locked dependencies into local storage
$ dull build --all              # build every image in dull.toml
$ dull registry add ghcr ghcr.io/myorg
$ dull push --all -r ghcr       # push every built image
```

May this tool make your uninteresting project even more dull.