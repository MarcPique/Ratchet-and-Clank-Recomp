#!/usr/bin/env bash
# Builds a Ratchet & Clank Recomp version folder and zips it (launcher contract: docs/plan/launcher_contract.md).
#
#   tools/package/package.sh              release build of `ratchet_clank` + `ratchet_clank-extract`, then package
#   tools/package/package.sh --no-build   package the binaries already in target/release
#
# Output: dist/ratchet_clank-<version>-<os>-<arch>/ and dist/ratchet_clank-<version>-<os>-<arch>.zip, holding
#   ratchet_clank[.exe]              the runtime (crate rc-engine; plain release build, never `--features dev`)
#   ratchet_clank-extract[.exe]      the extractor (crate rc-extract)
#   assets/shaders/*.wgsl    the runtime's shaders (found next to the executable; rc-engine main.rs `asset_dir`)
#   ratchet_clank-manifest.json      the version manifest, version taken from [workspace.package] in Cargo.toml
#   README.txt
#   LICENSE                 the project's ISC license and copyright notice
# Nothing from the disc is ever packaged: the folder is checked against that exact file list.
# The folder itself is a valid launcher version (the launcher's Development source can point at it).
#
# macOS: tested. Linux and Windows (Git Bash / MSYS2): written, UNTESTED.
set -euo pipefail

build=1
for a in "$@"; do
  case "$a" in
    --no-build) build=0 ;;
    -h|--help) sed -n '2,16p' "$0"; exit 0 ;;
    *) echo "unknown argument: $a" >&2; exit 2 ;;
  esac
done

repo="$(cd "$(dirname "$0")/../.." && pwd -P)"
cd "$repo"

# The one workspace version: `version` under [workspace.package] (every crate inherits it).
version="$(awk '/^\[/ { in_pkg = ($0 == "[workspace.package]") } in_pkg && /^version = "/ { gsub(/^version = "|"$/, ""); print; exit }' Cargo.toml)"
[ -n "$version" ] || { echo "cannot read the version from [workspace.package] in Cargo.toml" >&2; exit 1; }

exe=""
case "$(uname -s)" in
  Darwin) os=macos ;;
  Linux) os=linux ;;                                  # UNTESTED
  MINGW*|MSYS*|CYGWIN*) os=windows; exe=".exe" ;;     # UNTESTED
  *) echo "unsupported OS: $(uname -s)" >&2; exit 1 ;;
esac
case "$(uname -m)" in
  arm64|aarch64) arch=arm64 ;;
  x86_64|amd64) arch=x86_64 ;;
  *) arch="$(uname -m)" ;;
esac

name="ratchet_clank-$version-$os-$arch"
target="${CARGO_TARGET_DIR:-$repo/target}/release"
dist="$repo/dist"
out="$dist/$name"

# Keep the cleanup below inside this checkout even when dist is a symlink.
resolved_out="$(realpath -m "$out")"
resolved_zip="$(realpath -m "$dist/$name.zip")"
[ "$resolved_out" = "$repo/dist/$name" ] && [ "$resolved_zip" = "$repo/dist/$name.zip" ] || {
  echo "package paths resolve outside the checkout's dist directory" >&2
  exit 1
}

if [ "$build" = 1 ]; then
  # One cargo invocation for both binaries. No `--features dev`: that links Bevy dynamically.
  start=$(date +%s)
  cargo build --release --locked -p rc-engine -p rc-extract --bins
  echo "build: $(( $(date +%s) - start )) s"
fi
for b in ratchet_clank ratchet_clank-extract; do
  [ -x "$target/$b$exe" ] || { echo "missing $target/$b$exe (run without --no-build)" >&2; exit 1; }
done

