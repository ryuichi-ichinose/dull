#!/usr/bin/env bats
# init/venv/scan/lock/fetch/build are assumed to work (covered by their own
# *.bats) and are run as plain setup.

load "fixtures/common"

setup_file() {
    DULL="$(dull_bin)" || return 1
    echo "$DULL" >"$BATS_FILE_TMPDIR/dull_bin"

    read -r REGISTRY_HOST REGISTRY_PORT < <(start_fixture_registry "$DULL")
    echo "$REGISTRY_HOST" >"$BATS_FILE_TMPDIR/registry_host"
    echo "$REGISTRY_PORT" >"$BATS_FILE_TMPDIR/registry_port"

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
    REGISTRY_PORT=$(cat "$BATS_FILE_TMPDIR/registry_port")
    SCRATCH=$(cat "$BATS_FILE_TMPDIR/scratch")
    cd "$SCRATCH"
}

@test "images --json lists the fetched dependency as Lock and the built image as State" {
    run "$DULL" images --json
    [ "$status" -eq 0 ]
    lock_source="$(echo "$output" | jq -r --arg n "base-a" '.[] | select(.name==$n) | .source')"
    state_source="$(echo "$output" | jq -r --arg n "solo-a" '.[] | select(.name==$n) | .source')"
    [ "$lock_source" = "Lock" ]
    [ "$state_source" = "State" ]
}

@test "images --json flags a tracked image removed from storage as Missing" {
    env $(project_storage_env "$SCRATCH") buildah rmi localhost/solo-a:latest >/dev/null

    run "$DULL" images --json --state
    [ "$status" -eq 0 ]
    source="$(echo "$output" | jq -r --arg n "solo-a" '.[] | select(.name==$n) | .source')"
    [ "$source" = "State (Missing)" ]
}
