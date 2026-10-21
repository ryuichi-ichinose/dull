#!/usr/bin/env bats
# init/venv/scan/lock/fetch/build are assumed to work (covered by their own
# *.bats) and are run as plain setup. Several tests here remove state- or
# lock-tracked images, so later tests that need them re-run `build --all`
# or `fetch --insecure` to replenish.

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
        cat >>dull.toml <<'EOF'

[build_groups.deps]
base = "base"
images = ["dep-1", "dep-2"]
EOF
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

# base-a is lock-only (never built, see export.bats), everything else is
# state-tracked. A stray image, outside dull entirely, stands in for
# "untracked" garbage the --keep-* modes are meant to sweep up.
build_stray_image() {
    (
        export $(project_storage_env "$SCRATCH")
        cid="$(buildah from scratch)"
        buildah commit "$cid" localhost/stray:latest >/dev/null
        buildah rm "$cid" >/dev/null
    )
}

image_exists() {
    env $(project_storage_env "$SCRATCH") skopeo inspect "containers-storage:localhost/$1:latest" >/dev/null 2>&1
}

base_a_exists() {
    env $(project_storage_env "$SCRATCH") skopeo inspect \
        "containers-storage:localhost:$REGISTRY_PORT/base-a:latest" >/dev/null 2>&1
}

@test "rmi <name> removes it from storage and from dull_state" {
    image_exists solo-b
    run "$DULL" rmi solo-b
    [ "$status" -eq 0 ]
    [[ "$output" == *"- Removed: solo-b"* ]]
    [ "$(toml_json .dull/dull_state | jq -e '.images | has("solo-b")')" = "false" ]
    ! image_exists solo-b

    "$DULL" build --all >/dev/null
}

@test "rmi with no target and no flags is an error" {
    run "$DULL" rmi
    [ "$status" -ne 0 ]
    [[ "$output" == *"Specify an image name"* ]]
}

@test "rmi <name> on an untracked name is a no-op" {
    run "$DULL" rmi not-a-real-image
    [ "$status" -eq 0 ]
    [[ "$output" == *"not-a-real-image not tracked; skipping"* ]]
    [[ "$output" == *"Nothing to remove."* ]]
}

@test "rmi --all wipes project storage and resets dull_state" {
    run "$DULL" rmi --all
    [ "$status" -eq 0 ]
    [ "$(env $(project_storage_env "$SCRATCH") buildah images -q | wc -l)" -eq 0 ]
    [ "$(toml_json .dull/dull_state | jq -r '.images | length')" -eq 0 ]

    "$DULL" fetch --insecure >/dev/null
    "$DULL" build --all >/dev/null
}

@test "rmi --keep-tracked removes only what state/lock don't reference" {
    build_stray_image
    run "$DULL" rmi --keep-tracked
    [ "$status" -eq 0 ]
    ! image_exists stray
    for name in app solo-a solo-b base dep-1 dep-2; do
        image_exists "$name"
    done
    base_a_exists
}

@test "rmi --keep-state keeps built images, drops the lock-only dependency and stray garbage" {
    build_stray_image
    run "$DULL" rmi --keep-state
    [ "$status" -eq 0 ]
    ! image_exists stray
    for name in app solo-a solo-b base dep-1 dep-2; do
        image_exists "$name"
    done
    ! base_a_exists

    "$DULL" fetch --insecure >/dev/null
}

@test "rmi --keep-lock keeps the lock dependency, drops built images and stray garbage" {
    build_stray_image
    run "$DULL" rmi --keep-lock
    [ "$status" -eq 0 ]
    ! image_exists stray
    for name in app solo-a solo-b base dep-1 dep-2; do
        ! image_exists "$name"
    done
    base_a_exists

    "$DULL" build --all >/dev/null
}
