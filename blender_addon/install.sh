#!/bin/bash
# SKOPE UI Editor - Blender Addon 설치 스크립트

ADDON_NAME="skope_ui"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ADDON_SRC="$SCRIPT_DIR/$ADDON_NAME"

# Blender addon 경로 찾기
BLENDER_ADDON_PATHS=(
    "$HOME/.config/blender/4.3/scripts/addons"
    "$HOME/.config/blender/4.2/scripts/addons"
    "$HOME/.config/blender/4.1/scripts/addons"
    "$HOME/.config/blender/4.0/scripts/addons"
    "$HOME/.config/blender/3.6/scripts/addons"
    "$HOME/.config/blender/3.5/scripts/addons"
    "$HOME/.config/blender/3.4/scripts/addons"
    "$HOME/.config/blender/3.3/scripts/addons"
    "$HOME/.config/blender/3.0/scripts/addons"
)

echo "====================================="
echo "  SKOPE UI Editor - Blender Addon"
echo "====================================="
echo ""

# 소스 확인
if [ ! -d "$ADDON_SRC" ]; then
    echo "Error: Addon source not found at $ADDON_SRC"
    exit 1
fi

# Blender addon 경로 찾기
ADDON_DEST=""
for path in "${BLENDER_ADDON_PATHS[@]}"; do
    if [ -d "$(dirname "$path")" ]; then
        ADDON_DEST="$path"
        break
    fi
done

if [ -z "$ADDON_DEST" ]; then
    echo "Blender addon path not found!"
    echo ""
    echo "Please specify your Blender version's addon path:"
    echo "  Example: ~/.config/blender/4.0/scripts/addons"
    echo ""
    read -p "Path: " ADDON_DEST
fi

# 디렉토리 생성
mkdir -p "$ADDON_DEST"

# 기존 설치 제거
if [ -d "$ADDON_DEST/$ADDON_NAME" ]; then
    echo "Removing existing installation..."
    rm -rf "$ADDON_DEST/$ADDON_NAME"
fi

# 복사
echo "Installing to: $ADDON_DEST/$ADDON_NAME"
cp -r "$ADDON_SRC" "$ADDON_DEST/"

echo ""
echo "Installation complete!"
echo ""
echo "To enable the addon in Blender:"
echo "  1. Open Blender"
echo "  2. Go to Edit > Preferences > Add-ons"
echo "  3. Search for 'SKOPE'"
echo "  4. Enable 'SKOPE UI Editor'"
echo ""
echo "The addon panel will appear in the 3D View sidebar (N key) under 'SKOPE UI'"
