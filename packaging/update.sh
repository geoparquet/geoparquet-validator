#!/usr/bin/env sh
# Refresh the version and checksums in the Homebrew formula and the winget manifests from the
# assets of a published release:  sh packaging/update.sh v0.3.0
set -e
TAG=${1:?usage: update.sh vX.Y.Z}
VER=${TAG#v}
cd "$(dirname "$0")"
BASE="https://github.com/geoparquet/geoparquet-validator/releases/download/$TAG/geoparquet-validator-$TAG"
sum() { curl -sL "$1" | shasum -a 256 | cut -d' ' -f1; }
F=homebrew/geoparquet-validator.rb
sed -i.bak -E "s#/download/v[0-9.]+/geoparquet-validator-v[0-9.]+-#/download/$TAG/geoparquet-validator-$TAG-#g" "$F"
for t in aarch64-apple-darwin x86_64-apple-darwin aarch64-unknown-linux-gnu x86_64-unknown-linux-gnu; do
  S=$(sum "$BASE-$t.tar.gz")
  # the sha256 line that follows this target's url line
  awk -v t="$t" -v s="$S" '{ if (found && $1=="sha256") { sub(/"[0-9a-f]+"/, "\"" s "\""); found=0 } if ($0 ~ t) found=1; print }' "$F" > "$F.tmp" && mv "$F.tmp" "$F"
done
rm -f "$F.bak"
ZIP_SUM=$(sum "$BASE-x86_64-pc-windows-msvc.zip" | tr 'a-f' 'A-F')
OLD=$(ls -d winget/manifests/g/GeoParquet/Validator/*/ | head -1)
NEW="winget/manifests/g/GeoParquet/Validator/$VER"
[ "$OLD" = "$NEW/" ] || { mkdir -p "$NEW"; cp "$OLD"*.yaml "$NEW/"; }
for f in "$NEW"/*.yaml; do
  sed -i.bak -E "s/PackageVersion: .*/PackageVersion: $VER/; s#/download/v[0-9.]+/geoparquet-validator-v[0-9.]+-#/download/$TAG/geoparquet-validator-$TAG-#g; s#geoparquet-validator-v[0-9.]+-x86_64-pc-windows-msvc\\\\#geoparquet-validator-$TAG-x86_64-pc-windows-msvc\\\\#; s/InstallerSha256: .*/InstallerSha256: $ZIP_SUM/; s#releases/tag/v[0-9.]+#releases/tag/$TAG#" "$f"
  rm -f "$f.bak"
done
echo "updated packaging for $TAG"
