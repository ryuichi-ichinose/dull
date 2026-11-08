# Testing

```Bash
cargo build
bats tests/init.bats       # dull.toml baseline; blocks every other command until it exists
bats tests/scan.bats       # name derivation, OCI validation, collisions
bats tests/venv.bats       # every storage.conf option, alone and combined
bats tests/shell.bats      # $SHELL runs against this project's own storage
bats tests/lock.bats       # recorded digests match an independent skopeo inspect
bats tests/fetch.bats      # locked digests land in local storage
bats tests/build.bats      # build_groups ordering, parallel builds, --tag
bats tests/images.bats     # Lock vs. State vs. Missing sources
bats tests/push.bats       # destination FQIN, with and without --tag
bats tests/export.bats     # OCI layout output
bats tests/rmi.bats        # named removal, --all, and the --keep-* GC modes
```

One `.bats` file per command. They can never run concurrently: several files' setup starts a disposable local registry on the same fixed port, and two of those at once collide.

Once `init`/`scan`/`venv`/`shell` pass their own tests, every later command treats them as working and uses them as plain setup, without re-verifying.
