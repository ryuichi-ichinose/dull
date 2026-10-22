#!/usr/bin/env bats
# init/venv are assumed to work (covered by their own *.bats) and are run
# as plain setup.

load "fixtures/common"

setup_file() {
    DULL="$(dull_bin)" || return 1
    SCRATCH="$(new_scratch_dir)"
    echo "$DULL" >"$BATS_FILE_TMPDIR/dull_bin"
    echo "$SCRATCH" >"$BATS_FILE_TMPDIR/scratch"

    (cd "$SCRATCH" && "$DULL" init >/dev/null && "$DULL" venv >/dev/null)
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

# dull shell execs $SHELL and never returns on its own, so $SHELL has to
# point at something that runs one command and exits.
run_in_shell() {
    printf '#!/bin/bash\n%s\n' "$1" >_cmd.sh
    chmod +x _cmd.sh
    SHELL="$SCRATCH/_cmd.sh" "$DULL" shell </dev/null
    rm -f _cmd.sh
}

@test "shell: buildah inside it reports this project's own storage" {
    info="$(run_in_shell "buildah info")"
    [[ "$info" == *"$SCRATCH/.dull/containers/storage"* ]]
}

@test "shell: DULL_PROJECTNAME is set to the project directory's name" {
    # dull shell itself prints "Entering dull shell for project: ..." to
    # stdout first, ahead of whatever $SHELL goes on to print.
    out="$(run_in_shell 'echo "$DULL_PROJECTNAME"' | tail -1)"
    [ "$out" = "$(basename "$SCRATCH")" ]
}
