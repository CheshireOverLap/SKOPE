"""
SKOPE UI Panels
UI panels for component assignment in Blender
- Properties Panel: Simple component type selector
- N-Panel (3D Viewport): Full component editor + scene overview
"""

import bpy
from bpy.types import Panel

class SKOPE_PT_ComponentPanel(Panel):
    """Simple component panel in Object Properties"""
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

        # Simple info
        comp_type = props.component_type
        if comp_type != 'NONE':
            box = layout.box()
            box.label(text=f"{comp_type.replace('_', ' ').title()} Component", icon='CHECKMARK')
            box.label(text="Open N-Panel for details", icon='HAND')

        # Quick link to N-Panel
        layout.separator()
        layout.label(text="Press N → SKOPE tab for full editor", icon='RIGHTARROW')


class SKOPE_PT_NPanel(Panel):
    """Full SKOPE editor in 3D Viewport N-Panel"""
    bl_label = "SKOPE Component Editor"
    bl_idname = "SKOPE_PT_npanel"
    bl_space_type = 'VIEW_3D'
    bl_region_type = 'UI'
    bl_category = "SKOPE"

    def draw(self, context):
        layout = self.layout
        obj = context.object

        if obj is None:
            layout.label(text="No object selected", icon='ERROR')
            return

        props = obj.skope_component

        # ===== Component Type Selector =====
        box = layout.box()
        box.label(text="Component Type", icon='OBJECT_DATA')
        box.prop(props, "component_type", text="")

        comp_type = props.component_type

        # ===== Component-specific Properties =====
        if comp_type == 'PLAYER_SPAWN':
            self.draw_player_spawn(layout, props)
        elif comp_type == 'ENEMY_SPAWNER':
            self.draw_enemy_spawner(layout, props)
        elif comp_type == 'STATIC_PROP':
            self.draw_static_prop(layout, props, obj, context)
        elif comp_type == 'COLLIDER':
            self.draw_collider(layout, props)
        elif comp_type == 'ITEM_PICKUP':
            self.draw_item_pickup(layout, props)
        elif comp_type == 'TRIGGER_ZONE':
            self.draw_trigger_zone(layout, props)
        elif comp_type == 'LIGHT':
            self.draw_light(layout, obj)

    def draw_player_spawn(self, layout, props):
        box = layout.box()
        box.label(text="Player Spawn Point", icon='OUTLINER_OB_ARMATURE')
        box.label(text="Position: Object Location")
        box.label(text="Rotation: Object Rotation")

    def draw_enemy_spawner(self, layout, props):
        box = layout.box()
        box.label(text="Enemy Spawner", icon='COMMUNITY')
        box.prop(props, "enemy_type")
        box.prop(props, "enemy_count")
        box.prop(props, "enemy_respawn")

    def draw_static_prop(self, layout, props, obj, context):
        box = layout.box()
        box.label(text="Static Prop", icon='MESH_CUBE')
        box.prop(props, "has_collision")

        # Mesh name with auto-fill button
        row = box.row(align=True)
        row.prop(props, "mesh_name", text="Mesh")
        row.operator("skope.auto_fill_mesh_name", text="", icon='EYEDROPPER')

        # Show current mesh if not explicitly set
        if not props.mesh_name or not props.mesh_name.strip():
            if obj.data and hasattr(obj.data, 'name'):
                box.label(text=f"Default: {obj.data.name}", icon='INFO')

    def draw_collider(self, layout, props):
        box = layout.box()
        box.label(text="Collider", icon='MESH_UVSPHERE')
        box.prop(props, "collider_shape")
        box.prop(props, "is_trigger")

    def draw_item_pickup(self, layout, props):
        box = layout.box()
        box.label(text="Item Pickup", icon='GIFT')
        box.prop(props, "item_id")
        box.prop(props, "item_type")

    def draw_trigger_zone(self, layout, props):
        box = layout.box()
        box.label(text="Trigger Zone", icon='LIGHTPROBE_VOLUME')
        box.prop(props, "trigger_event")

    def draw_light(self, layout, obj):
        box = layout.box()
        box.label(text="Light Source", icon='LIGHT')

        if obj.type == 'LIGHT':
            light = obj.data
            box.label(text=f"Type: {light.type}")
            box.label(text=f"Energy: {light.energy:.2f}")
            color_text = f"Color: ({light.color[0]:.2f}, {light.color[1]:.2f}, {light.color[2]:.2f})"
            box.label(text=color_text)
        else:
            box.label(text="⚠ Not a light object", icon='ERROR')


class SKOPE_PT_SceneOverview(Panel):
    """Scene overview showing all SKOPE components"""
    bl_label = "Scene Overview"
    bl_idname = "SKOPE_PT_scene_overview"
    bl_space_type = 'VIEW_3D'
    bl_region_type = 'UI'
    bl_category = "SKOPE"
    bl_options = {'DEFAULT_CLOSED'}

    def draw(self, context):
        layout = self.layout
        scene = context.scene

        # Count components
        component_counts = {}
        for obj in scene.objects:
            comp_type = obj.skope_component.component_type
            if comp_type != 'NONE':
                component_counts[comp_type] = component_counts.get(comp_type, 0) + 1

        # Display summary
        if not component_counts:
            layout.label(text="No SKOPE components in scene", icon='INFO')
            return

        box = layout.box()
        box.label(text=f"Total Components: {sum(component_counts.values())}", icon='OBJECT_DATA')

        for comp_type, count in sorted(component_counts.items()):
            readable_name = comp_type.replace('_', ' ').title()
            box.label(text=f"{readable_name}: {count}")

        # List all objects with components
        layout.separator()
        layout.label(text="Objects:", icon='OUTLINER')

        for obj in scene.objects:
            comp_type = obj.skope_component.component_type
            if comp_type != 'NONE':
                row = layout.row()
                row.label(text=obj.name, icon='OBJECT_DATA')
                row.label(text=comp_type.replace('_', ' ').title())


class SKOPE_PT_Export(Panel):
    """Export panel in N-Panel"""
    bl_label = "Export"
    bl_idname = "SKOPE_PT_export"
    bl_space_type = 'VIEW_3D'
    bl_region_type = 'UI'
    bl_category = "SKOPE"

    def draw(self, context):
        layout = self.layout

        # Export button
        layout.operator("skope.export_scene", text="Export Scene (.skope)", icon='EXPORT')

        layout.separator()
        layout.label(text="Export Format: RON", icon='FILE_TEXT')
        layout.label(text="File Extension: .skope", icon='FILE')


# ============ Registration ============

classes = [
    SKOPE_PT_ComponentPanel,
    SKOPE_PT_NPanel,
    SKOPE_PT_SceneOverview,
    SKOPE_PT_Export,
]

def register():
    """Register panel classes"""
    for cls in classes:
        bpy.utils.register_class(cls)

def unregister():
    """Unregister panel classes"""
    for cls in reversed(classes):
        bpy.utils.unregister_class(cls)
