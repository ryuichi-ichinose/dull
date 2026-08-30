# dull registry

Manage registry aliases used by push.

## Usage

```console
dull registry add <name> <url>
dull registry rm <name>
dull registry ls
```

## Description

Manages the [`[registries]`](dull.toml.md) table in `dull.toml`.
`dull push -r` takes one of these aliases, not a raw registry URL.

## Subcommands

- `add <name> <url>` — Add or update an alias. A trailing `/` on `url` is stripped.
- `rm <name>` — Remove an alias.
- `ls` — List configured aliases and their URLs.