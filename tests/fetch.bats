#!/usr/bin/env bats
# init/venv/lock are assumed to work (covered by their own *.bats) and are
# run as plain setup.

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
        "$DULL" venv >/dev/null
        sed -i "s#^dependencies = \[\]\$#dependencies = [\"localhost:$REGISTRY_PORT/base-a:latest\", \"localhost:$REGISTRY_PORT/base-b:latest\"]#" dull.toml
        "$DULL" lock --insecure >/dev/null
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

recorded_lock_digest() {
    toml_json dull.lock | jq -r --arg k "localhost:$REGISTRY_PORT/$1:latest" '.images[$k].digest'
}

@test "fetch pulls the locked digest into this project's own storage" {
    run "$DULL" fetch --insecure
    [ "$status" -eq 0 ]

    for name in base-a base-b; do
        in_storage="$(env $(project_storage_env "$SCRATCH") skopeo inspect \
            "containers-storage:localhost:$REGISTRY_PORT/$name:latest" | jq -r '.Digest')"
        [ "$in_storage" = "$(recorded_lock_digest "$name")" ]
    done
}

@test "fetch is a no-op before lock has written dull.lock" {
    SCRATCH2="$(new_scratch_dir)"
    (cd "$SCRATCH2" && "$DULL" init >/dev/null && "$DULL" venv >/dev/null)
    cd "$SCRATCH2"
    run "$DULL" fetch
    [ "$status" -eq 0 ]
    [[ "$output" == *"Run 'dull lock' first"* ]]
    remove_scratch_dir "$SCRATCH2"
}

@test "fetch can target a single named dependency" {
    run "$DULL" fetch --insecure "localhost:$REGISTRY_PORT/base-a:latest"
    [ "$status" -eq 0 ]
    [[ "$output" == *"Fetched 1 dependencies."* ]]
}
