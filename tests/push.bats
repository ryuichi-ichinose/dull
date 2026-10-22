#!/usr/bin/env bats
# init/venv/scan/lock/fetch/build/registry are assumed to work (covered by
# their own *.bats) and are run as plain setup.

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
        "$DULL" registry add local "localhost:$REGISTRY_PORT" >/dev/null
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

@test "push lands at exactly <url>/<name>:<tag> (the [images.X] key and its local tag)" {
    run "$DULL" push --all -r local --insecure
    [ "$status" -eq 0 ]
    [[ "$output" == *"+ localhost/solo-a:latest -> localhost:$REGISTRY_PORT/solo-a:latest"* ]]

    skopeo inspect --tls-verify=false "docker://localhost:$REGISTRY_PORT/solo-a:latest" >/dev/null
}

@test "push --tag overrides only the destination tag, not the name" {
    run "$DULL" push --all -r local --insecure --tag pushed
    [ "$status" -eq 0 ]
    [[ "$output" == *"+ localhost/solo-a:latest -> localhost:$REGISTRY_PORT/solo-a:pushed"* ]]

    skopeo inspect --tls-verify=false "docker://localhost:$REGISTRY_PORT/solo-a:pushed" >/dev/null
    run skopeo inspect --tls-verify=false "docker://localhost:$REGISTRY_PORT/solo-a:latest"
    [ "$status" -eq 0 ]
}
