#!/bin/sh
set -eu
project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cache_path="$project_root/.cache/mlx"
mkdir -p "$cache_path"
fetch() {
  source_url=$1
  source_name=$2
  expected_hash=$3
  archive_path="$cache_path/$source_name.tar.gz"
  if [ ! -f "$archive_path" ]; then
    curl -fL --max-time 120 "$source_url" --output "$archive_path.part"
    mv "$archive_path.part" "$archive_path"
  fi
  actual_hash=$(shasum -a 256 "$archive_path" | cut -d ' ' -f 1)
  [ "$actual_hash" = "$expected_hash" ] || { echo "Checksum mismatch: $source_name" >&2; exit 1; }
  if [ ! -d "$cache_path/$source_name" ]; then tar -xzf "$archive_path" -C "$cache_path"; fi
}
fetch https://codeload.github.com/ml-explore/mlx/tar.gz/refs/tags/v0.32.2 mlx-0.32.2 bf892386ccac1d249acc9da21574fd7b95a995f4c58b03310105fe05f077b175
fetch https://codeload.github.com/nlohmann/json/tar.gz/refs/tags/v3.11.3 json-3.11.3 0d8ef5af7f9794e3263480193c491549b2ba6cc74bb018906202ada498a79406
fetch https://codeload.github.com/fmtlib/fmt/tar.gz/refs/tags/12.1.0 fmt-12.1.0 ea7de4299689e12b6dddd392f9896f08fb0777ac7168897a244a6d6085043fea
fetch https://codeload.github.com/antirez/gguf-tools/tar.gz/8fa6eb65236618e28fd7710a0fba565f7faa1848 gguf-tools-8fa6eb65236618e28fd7710a0fba565f7faa1848 9e30bc1eb82cc2231150d39ce37dcdd6f844d6994fba18da83fc537a487ba86f
cp "$project_root/scripts/mlx-toolchain.cmake" "$cache_path/toolchain.cmake"
echo "Pinned MLX sources ready in $cache_path"
