#!/usr/bin/env bash
# Builds the pinned llama-server that the app bundles as its Tauri externalBin, plus the
# license files it must ship with. Must not install anything outside this repository.
set -euo pipefail

LLAMA_CPP_TAG="b11123"

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
source_dir="$repo_root/target/llama.cpp-$LLAMA_CPP_TAG"
binaries_dir="$repo_root/app/src-tauri/binaries"
licenses_dir="$repo_root/app/src-tauri/licenses/llama.cpp"

if [ ! -d "$source_dir" ]; then
  git clone --depth 1 --branch "$LLAMA_CPP_TAG" https://github.com/ggml-org/llama.cpp.git "$source_dir"
fi

common_flags=(
  -DCMAKE_BUILD_TYPE=Release
  -DBUILD_SHARED_LIBS=OFF
  -DGGML_NATIVE=OFF
  -DLLAMA_BUILD_TESTS=OFF
  -DLLAMA_BUILD_EXAMPLES=OFF
  -DLLAMA_BUILD_TOOLS=ON
  -DLLAMA_BUILD_SERVER=ON
  -DLLAMA_OPENSSL=OFF
  -DCMAKE_OSX_DEPLOYMENT_TARGET=13.3
)

# The Metal shaders are embedded in the binary, so no Metal compiler (full Xcode) is needed.
cmake -S "$source_dir" -B "$source_dir/build-arm64" "${common_flags[@]}" \
  -DCMAKE_OSX_ARCHITECTURES=arm64 -DGGML_METAL=ON -DGGML_METAL_EMBED_LIBRARY=ON
cmake --build "$source_dir/build-arm64" --config Release --target llama-server -j "$(sysctl -n hw.ncpu)"

cmake -S "$source_dir" -B "$source_dir/build-x86_64" "${common_flags[@]}" \
  -DCMAKE_OSX_ARCHITECTURES=x86_64 -DGGML_METAL=OFF
cmake --build "$source_dir/build-x86_64" --config Release --target llama-server -j "$(sysctl -n hw.ncpu)"

# Tauri needs one file per target triple: cargo builds each architecture of a universal
# app separately, and the bundler then looks for the universal file.
mkdir -p "$binaries_dir"
cp "$source_dir/build-arm64/bin/llama-server" "$binaries_dir/llama-server-aarch64-apple-darwin"
cp "$source_dir/build-x86_64/bin/llama-server" "$binaries_dir/llama-server-x86_64-apple-darwin"
lipo -create -output "$binaries_dir/llama-server-universal-apple-darwin" \
  "$binaries_dir/llama-server-aarch64-apple-darwin" "$binaries_dir/llama-server-x86_64-apple-darwin"

mkdir -p "$licenses_dir"
cp "$source_dir/LICENSE" "$licenses_dir/LICENSE-llama.cpp"
cp "$source_dir/vendor/cpp-httplib/LICENSE" "$licenses_dir/LICENSE-cpp-httplib"
cp "$source_dir/licenses/"* "$licenses_dir/"
