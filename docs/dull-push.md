# dull push

Push built images to a registry.

## Usage

```console
dull push [options] -r <registry> [--all | images...]
```

## Description

Pushes images tracked in `.dull/dull_state` (i.e. already built by
`dull build`) to a registry alias configured with `dull registry
add`. An image with no state entry, or with no
resolvable tag, is skipped with a warning rather than failing the
whole run.

Each image is pushed to `<url>/<name>:<tag>`, where `<url>` is the registry
alias's `url` (`dull registry add`), `<name>` is the `[images.X]` key, and
`<tag>` is the tag `dull build` recorded in `.dull/dull_state` for that image
(or `--tag`, to override the destination tag).

If `--sign-by` or `--sign-by-sigstore-private-key` is given without
`--sign-passphrase-file`, pushes fall back to sequential execution
(`--jobs` is ignored) with inherited stdio, since GPG/pinentry may
need to prompt interactively and a redrawing progress spinner would
fight it for the terminal.

## Options

- `--all`, `-a` — Push every image tracked in `.dull/dull_state`. Conflicts with `images`.
- `images` — Image names (`[images.X]` keys) to push.
- `--registry`, `-r <alias>` — Registry alias (see [dull registry](dull-registry.md)) to push to. Required.
- `--tag`, `-t <tag>` — Override the destination tag (default: each image's local tag).
- `--jobs`, `-j <n>` — Max number of images to push concurrently (ignored during interactive signing).
- `--insecure` — Allow HTTP / untrusted TLS certificates for this registry.
- `--sign-by <key-id>` — Sign each pushed image with this GPG key ID.
- `--sign-by-sigstore-private-key <path>` — Sign each pushed image with this sigstore
  private key file. Conflicts with `--sign-by`.
- `--sign-passphrase-file <path>` — File containing the passphrase for `--sign-by-sigstore-private-key`.
