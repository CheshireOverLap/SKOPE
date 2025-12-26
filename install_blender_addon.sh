#!/bin/bash
# SKOPE Blender Addon Installer
# Installs the addon to Blender 5.0 user scripts directory

set -e

ADDON_NAME="skope_exporter"
BLENDER_VERSION="5.0"
ADDON_SOURCE="blender_addon"

# Detect OS and set Blender scripts path
if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    SCRIPTS_DIR="$HOME/.config/blender/$BLENDER_VERSION/scripts/addons"
elif [[ "$OSTYPE" == "darwin"* ]]; then
    SCRIPTS_DIR="$HOME/Library/Application Support/Blender/$BLENDER_VERSION/scripts/addons"
elif [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "win32" ]]; then
    SCRIPTS_DIR="$APPDATA/Blender Foundation/Blender/$BLENDER_VERSION/scripts/addons"
else
    echo "Unsupported OS: $OSTYPE"
    exit 1
fi

ADDON_DEST="$SCRIPTS_DIR/$ADDON_NAME"

echo "=== SKOPE Blender Addon Installer ==="
echo "Source: $ADDON_SOURCE"
echo "Destination: $ADDON_DEST"
echo ""

# Create scripts directory if it doesn't exist
if [ ! -d "$SCRIPTS_DIR" ]; then
    echo "Creating Blender scripts directory..."
    mkdir -p "$SCRIPTS_DIR"
fi

# Remove old installation if exists
if [ -d "$ADDON_DEST" ]; then
    echo "Removing old installation..."
    rm -rf "$ADDON_DEST"
fi

# Copy addon files
echo "Installing addon..."
cp -r "$ADDON_SOURCE" "$ADDON_DEST"

echo ""
echo "✓ Installation complete!"
echo ""
echo "Next steps:"
echo "1. Open Blender 5.0.1"
echo "2. Go to Edit → Preferences → Add-ons"
echo "3. Search for 'SKOPE Exporter'"
echo "4. Enable the checkbox"
echo ""
echo "Or run Blender with:"
echo "  blender --python-expr \"import bpy; bpy.ops.preferences.addon_enable(module='$ADDON_NAME')\""
