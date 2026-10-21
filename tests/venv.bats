#!/usr/bin/env bats
# driver/mountopt/size/inodes are free-form strings, so checked individually,
# not combinatorially like the boolean options below.
#
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
    rm -rf "$SCRATCH"
    mkdir -p "$SCRATCH"
    cd "$SCRATCH"
    "$DULL" init >/dev/null
}

# `venv` no-ops if .dull/ already exists, so each case needs a clean slate.
reset_venv() {
    rm -rf .dull
}

@test "venv storage.conf reflects every boolean option, in every combination" {
    for composefs in true false; do
        for convert in true false; do
            for partial in true false; do
                for hardlinks in true false; do
                    reset_venv
                    run "$DULL" venv --composefs "$composefs" \
                        --convert-images "$convert" \
                        --enable-partial-images "$partial" \
                        --use-hard-links "$hardlinks"
                    [ "$status" -eq 0 ]
                    toml_json .dull/containers/storage.conf | jq -e \
                        --arg composefs "$composefs" --arg convert "$convert" \
                        --arg partial "$partial" --arg hardlinks "$hardlinks" \
                        '.storage.options.overlay.use_composefs == $composefs
                            and .storage.options.pull_options.convert_images == $convert
                            and .storage.options.pull_options.enable_partial_images == $partial
                            and .storage.options.pull_options.use_hard_links == $hardlinks' >/dev/null
                done
            done
        done
    done
}

@test "venv --driver is written as given" {
    reset_venv
    run "$DULL" venv --driver vfs
    [ "$status" -eq 0 ]
    [ "$(toml_json .dull/containers/storage.conf | jq -r '.storage.driver')" = "vfs" ]
}

@test "venv --mountopt is written as given" {
    reset_venv
    run "$DULL" venv --mountopt "nodev,metacopy=on"
    [ "$status" -eq 0 ]
    [ "$(toml_json .dull/containers/storage.conf | jq -r '.storage.options.overlay.mountopt')" = "nodev,metacopy=on" ]
}

@test "venv --size is written as given" {
    reset_venv
    run "$DULL" venv --size 5G
    [ "$status" -eq 0 ]
    [ "$(toml_json .dull/containers/storage.conf | jq -r '.storage.options.overlay.size')" = "5G" ]
}

@test "venv --inodes is written as given" {
    reset_venv
    run "$DULL" venv --inodes 100000
    [ "$status" -eq 0 ]
    [ "$(toml_json .dull/containers/storage.conf | jq -r '.storage.options.overlay.inodes')" = "100000" ]
}

@test "venv without --size/--inodes omits those keys" {
    reset_venv
    run "$DULL" venv
    [ "$status" -eq 0 ]
    toml_json .dull/containers/storage.conf | jq -e \
        '(.storage.options.overlay | has("size") | not) and (.storage.options.overlay | has("inodes") | not)' >/dev/null
}

@test "venv --global writes an empty storage.conf and skips the project store" {
    reset_venv
    run "$DULL" venv --global
    [ "$status" -eq 0 ]
    [ "$(toml_json .dull/containers/storage.conf | jq -e '. == {}')" = "true" ]
    # The emptiness itself is data (checked above); the explanatory comment
    # is not, and tomllib discards comments, so this one stays a plain grep.
    grep -qi 'global container storage' .dull/containers/storage.conf
}

@test "venv is a no-op if .dull already exists" {
    reset_venv
    "$DULL" venv --driver vfs >/dev/null
    run "$DULL" venv --driver overlay
    [ "$status" -eq 0 ]
    [[ "$output" == *"already exists"* ]]
    [ "$(toml_json .dull/containers/storage.conf | jq -r '.storage.driver')" = "vfs" ]
}
