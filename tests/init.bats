#!/usr/bin/env bats
# The one file that checks init's own behavior; every other file just runs
# it as setup without re-checking it.

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
    cd "$SCRATCH"
}

@test "init writes the baseline tables" {
    run "$DULL" init
    [ "$status" -eq 0 ]
    toml_json dull.toml | jq -e '.dependencies == [] and .registries == {} and .images == {}' >/dev/null
}

@test "init does not overwrite an existing dull.toml" {
    echo "# marker" >>dull.toml
    run "$DULL" init
    [ "$status" -eq 0 ]
    grep -q '# marker' dull.toml
}

@test "init is the only command that works without an existing dull.toml" {
    rm -f dull.toml
    run "$DULL" scan
    [ "$status" -ne 0 ]
    [[ "$output" == *"No dull.toml found"* ]]
}
