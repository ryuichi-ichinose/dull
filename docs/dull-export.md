# dull export

Export built images to an OCI layout on disk.

## Usage

```console
dull export [options] [--all | images...]
```

## Description

Copies images tracked in `.dull/dull_state` out of this project's
isolated storage into an OCI image layout directory on disk, one subdirectory per image (named after the image's
tag, with `/` and `:` replaced by `_`). 

## Options

- `--all`, `-a` — Export every image tracked in `.dull/dull_state`. Conflicts with `images`.
- `images` — Image names (`[images.X]` keys) to export.
- `--output`, `-o <dir>` — Output directory (default: the project root). Created if it doesn't exist.