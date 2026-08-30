# dull login

Log in to a container registry.

## Usage

```console
dull login [options] <registry>
```

## Description

Logs into `registry`, using this project's
isolated storage. Credentials are managed by
`skopeo`/`containers-auth.json`, not by `dull` itself.

## Options

- `--username`, `-u <user>` — Username for the registry.
- `--password`, `-p <pass>` — Password for the registry.
- `--insecure` — Allow HTTP / untrusted TLS certificates for this registry.
