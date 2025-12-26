"""
SKOPE Blender Addon
Export Blender scenes to SKOPE Engine (.skope format)
"""

bl_info = {
    "name": "SKOPE Exporter",
    "author": "SKOPE Games",
    "version": (0, 1, 0),
    "blender": (5, 0, 1),
    "location": "Properties > Object > SKOPE Component",
    "description": "Export Blender scenes to SKOPE Engine with game components",
    "category": "Import-Export",
}

import bpy
from . import properties
from . import panels
from . import operators

# Module registration
modules = [
    properties,
    panels,
    operators,
]

def register():
    """Register all addon classes"""
    for module in modules:
        module.register()

    print("SKOPE Exporter addon registered")

def unregister():
    """Unregister all addon classes"""
    for module in reversed(modules):
        module.unregister()

    print("SKOPE Exporter addon unregistered")

if __name__ == "__main__":
    register()
