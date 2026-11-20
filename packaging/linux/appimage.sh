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
  LD="$WORK/linuxdeploy-$ARCH.AppImage"
  curl -fsSL -o "$LD" "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-$ARCH.AppImage"
  chmod +x "$LD"
fi
# the icon file name must match Icon= of the .desktop file
cp "$HERE/icons/256.png" "$WORK/$ID.png"
mkdir -p "$WORK/AppDir/usr/share/metainfo" "$WORK/AppDir/usr/share/icons/hicolor/scalable/apps"
cp "$HERE/$ID.metainfo.xml" "$WORK/AppDir/usr/share/metainfo/"
cp "$HERE/icons/sqlion.svg" "$WORK/AppDir/usr/share/icons/hicolor/scalable/apps/$ID.svg"
mkdir -p "$OUT"
cd "$WORK"
# no FUSE on CI runners: the tool runs extracted
APPIMAGE_EXTRACT_AND_RUN=1 LINUXDEPLOY_OUTPUT_VERSION="$VERSION" "$LD" \
  --appdir AppDir --executable "$BIN" \
  --desktop-file "$HERE/$ID.desktop" --icon-file "$WORK/$ID.png" \
  --output appimage
mv "$WORK"/*.AppImage "$OUT/sqlion-$VERSION-$ARCH.AppImage"
echo "$OUT/sqlion-$VERSION-$ARCH.AppImage"
