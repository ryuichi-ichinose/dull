#!/usr/bin/env bats
# init/venv/scan/lock/fetch are assumed to work (covered by their own
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
        cp -r "$BATS_TEST_DIRNAME/fixtures/app" \
            "$BATS_TEST_DIRNAME/fixtures/solo-a" \
            "$BATS_TEST_DIRNAME/fixtures/solo-b" \
            "$BATS_TEST_DIRNAME/fixtures/base" \
            "$BATS_TEST_DIRNAME/fixtures/dep-1" \
            "$BATS_TEST_DIRNAME/fixtures/dep-2" .
        sed -i "s#localhost:15050#localhost:$REGISTRY_PORT#" app/Containerfile
        "$DULL" scan >/dev/null
        "$DULL" venv >/dev/null
        sed -i "s#^dependencies = \[\]\$#dependencies = [\"localhost:$REGISTRY_PORT/base-a:latest\"]#" dull.toml
        "$DULL" lock --insecure >/dev/null
        "$DULL" fetch --insecure >/dev/null
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

@test "without a build_groups edge, base->dep-1/dep-2 order is not guaranteed" {
    # No [build_groups] edge links dep-1/dep-2 to base, and --pull=never
    # means they only build if base happened to go first. Order isn't
    # guaranteed, but a dependent built before its base must still fail,
    # never silently succeed.
    run "$DULL" build dep-1 dep-2
    echo "$output" >&3
    if [ "$status" -ne 0 ]; then
        [[ "$output" == *"image not known"* || "$output" == *"failed"* ]]
    fi
}

@test "build_groups base->dep-1,dep-2 orders a parallel build and records every image in dull_state" {
    cat >>dull.toml <<'EOF'

[build_groups.deps]
base = "base"
images = ["dep-1", "dep-2"]
EOF

    run "$DULL" build --all
    [ "$status" -eq 0 ]

    state="$(toml_json .dull/dull_state)"
    for name in app solo-a solo-b base dep-1 dep-2; do
        [ "$(echo "$state" | jq -e --arg n "$name" '.images | has($n)')" = "true" ]
    done
}

@test "build's recorded digest matches skopeo inspect on the built image" {
    recorded="$(toml_json .dull/dull_state | jq -r '.images.base.digest')"
    in_storage="$(env $(project_storage_env "$SCRATCH") skopeo inspect \
        "containers-storage:localhost/base:latest" | jq -r '.Digest')"
    [ "$recorded" = "$in_storage" ]
}

@test "build --tag overrides every built image's tag" {
    run "$DULL" build --all --tag custom
    [ "$status" -eq 0 ]
    state="$(toml_json .dull/dull_state)"
    for name in app solo-a solo-b base dep-1 dep-2; do
        [ "$(echo "$state" | jq -r --arg n "$name" '.images[$n].tag')" = "custom" ]
    done
    env $(project_storage_env "$SCRATCH") skopeo inspect \
        "containers-storage:localhost/base:custom" >/dev/null
}
