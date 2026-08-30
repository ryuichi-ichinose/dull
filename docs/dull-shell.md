# dull shell

Open a shell configured for this project's isolated storage.

## Usage

```console
dull shell
```

## Description

Execs `$SHELL` (default `/bin/bash`) with
`CONTAINERS_STORAGE_CONF`/`TMPDIR` pointed at this project's `.dull/`
storage, and `DULL_PROJECTNAME` set to the project directory's name.
Any `buildah`/`podman`/`skopeo` command run from that shell operates
against this project's isolated storage directly, without going
through `dull` itself.

This replaces the current process (it never returns) and takes no
options.
