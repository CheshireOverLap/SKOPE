"""
SKOPE UI Panels
UI panels for component assignment in Blender Properties tab
"""

import bpy
from bpy.types import Panel

class SKOPE_PT_ComponentPanel(Panel):
    """Main component panel in Object Properties"""
    bl_label = "SKOPE Component"
    bl_idname = "SKOPE_PT_component_panel"
    bl_space_type = 'PROPERTIES'
    bl_region_type = 'WINDOW'
    bl_context = "object"

    def draw(self, context):
        layout = self.layout
        obj = context.object

        if obj is None:
            layout.label(text="No object selected")
            return

        props = obj.skope_component

        # Component Type Selector
        layout.prop(props, "component_type", text="Type")

        # Component-specific properties
        comp_type = props.component_type

        if comp_type == 'NONE':
            layout.label(text="No component assigned")

        elif comp_type == 'PLAYER_SPAWN':
            box = layout.box()
            box.label(text="Player Spawn", icon='OUTLINER_OB_ARMATURE')
            box.label(text="Position only (use object transform)")

        elif comp_type == 'ENEMY_SPAWNER':
            box = layout.box()
            box.label(text="Enemy Spawner", icon='OBJECT_ORIGIN')
            box.prop(props, "enemy_type")
            box.prop(props, "enemy_count")
            box.prop(props, "enemy_respawn")

        elif comp_type == 'STATIC_PROP':
            box = layout.box()
            box.label(text="Static Prop", icon='MESH_CUBE')
            box.prop(props, "has_collision")

        elif comp_type == 'COLLIDER':
            box = layout.box()
            box.label(text="Collider", icon='MESH_UVSPHERE')
            box.prop(props, "collider_shape")
            box.prop(props, "is_trigger")

        elif comp_type == 'ITEM_PICKUP':
            box = layout.box()
            box.label(text="Item Pickup", icon='PACKAGE')
            box.prop(props, "item_id")
            box.prop(props, "item_type")

        elif comp_type == 'TRIGGER_ZONE':
            box = layout.box()
            box.label(text="Trigger Zone", icon='LIGHTPROBE_CUBEMAP')
            box.prop(props, "trigger_event")

        elif comp_type == 'LIGHT':
            box = layout.box()
            box.label(text="Game Light", icon='LIGHT')
            box.label(text="Uses Blender light properties")

        # Export button
        layout.separator()
        layout.operator("skope.export_scene", text="Export SKOPE Scene", icon='EXPORT')

# ============ Registration ============

classes = [
    SKOPE_PT_ComponentPanel,
]

def register():
    """Register panel classes"""
    for cls in classes:
        bpy.utils.register_class(cls)

def unregister():
    """Unregister panel classes"""
    for cls in reversed(classes):
        bpy.utils.unregister_class(cls)