rm -rf "$out" "$dist/$name.zip"
mkdir -p "$out/assets/shaders"
cp "$target/ratchet_clank$exe" "$target/ratchet_clank-extract$exe" "$out/"
cp LICENSE "$out/"
cp tools/package/CONTROLLER-PROMPTS-LICENSE.txt "$out/"
cp crates/rc-engine/assets/shaders/*.wgsl "$out/assets/shaders/"
if [ "$os" = windows ]; then
  cp tools/package/*.ps1 tools/package/*.cmd "$out/"
fi

cat > "$out/ratchet_clank-manifest.json" <<EOF
{"schema":1,"name":"ratchet_clank","version":"$version","game":"rac1","runtime":"ratchet_clank$exe","extractor":"ratchet_clank-extract$exe","supported_discs":["SCUS_971.99"],"data_format":1}
EOF

cat > "$out/README.txt" <<EOF
Ratchet & Clank Recomp $version ($os-$arch)
A reimplementation of Ratchet & Clank (PS2). It contains no game data: it runs from data
extracted once from your own disc image (NTSC-U SCUS_971.99, .iso).

Normal use: add this folder as a version in the Ratchet & Clank Recomp launcher. The launcher runs the
extractor and starts the game.

By hand:
  ./ratchet_clank-extract identify --iso <your disc>.iso
  ./ratchet_clank-extract extract --iso <your disc>.iso --out <data folder>
  ./ratchet_clank --data-dir <data folder>

On Windows, double-click "Instalar Ratchet y Clank.cmd" to select your own ISO,
verify the extracted data and create a desktop shortcut. "Actualizar Ratchet y
Clank.cmd" checks the personal release and verifies its SHA-256 before replacing
the game files; the data and saves are kept outside the version folder.
"Configurar Controles.cmd" opens the keyboard/gamepad mapping editor. Restart
the game after changing buttons. Camera and accessibility settings are inside
Advanced Options in both the title and pause menus.
Button icons follow DualSense, DualShock 4/3, Xbox Series/One/360 and Switch controllers
automatically, including remapped gamepad buttons. Advanced Options also has a
manual icon choice for adapters. The High resolution HUD option scales the menu
and HUD render target with the output resolution, up to four times the original.
The HD controller icons use Kenney Input Prompts (CC0); see CONTROLLER-PROMPTS-LICENSE.txt.
Changed save files keep five older copies beside each save, named .ratchet_clank-bak1
through .ratchet_clank-bak5. To restore one, close the game and copy it over the
matching save file in %APPDATA%/ratchet_clank/memcard.

Files: ratchet_clank (the game), ratchet_clank-extract (the extractor), assets/ (shaders, must stay next
to ratchet_clank), ratchet_clank-manifest.json (the launcher's version manifest), LICENSE.
Exit codes of ratchet_clank: 2 bad arguments, 3 data folder missing or incomplete, 4 data made for
another version (re-extract).
EOF
if [ "$os" = windows ]; then  # UNTESTED
  sed -i 's#\./ratchet_clank#ratchet_clank#g; s/$/\r/' "$out/README.txt"
fi

# Allow-list: exactly the files above, and only .wgsl under assets/. Anything else fails the package.
unexpected="$(cd "$out" && find . -type f \
  ! -path "./ratchet_clank$exe" ! -path "./ratchet_clank-extract$exe" ! -path ./ratchet_clank-manifest.json ! -path ./README.txt ! -path ./LICENSE ! -path ./CONTROLLER-PROMPTS-LICENSE.txt \
  ! -path './Install-RatchetClank.ps1' ! -path './Update-RatchetClank.ps1' \
  ! -path './Configure-Controls.ps1' \
  ! -path './Instalar Ratchet y Clank.cmd' ! -path './Actualizar Ratchet y Clank.cmd' ! -path './Configurar Controles.cmd' \
  ! -path './assets/shaders/*.wgsl')"
if [ -n "$unexpected" ]; then
  echo "unexpected files in the package (allow-list in tools/package/package.sh):" >&2
  echo "$unexpected" >&2
  exit 1
fi

# The packaged runtime must answer the contract line with this version.
got="$("$out/ratchet_clank$exe" --version-json)"
want="{\"name\":\"ratchet_clank\",\"version\":\"$version\",\"game\":\"rac1\",\"data_format\":1}"
[ "$got" = "$want" ] || { echo "ratchet_clank --version-json: got $got, want $want" >&2; exit 1; }

case "$os" in
  macos) (cd "$dist" && ditto -c -k --keepParent "$name" "$name.zip") ;;
  # Windows' own bsdtar writes a standard zip ('/' separators); PowerShell 5.1's Compress-Archive writes '\'.
  windows) (cd "$dist" && "$(cygpath "$SYSTEMROOT")/System32/tar.exe" -a -c -f "$name.zip" "$name") ;;  # UNTESTED
  linux) (cd "$dist" && zip -qry "$name.zip" "$name") ;;  # UNTESTED; needs `zip`
esac

size=$(wc -c < "$dist/$name.zip" | tr -d ' ')
echo "packaged $out"
echo "zip $dist/$name.zip ($size bytes)"
