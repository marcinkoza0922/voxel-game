#!/usr/bin/env bash
# Build/run with official native DLSS and FSR 2 SDKs; caches downloads in target/.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo_args=()
if [[ "${1:-}" == "--release" ]]; then
    cargo_args+=(--release)
    shift
fi
SDK_CACHE="$PWD/target/native-sdk"
mkdir -p "$SDK_CACHE"
if [[ -z "${DLSS_SDK:-}" ]]; then
    export DLSS_SDK="$SDK_CACHE/DLSS"
    if [[ ! -d "$DLSS_SDK/.git" ]]; then
        git clone --depth 1 --branch v310.4.0 --filter=blob:none --sparse https://github.com/NVIDIA/DLSS.git "$DLSS_SDK"
        git -C "$DLSS_SDK" sparse-checkout set include lib/Linux_x86_64
    fi
fi
if [[ -z "${VULKAN_SDK:-}" ]]; then
    export VULKAN_SDK="$SDK_CACHE/Vulkan-Headers"
    if [[ ! -d "$VULKAN_SDK/.git" ]]; then
        git clone --depth 1 --branch v1.4.309 https://github.com/KhronosGroup/Vulkan-Headers.git "$VULKAN_SDK"
    fi
fi
DLSS_RUNTIME="$DLSS_SDK/lib/Linux_x86_64/rel"
# NGX loads the runtime by its unversioned name. Keep SDK source files intact.
mkdir -p "$SDK_CACHE/runtime"
for library in "$DLSS_RUNTIME"/libnvidia-ngx-*.so.*; do
    name="$(basename "$library")"
    ln -sf "$library" "$SDK_CACHE/runtime/${name%%.so.*}.so"
done
export LD_LIBRARY_PATH="$SDK_CACHE/runtime${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
exec cargo run "${cargo_args[@]}" --features native-upscalers -- "$@"
