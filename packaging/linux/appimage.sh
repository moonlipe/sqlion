#!/usr/bin/env bash
# Builds sqlion-<version>-<arch>.AppImage from an already compiled binary.
# usage: packaging/linux/appimage.sh <path to sqlion binary> <version> <out dir>
# Needs linuxdeploy-<arch>.AppImage in PATH or in $LINUXDEPLOY (it is downloaded when missing).
set -euo pipefail
BIN=$(realpath "$1"); VERSION=$2; OUT=$(realpath -m "$3")
ARCH=$(uname -m)
HERE=$(cd "$(dirname "$0")" && pwd)
ID=io.github.moonlipe.Sqlion
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
LD=${LINUXDEPLOY:-}
if [ -z "$LD" ]; then
  mkdir -p "$WORK/tools"
  LD="$WORK/tools/linuxdeploy-$ARCH.AppImage"
  curl -fsSL -o "$LD" "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-$ARCH.AppImage"
  chmod +x "$LD"
fi
# the icon file name must match Icon= of the .desktop file
cp "$HERE/icons/256.png" "$WORK/$ID.png"
mkdir -p "$WORK/AppDir/usr/share/metainfo" "$WORK/AppDir/usr/share/icons/hicolor/scalable/apps"
cp "$HERE/$ID.metainfo.xml" "$WORK/AppDir/usr/share/metainfo/"
cp "$HERE/icons/sqlion.svg" "$WORK/AppDir/usr/share/icons/hicolor/scalable/apps/$ID.svg"
mkdir -p "$OUT" "$WORK/out"
# linuxdeploy writes the AppImage to the current folder: an empty one, so only it matches below
cd "$WORK/out"
# no FUSE on CI runners: the tool runs extracted
APPIMAGE_EXTRACT_AND_RUN=1 LINUXDEPLOY_OUTPUT_VERSION="$VERSION" "$LD" \
  --appdir "$WORK/AppDir" --executable "$BIN" \
  --desktop-file "$HERE/$ID.desktop" --icon-file "$WORK/$ID.png" \
  --output appimage
shopt -s nullglob
made=("$WORK"/out/*.AppImage)
[ ${#made[@]} -eq 1 ] || { echo "expected one AppImage, got: ${made[*]:-none}" >&2; exit 1; }
mv "${made[0]}" "$OUT/sqlion-$VERSION-$ARCH.AppImage"
echo "$OUT/sqlion-$VERSION-$ARCH.AppImage"
