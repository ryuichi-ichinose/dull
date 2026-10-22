#!/usr/bin/env bats
# init/venv/scan/lock/fetch/build are assumed to work (covered by their own
# *.bats) and are run as plain setup.

load "fixtures/common"

setup_file() {
    DULL="$(dull_bin)" || return 1
    echo "$DULL" >"$BATS_FILE_TMPDIR/dull_bin"

    read -r REGISTRY_HOST REGISTRY_PORT < <(start_fixture_registry "$DULL")
    echo "$REGISTRY_HOST" >"$BATS_FILE_TMPDIR/registry_host"

    SCRATCH="$(new_scratch_dir)"
    echo "$SCRATCH" >"$BATS_FILE_TMPDIR/scratch"
    (
        cd "$SCRATCH"
        "$DULL" init >/dev/null
        cp -r "$BATS_TEST_DIRNAME/fixtures/solo-a" .
        "$DULL" scan >/dev/null
        "$DULL" venv >/dev/null
        sed -i "s#^dependencies = \[\]\$#dependencies = [\"localhost:$REGISTRY_PORT/base-a:latest\"]#" dull.toml
        "$DULL" lock --insecure >/dev/null
        "$DULL" fetch --insecure >/dev/null
        "$DULL" build --all >/dev/null
    )
}

teardown_file() {
    DULL=$(cat "$BATS_FILE_TMPDIR/dull_bin")
    REGISTRY_HOST=$(cat "$BATS_FILE_TMPDIR/registry_host")
    SCRATCH=$(cat "$BATS_FILE_TMPDIR/scratch")
    stop_fixture_registry "$DULL" "$REGISTRY_HOST"
    remove_scratch_dir "$SCRATCH"
}

setup() {
    DULL=$(cat "$BATS_FILE_TMPDIR/dull_bin")
    SCRATCH=$(cat "$BATS_FILE_TMPDIR/scratch")
    cd "$SCRATCH"
}

@test "export --all writes an OCI layout for every built image" {
    # --all exports ctx.state.images (dull_state), which only ever holds
    # images dull itself built. base-a is fetched, not built, and is never
    # written to dull_state, so it has nothing to export here.
    run "$DULL" export --all -o oci-out
    [ "$status" -eq 0 ]
    [ -f oci-out/localhost_solo-a_latest/index.json ]
}
