#!/bin/bash
# base-a/base-b are filler, not meaningful example content, so they're
# synthesized here rather than living under fixtures/. Run with
# REGISTRY_PORT and REGISTRY_IMAGE set.
set -euo pipefail

podman run -d --name fixture-registry -p "${REGISTRY_PORT}:5000" "$REGISTRY_IMAGE" >/dev/null

for _ in $(seq 1 30); do
    curl -sf "http://127.0.0.1:${REGISTRY_PORT}/v2/" >/dev/null && break
    sleep 1
done

for name in base-a base-b; do
    mkdir -p "$name"
    printf 'FROM scratch\nCOPY dull.txt /dull.txt\n' >"$name/Containerfile"
    printf 'dull-%s-content\n' "$name" >"$name/dull.txt"
    buildah bud -t "localhost:${REGISTRY_PORT}/${name}:latest" -f "${name}/Containerfile" "${name}"
    buildah push --tls-verify=false "localhost:${REGISTRY_PORT}/${name}:latest"
done
