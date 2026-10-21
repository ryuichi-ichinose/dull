# Shared helpers for the dull CLI bats suite; `load "fixtures/common"`.

dull_bin() {
    local bin="$BATS_TEST_DIRNAME/../target/debug/dull"
    if [ ! -x "$bin" ]; then
        echo "target/debug/dull not found; run 'cargo build' first" >&2
        return 1
    fi
    echo "$bin"
}

new_scratch_dir() {
    # Resolved to a clean path: buildah/skopeo report storage paths that
    # way, so a literal ".." here would never substring-match them.
    local dir
    dir="$(mktemp -d -p "$BATS_TEST_DIRNAME/../target")"
    (cd "$dir" && pwd)
}

remove_scratch_dir() {
    buildah unshare rm -rf "$1" 2>/dev/null || rm -rf "$1"
}

# buildah/skopeo honor these directly, the same way dull invokes them, so
# no dull-shell wrapper is needed to inspect a project's storage from outside.
project_storage_env() {
    echo "CONTAINERS_STORAGE_CONF=$1/.dull/containers/storage.conf TMPDIR=$1/.dull/containers/tmp"
}

# jq-based assertions instead of grep/line-offset matching against the raw file.
toml_json() {
    python3 -I "$BATS_TEST_DIRNAME/fixtures/toml2json.py" "$1"
}

# Prints "<registry_host_dir> <port>" on success.
start_fixture_registry() {
    local dull="$1"
    local host_dir
    host_dir="$(new_scratch_dir)"
    local port=15050

    (cd "$host_dir" && "$dull" init >/dev/null && "$dull" venv >/dev/null) || return 1
    (
        cd "$host_dir" || exit 1
        # dull shell prints its own banner to stdout first; discard it so
        # it doesn't end up in the "<host_dir> <port>" this prints.
        REGISTRY_PORT="$port" REGISTRY_IMAGE="quay.io/libpod/registry:2.8.2" \
            SHELL="$BATS_TEST_DIRNAME/fixtures/setup-registry.sh" "$dull" shell >/dev/null </dev/null
    ) || return 1

    echo "$host_dir $port"
}

stop_fixture_registry() {
    local dull="$1" host_dir="$2"
    (cd "$host_dir" && SHELL="$BATS_TEST_DIRNAME/fixtures/teardown-registry.sh" "$dull" shell </dev/null) || true
    (cd "$host_dir" && "$dull" rmi --all >/dev/null 2>&1) || true
    remove_scratch_dir "$host_dir"
}
