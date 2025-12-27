"""
SKOPE Blender Addon
Export Blender scenes to SKOPE Engine (.skope format)
"""

bl_info = {
    "name": "SKOPE Editor",
    "author": "SKOPE Games",
    "version": (0, 2, 0),
    "blender": (4, 0, 0),
    "location": "3D Viewport > N-Panel > SKOPE",
    "description": "Game editor integration for SKOPE Engine - Hierarchy, Inspector, Play button",
    "category": "Game Engine",
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
