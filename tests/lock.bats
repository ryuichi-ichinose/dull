#!/usr/bin/env bats
# init/venv are assumed to work (covered by their own *.bats) and are run
# as plain setup.

load "fixtures/common"

setup_file() {
    DULL="$(dull_bin)" || return 1
    echo "$DULL" >"$BATS_FILE_TMPDIR/dull_bin"

    read -r REGISTRY_HOST REGISTRY_PORT < <(start_fixture_registry "$DULL")
    echo "$REGISTRY_HOST" >"$BATS_FILE_TMPDIR/registry_host"
    echo "$REGISTRY_PORT" >"$BATS_FILE_TMPDIR/registry_port"

    SCRATCH="$(new_scratch_dir)"
    echo "$SCRATCH" >"$BATS_FILE_TMPDIR/scratch"
    (cd "$SCRATCH" && "$DULL" init >/dev/null && "$DULL" venv >/dev/null)
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

# Computed without going through dull at all, to check dull.lock against
# an outside source of truth, not just "dull agrees with itself".
independent_digest() {
    skopeo inspect --raw --tls-verify=false "docker://localhost:$REGISTRY_PORT/$1:latest" \
        | sha256sum | cut -d' ' -f1
}

@test "lock records the same digest skopeo inspect --raw computes" {
    sed -i "s#^dependencies = \[\]\$#dependencies = [\"localhost:$REGISTRY_PORT/base-a:latest\", \"localhost:$REGISTRY_PORT/base-b:latest\"]#" dull.toml

    run "$DULL" lock --insecure
    [ "$status" -eq 0 ]

    for name in base-a base-b; do
        expected="sha256:$(independent_digest "$name")"
        recorded="$(toml_json dull.lock | jq -r --arg k "localhost:$REGISTRY_PORT/$name:latest" '.images[$k].digest')"
        [ "$recorded" = "$expected" ]
    done
}

@test "lock --upgrade re-resolves an already-locked dependency" {
    run "$DULL" lock --insecure --upgrade "localhost:$REGISTRY_PORT/base-a:latest"
    [ "$status" -eq 0 ]
    [[ "$output" == *"No digest changes."* ]]
}

@test "lock rejects a short name (non-FQIN)" {
    SCRATCH2="$(new_scratch_dir)"
    (cd "$SCRATCH2" && "$DULL" init >/dev/null && "$DULL" venv >/dev/null)
    cd "$SCRATCH2"
    sed -i 's/^dependencies = \[\]$/dependencies = ["fedora:41"]/' dull.toml
    run "$DULL" lock --insecure
    [ "$status" -ne 0 ]
    [[ "$output" == *"forbidden for reproducibility"* ]]
    remove_scratch_dir "$SCRATCH2"
}
