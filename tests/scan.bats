#!/usr/bin/env bats
# init is assumed to work (covered by init.bats) and is run as plain setup.

load "fixtures/common"

setup_file() {
    DULL="$(dull_bin)" || return 1
    SCRATCH="$(new_scratch_dir)"
    echo "$DULL" >"$BATS_FILE_TMPDIR/dull_bin"
    echo "$SCRATCH" >"$BATS_FILE_TMPDIR/scratch"
}

teardown_file() {
    DULL=$(cat "$BATS_FILE_TMPDIR/dull_bin")
    SCRATCH=$(cat "$BATS_FILE_TMPDIR/scratch")
    remove_scratch_dir "$SCRATCH"
}

setup() {
    DULL=$(cat "$BATS_FILE_TMPDIR/dull_bin")
    SCRATCH=$(cat "$BATS_FILE_TMPDIR/scratch")
    # Each test wants a fresh dull.toml; `dull init` no-ops if one already
    # exists, so leftovers from the previous test would otherwise leak in.
    rm -rf "$SCRATCH"
    mkdir -p "$SCRATCH"
    cd "$SCRATCH"
}

@test "scan finds a Containerfile and adds [images.X]" {
    "$DULL" init >/dev/null
    cp -r "$BATS_TEST_DIRNAME/fixtures/solo-a" .
    run "$DULL" scan
    [ "$status" -eq 0 ]
    [[ "$output" == *"+ Found new image: solo-a"* ]]
    toml_json dull.toml | jq -e '.images["solo-a"].path == "solo-a/Containerfile" and .images["solo-a"].context == "solo-a"' >/dev/null
}

@test "scan derives the name from a nested directory path" {
    "$DULL" init >/dev/null
    mkdir -p services/api
    printf 'FROM scratch\n' >services/api/Containerfile
    run "$DULL" scan
    [ "$status" -eq 0 ]
    [ "$(toml_json dull.toml | jq -r '.images["services/api"].path')" = "services/api/Containerfile" ]
}

@test "scan rejects a name that is not OCI compliant" {
    "$DULL" init >/dev/null
    mkdir -p "bad name"
    printf 'FROM scratch\n' >"bad name/Containerfile"
    run "$DULL" scan
    [ "$status" -ne 0 ]
    [[ "$output" == *"Invalid image name 'bad name'"* ]]
    [[ "$output" == *"OCI compliant"* ]]
}

@test "scan removes an entry whose Containerfile disappeared" {
    "$DULL" init >/dev/null
    cp -r "$BATS_TEST_DIRNAME/fixtures/solo-a" .
    "$DULL" scan >/dev/null
    [ "$(toml_json dull.toml | jq -r '.images["solo-a"].path')" = "solo-a/Containerfile" ]

    rm -rf solo-a
    run "$DULL" scan
    [ "$status" -eq 0 ]
    [[ "$output" == *"- Removed missing image (file not found): solo-a"* ]]
    [ "$(toml_json dull.toml | jq -e '.images | has("solo-a")')" = "false" ]
}

@test "scan does not re-add an entry renamed by hand" {
    "$DULL" init >/dev/null
    cp -r "$BATS_TEST_DIRNAME/fixtures/solo-a" .
    "$DULL" scan >/dev/null
    sed -i 's/\[images.solo-a\]/[images.renamed]/' dull.toml

    run "$DULL" scan
    [ "$status" -eq 0 ]
    toml_json dull.toml | jq -e '.images | has("renamed") and (has("solo-a") | not)' >/dev/null
}

@test "scan detects two new entries colliding on the same lowercased name" {
    "$DULL" init >/dev/null
    mkdir -p Collide collide
    printf 'FROM scratch\n' >Collide/Containerfile
    printf 'FROM scratch\n' >collide/Containerfile

    run "$DULL" scan
    [ "$status" -ne 0 ]
    [[ "$output" == *"Image name collision detected"* ]]
}

@test "scan detects a new entry colliding with an existing differently-cased one" {
    "$DULL" init >/dev/null
    mkdir -p Collide
    printf 'FROM scratch\n' >Collide/Containerfile
    "$DULL" scan >/dev/null
    # The name is lowercased ("collide"); path/context keep the original case.
    [ "$(toml_json dull.toml | jq -r '.images["collide"].path')" = "Collide/Containerfile" ]

    mkdir -p collide
    printf 'FROM scratch\n' >collide/Containerfile
    # Restricted to "collide" so this run's walk never re-visits "Collide/";
    # its [images.collide] entry stays in dull.toml (the file still exists)
    # but isn't rediscovered as a same-run sibling collision.
    run "$DULL" scan collide
    [ "$status" -ne 0 ]
    [[ "$output" == *"Image name collision: 'collide' is already configured"* ]]
}
