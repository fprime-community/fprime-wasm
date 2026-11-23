#!/usr/bin/env bash
# Builds the `fprime-wasm-opt-<platform>` crates: binaryen's own `wasm-opt` release
# builds, one crate per platform, which `fprime-wasm` embeds and runs. Each tarball is
# checked against `sha256` before anything is taken from it, and each crate is
# `templates/` rendered for its platform.
#
#   fprime_wasm_opt/package.sh [--publish] [OUT]   # crates into OUT, default target/fprime_wasm_opt
#
# The crates are versioned by binaryen, not with this workspace: `version_133` is
# `133.0.0`. To move to a new binaryen, replace every line of `sha256` with the new
# release's digests, run this with --publish, and bump the `=` requirements in
# fprime_wasm/Cargo.toml to match. A repackaging of the same release bumps PATCH.
set -euo pipefail

PATCH=0

here="$(cd "$(dirname "$0")" && pwd)"
out="$here/../target/fprime_wasm_opt"
publish=""
for arg in "$@"; do
  case "$arg" in
    --publish) publish=1 ;;
    -*) echo "error: unknown option $arg" >&2; exit 2 ;;
    *) out="$arg" ;;
  esac
done
mkdir -p "$out"
out="$(cd "$out" && pwd)"

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1"; else shasum -a 256 "$1"; fi \
    | awk '{print $1}'
}

# `render TEMPLATE OUTPUT`: replaces each `{{key}}` with the variable `key`. A key the
# template uses but nothing set fails rather than rendering empty.
render() {
  local text key
  text="$(cat "$here/templates/$1")"
  for key in $(grep -o '{{[a-z_]*}}' "$here/templates/$1" | sort -u | tr -d '{}'); do
    if [ -z "${!key+set}" ]; then
      echo "error: templates/$1 uses {{$key}}, which package.sh does not set" >&2
      exit 1
    fi
    text="${text//"{{$key}}"/${!key}}"
  done
  printf '%s\n' "$text" > "$2"
}

while read -r digest path; do
  [ -n "$digest" ] || continue
  binaryen="${path%%/*}"
  tarball="${path#*/}"
  platform="${tarball#binaryen-"$binaryen"-}"
  platform="${platform%.tar.gz}"
  version="${binaryen#version_}.0.$PATCH"
  name="fprime-wasm-opt-$platform"
  crate="$out/$name"
  echo "== $name $version"

  download="$out/$tarball"
  if [ ! -f "$download" ] || [ "$(sha256 "$download")" != "$digest" ]; then
    curl -fsSL --retry 3 -o "$download" \
      "https://github.com/WebAssembly/binaryen/releases/download/$binaryen/$tarball"
  fi
  actual="$(sha256 "$download")"
  if [ "$actual" != "$digest" ]; then
    echo "error: $tarball has sha256 $actual, expected $digest" >&2
    exit 1
  fi

  # Built aside and moved into place whole, so a failure leaves no half-written crate.
  staging="$crate.partial"
  rm -rf "$staging"
  mkdir -p "$staging/src"
  case "$platform" in
    *-windows) wanted=(bin/wasm-opt.exe) ;;
    # Loads its library from `@loader_path/../lib`.
    *-macos) wanted=(bin/wasm-opt lib/libbinaryen.dylib) ;;
    *) wanted=(bin/wasm-opt) ;;
  esac
  members=()
  files=""
  for file in "${wanted[@]}"; do
    members+=("binaryen-$binaryen/$file")
    files+="    (\"$file\", include_bytes!(\"../$file\")),
"
  done
  tar xzf "$download" -C "$staging" --strip-components 1 "${members[@]}"
  curl -fsSL --retry 3 -o "$staging/LICENSE" \
    "https://raw.githubusercontent.com/WebAssembly/binaryen/$binaryen/LICENSE"

  render Cargo.toml.tmpl "$staging/Cargo.toml"
  render README.md.tmpl "$staging/README.md"
  render lib.rs.tmpl "$staging/src/lib.rs"
  rm -rf "$crate"
  mv "$staging" "$crate"

  if [ -n "$publish" ]; then
    # crates.io takes only ~5 new crate names in a burst, so the sixth may need a rerun
    # once the limit refills; a rerun skips what already went up.
    if curl -sf -A "fprime-wasm-opt packaging (https://github.com/fprime-community/fprime-wasm)" \
      "https://crates.io/api/v1/crates/$name/$version" >/dev/null; then
      echo "$name $version is already on crates.io; skipping."
      continue
    fi
    (cd "$crate" && cargo publish)
  else
    (cd "$crate" && cargo package --quiet --no-verify && ls -l "target/package/$name-$version.crate" | awk '{print $5, $NF}')
  fi
done < "$here/sha256"
