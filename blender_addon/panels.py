"""
SKOPE UI Panels
Game Editor style UI in Blender
- Hierarchy Panel: Tree view of scene objects (like Unity/Unreal)
- Inspector Panel: Component editor
- Play Button: Launch SKOPE engine
"""

import bpy
from bpy.types import Panel, UIList


# ============ Hierarchy Panel (Game Editor Style) ============

class SKOPE_UL_HierarchyList(UIList):
    """UIList for displaying scene objects in hierarchy"""
    bl_idname = "SKOPE_UL_hierarchy_list"

    def draw_item(self, context, layout, data, item, icon, active_data, active_propname, index):
        obj = item

        if self.layout_type in {'DEFAULT', 'COMPACT'}:
            row = layout.row(align=True)

            # Indentation based on parent depth
            depth = 0
            parent = obj.parent
            while parent:
                depth += 1
                parent = parent.parent
            for _ in range(depth):
                row.label(text="", icon='BLANK1')

            # Component type icon
            comp_type = obj.skope_component.component_type
            icon_map = {
                'NONE': 'OBJECT_DATA',
                'PLAYER_SPAWN': 'ARMATURE_DATA',
                'ENEMY_SPAWNER': 'COMMUNITY',
                'STATIC_PROP': 'MESH_CUBE',
                'COLLIDER': 'MESH_UVSPHERE',
                'ITEM_PICKUP': 'GIFT',
                'TRIGGER_ZONE': 'LIGHTPROBE_VOLUME',
                'LIGHT': 'LIGHT',
                'CAMERA': 'CAMERA_DATA',
                'CHARACTER': 'OUTLINER_OB_ARMATURE',
                'AUDIO_SOURCE': 'SPEAKER',
                'PARTICLE_EMITTER': 'PARTICLES',
            }
            obj_icon = icon_map.get(comp_type, 'OBJECT_DATA')

            # Visibility toggle
            row.prop(obj, "hide_viewport", text="", icon='HIDE_OFF' if not obj.hide_viewport else 'HIDE_ON', emboss=False)

            # Object name
            row.prop(obj, "name", text="", emboss=False, icon=obj_icon)

            # Component badge
            if comp_type != 'NONE':
                row.label(text="", icon='CHECKMARK')

        elif self.layout_type == 'GRID':
            layout.alignment = 'CENTER'
            layout.label(text=obj.name, icon='OBJECT_DATA')


class SKOPE_PT_Hierarchy(Panel):
    """Game Engine style Hierarchy panel"""
    bl_label = "Hierarchy"
    bl_idname = "SKOPE_PT_hierarchy"
    bl_space_type = 'VIEW_3D'
    bl_region_type = 'UI'
    bl_category = "SKOPE"
    bl_order = 0  # First panel

    def draw(self, context):
        layout = self.layout
        scene = context.scene

        # Play controls at top
        row = layout.row(align=True)
        row.scale_y = 1.5
        row.operator("skope.play", text="Play", icon='PLAY')
        row.operator("skope.play", text="", icon='PAUSE').action = 'STOP'

        layout.separator()

        # Search filter
        row = layout.row()
        row.prop(scene, "skope_hierarchy_filter", text="", icon='VIEWZOOM')

        # Quick filters
        row = layout.row(align=True)
        row.prop(scene, "skope_filter_components_only", text="Components Only", toggle=True)

        layout.separator()

        # Object list
        box = layout.box()

        # Get filtered objects
        filter_text = scene.skope_hierarchy_filter.lower() if hasattr(scene, 'skope_hierarchy_filter') else ""
        components_only = getattr(scene, 'skope_filter_components_only', False)

        # Sort: parents first, then children
        def get_hierarchy_order(obj):
            depth = 0
            parent = obj.parent
            while parent:
                depth += 1
                parent = parent.parent
            return (depth, obj.name)

        objects = sorted(scene.objects, key=get_hierarchy_order)

        displayed = 0
        for obj in objects:
            # Filter
            if filter_text and filter_text not in obj.name.lower():
                continue
            if components_only and obj.skope_component.component_type == 'NONE':
                continue

            displayed += 1
            self.draw_hierarchy_item(box, obj, context)

        if displayed == 0:
            box.label(text="No objects found", icon='INFO')

        # Footer: object count
        layout.label(text=f"{displayed} objects", icon='OBJECT_DATA')

    def draw_hierarchy_item(self, layout, obj, context):
        """Draw a single hierarchy item"""
        row = layout.row(align=True)

        # Indentation
        depth = 0
        parent = obj.parent
        while parent:
            depth += 1
            parent = parent.parent

        # Collapse/expand indicator (if has children)
        has_children = any(child.parent == obj for child in context.scene.objects)
        if has_children:
            row.label(text="", icon='TRIA_DOWN')
        else:
            for _ in range(min(depth, 3)):  # Max 3 levels of indent shown
                row.label(text="", icon='BLANK1')

        # Selection highlight
        is_selected = obj.select_get()
        is_active = context.object == obj

        # Component icon
        comp_type = obj.skope_component.component_type
        icon_map = {
            'NONE': 'DOT',
            'PLAYER_SPAWN': 'ARMATURE_DATA',
            'ENEMY_SPAWNER': 'COMMUNITY',
            'STATIC_PROP': 'MESH_CUBE',
            'COLLIDER': 'MOD_PHYSICS',
            'ITEM_PICKUP': 'GIFT',
            'TRIGGER_ZONE': 'SELECT_SET',
            'LIGHT': 'LIGHT',
            'CAMERA': 'CAMERA_DATA',
            'CHARACTER': 'OUTLINER_OB_ARMATURE',
            'AUDIO_SOURCE': 'SPEAKER',
            'PARTICLE_EMITTER': 'PARTICLES',
        }
        icon = icon_map.get(comp_type, 'OBJECT_DATA')

        # Object button (click to select)
        op = row.operator("skope.select_object", text=obj.name, icon=icon, emboss=is_active, depress=is_selected)
        op.object_name = obj.name

        # Visibility
        row.prop(obj, "hide_viewport", text="", icon='HIDE_OFF' if not obj.hide_viewport else 'HIDE_ON', emboss=False)


# ============ Inspector Panel (Enhanced) ============

class SKOPE_PT_Inspector(Panel):
    """Inspector panel - component details"""
    bl_label = "Inspector"
    bl_idname = "SKOPE_PT_inspector"
    bl_space_type = 'VIEW_3D'
    bl_region_type = 'UI'
    bl_category = "SKOPE"
    bl_order = 1

    def draw(self, context):
        layout = self.layout
        obj = context.object

        if obj is None:
            layout.label(text="No object selected", icon='ERROR')
            return

        # Object info header
        box = layout.box()
        row = box.row()
        row.label(text=obj.name, icon='OBJECT_DATA')
        row.prop(obj, "hide_viewport", text="", icon='HIDE_OFF' if not obj.hide_viewport else 'HIDE_ON')

        # Transform
        self.draw_transform(layout, obj)

        # Components
        self.draw_components(layout, obj, context)

    def draw_transform(self, layout, obj):
        """Draw transform section"""
        box = layout.box()
        row = box.row()
        row.label(text="Transform", icon='ORIENTATION_GLOBAL')

        col = box.column(align=True)

        # Position
        row = col.row(align=True)
        row.label(text="Position")
        row.prop(obj, "location", text="")

        # Rotation
        row = col.row(align=True)
        row.label(text="Rotation")
        row.prop(obj, "rotation_euler", text="")

        # Scale
        row = col.row(align=True)
        row.label(text="Scale")
        row.prop(obj, "scale", text="")

    def draw_components(self, layout, obj, context):
        """Draw component section"""
        props = obj.skope_component

        # Component type selector
        box = layout.box()
        row = box.row()
        row.label(text="SKOPE Component", icon='MODIFIER')
        box.prop(props, "component_type", text="")

        comp_type = props.component_type

        # Component-specific properties
        if comp_type == 'NONE':
            box.label(text="No component assigned", icon='INFO')
            return

        # Draw component properties based on type
        if comp_type == 'PLAYER_SPAWN':
            self.draw_player_spawn(box, props)
        elif comp_type == 'ENEMY_SPAWNER':
            self.draw_enemy_spawner(box, props)
        elif comp_type == 'STATIC_PROP':
            self.draw_static_prop(box, props, obj)
        elif comp_type == 'COLLIDER':
            self.draw_collider(box, props)
        elif comp_type == 'ITEM_PICKUP':
            self.draw_item_pickup(box, props)
        elif comp_type == 'TRIGGER_ZONE':
            self.draw_trigger_zone(box, props)
        elif comp_type == 'LIGHT':
            self.draw_light(box, obj, props)
        elif comp_type == 'CAMERA':
            self.draw_camera(box, obj, props)
        elif comp_type == 'CHARACTER':
            self.draw_character(box, props)
        elif comp_type == 'AUDIO_SOURCE':
            self.draw_audio_source(box, props)
        elif comp_type == 'PARTICLE_EMITTER':
            self.draw_particle_emitter(box, props)

    def draw_player_spawn(self, box, props):
        col = box.column()
        col.label(text="Player Spawn Point", icon='ARMATURE_DATA')
        col.label(text="Player will spawn at this location")

    def draw_enemy_spawner(self, box, props):
        col = box.column()
        col.prop(props, "enemy_type")
        col.prop(props, "enemy_count")
        col.prop(props, "enemy_respawn")

    def draw_static_prop(self, box, props, obj):
        col = box.column()

        # Mesh
        row = col.row(align=True)
        row.prop(props, "mesh_name", text="Mesh")
        row.operator("skope.auto_fill_mesh_name", text="", icon='EYEDROPPER')
        if not props.mesh_name and obj.data:
            col.label(text=f"Default: {obj.data.name}", icon='INFO')

        col.separator()

        # Shading
        col.label(text="Shading", icon='SHADING_RENDERED')
        col.prop(props, "shading_model", text="")

        # Outline
        row = col.row()
        row.prop(props, "enable_outline")
        if props.enable_outline:
            col.prop(props, "outline_thickness")
            col.prop(props, "outline_color", text="")

        col.separator()

        # Physics / Collision
        col.label(text="Physics", icon='PHYSICS')
        col.prop(props, "has_collision")
        if props.has_collision:
            col.prop(props, "collider_shape", text="Shape")
            col.prop(props, "physics_type", text="Type")

            if props.physics_type == 'DYNAMIC':
                col.prop(props, "mass")

            if props.physics_type != 'NONE':
                col.prop(props, "friction")
                col.prop(props, "restitution")

        col.separator()

        # Tags & Layer
        col.label(text="Gameplay", icon='GAME')
        col.prop(props, "tags")
        col.prop(props, "layer")

    def draw_collider(self, box, props):
        col = box.column()
        col.prop(props, "collider_shape")
        col.prop(props, "is_trigger")

    def draw_item_pickup(self, box, props):
        col = box.column()
        col.prop(props, "item_id")
        col.prop(props, "item_type")

    def draw_trigger_zone(self, box, props):
        col = box.column()
        col.prop(props, "trigger_event")

    def draw_light(self, box, obj, props):
        col = box.column()

        if obj.type == 'LIGHT':
            light = obj.data
            col.label(text=f"Type: {light.type}", icon='LIGHT')
            col.prop(light, "energy", text="Intensity")
            col.prop(light, "color", text="Color")

            if light.type == 'SPOT':
                col.prop(light, "spot_size", text="Spot Angle")
                col.prop(light, "spot_blend", text="Spot Blend")

            if light.type in {'POINT', 'SPOT'}:
                col.prop(light, "shadow_soft_size", text="Radius")

            # SKOPE-specific light settings
            col.separator()
            col.label(text="SKOPE Settings", icon='PREFERENCES')
            col.prop(props, "cast_shadows")
            col.prop(props, "shadow_bias")
        else:
            col.label(text="Not a light object!", icon='ERROR')
            col.label(text="Assign to a Light object")

    def draw_camera(self, box, obj, props):
        col = box.column()

        if obj.type == 'CAMERA':
            cam = obj.data
            col.label(text="Camera", icon='CAMERA_DATA')
            col.prop(cam, "lens", text="Focal Length")
            col.prop(cam, "clip_start", text="Near Clip")
            col.prop(cam, "clip_end", text="Far Clip")

            col.separator()
            col.label(text="SKOPE Settings", icon='PREFERENCES')
            col.prop(props, "camera_type")
            col.prop(props, "is_main_camera")
        else:
            col.label(text="Not a camera object!", icon='ERROR')

    def draw_character(self, box, props):
        col = box.column()
        col.label(text="Character", icon='OUTLINER_OB_ARMATURE')
        col.prop(props, "character_id")
        col.prop(props, "character_type")
        col.prop(props, "team")

        col.separator()
        col.label(text="Stats", icon='PREFERENCES')
        col.prop(props, "max_health")
        col.prop(props, "move_speed")

    def draw_audio_source(self, box, props):
        col = box.column()
        col.label(text="Audio Source", icon='SPEAKER')
        col.prop(props, "audio_clip")
        col.prop(props, "audio_volume")
        col.prop(props, "audio_loop")
        col.prop(props, "audio_spatial")
        if props.audio_spatial:
            col.prop(props, "audio_min_distance")
            col.prop(props, "audio_max_distance")

    def draw_particle_emitter(self, box, props):
        col = box.column()
        col.label(text="Particle Emitter", icon='PARTICLES')
        col.prop(props, "particle_type")
        col.prop(props, "emission_rate")
        col.prop(props, "particle_lifetime")


# ============ Scene Overview Panel ============

class SKOPE_PT_SceneOverview(Panel):
    """Scene statistics and overview"""
    bl_label = "Scene Overview"
    bl_idname = "SKOPE_PT_scene_overview"
    bl_space_type = 'VIEW_3D'
    bl_region_type = 'UI'
    bl_category = "SKOPE"
    bl_options = {'DEFAULT_CLOSED'}
    bl_order = 2

    def draw(self, context):
        layout = self.layout
        scene = context.scene

        # Count components
        component_counts = {}
        for obj in scene.objects:
            comp_type = obj.skope_component.component_type
            if comp_type != 'NONE':
                component_counts[comp_type] = component_counts.get(comp_type, 0) + 1

        total = sum(component_counts.values())

        # Summary
        box = layout.box()
        box.label(text=f"Total Components: {total}", icon='OBJECT_DATA')
        box.label(text=f"Total Objects: {len(scene.objects)}", icon='OUTLINER')

        if not component_counts:
            box.label(text="No SKOPE components in scene", icon='INFO')
            return

        # Breakdown
        layout.separator()
        for comp_type, count in sorted(component_counts.items()):
            row = layout.row()
            readable_name = comp_type.replace('_', ' ').title()
            row.label(text=readable_name)
            row.label(text=str(count))


# ============ Project & Export Panel ============

class SKOPE_PT_Project(Panel):
    """Project settings and export"""
    bl_label = "Project"
    bl_idname = "SKOPE_PT_project"
    bl_space_type = 'VIEW_3D'
    bl_region_type = 'UI'
    bl_category = "SKOPE"
    bl_order = 3

    def draw(self, context):
        layout = self.layout
        settings = context.scene.skope_project

        # Project path
        box = layout.box()
        box.label(text="SKOPE Project", icon='FILE_FOLDER')
        box.prop(settings, "project_path", text="")

        if not settings.project_path:
            box.label(text="Set project path to enable export", icon='ERROR')
            return

        # Export buttons
        layout.separator()
        box = layout.box()
        box.label(text="Export", icon='EXPORT')

        # One-click export
        row = box.row()
        row.scale_y = 1.5
        row.operator("skope.export_all", text="Export All", icon='PACKAGE')

        # Individual exports
        col = box.column(align=True)
        col.operator("skope.export_selected_gltf", text="Selected → glTF", icon='MESH_DATA')
        col.operator("skope.export_scene", text="Scene → .skope", icon='SCENE_DATA')

        # Export options
        layout.separator()
        box = layout.box()
        box.label(text="Options", icon='PREFERENCES')
        box.prop(settings, "auto_export_gltf")
        box.prop(settings, "export_textures")

        # Paths info
        layout.separator()
        col = layout.column()
        col.prop(settings, "assets_subfolder", text="Assets")
        col.prop(settings, "levels_subfolder", text="Levels")


# ============ Old Panels (Compatibility) ============

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
        layout.prop(props, "component_type", text="Type")

        if props.component_type != 'NONE':
            layout.label(text="See N-Panel (SKOPE tab) for details", icon='RIGHTARROW')


# ============ Registration ============

classes = [
    SKOPE_UL_HierarchyList,
    SKOPE_PT_Hierarchy,
    SKOPE_PT_Inspector,
    SKOPE_PT_SceneOverview,
    SKOPE_PT_Project,
    SKOPE_PT_ComponentPanel,
]


def register():
    """Register panel classes"""
    for cls in classes:
        bpy.utils.register_class(cls)

    # Scene properties for hierarchy
    bpy.types.Scene.skope_hierarchy_filter = bpy.props.StringProperty(
        name="Filter",
        description="Filter objects by name",
        default="",
    )
    bpy.types.Scene.skope_filter_components_only = bpy.props.BoolProperty(
        name="Components Only",
        description="Show only objects with SKOPE components",
        default=False,
    )


def unregister():
    """Unregister panel classes"""
    del bpy.types.Scene.skope_hierarchy_filter
    del bpy.types.Scene.skope_filter_components_only

    for cls in reversed(classes):
        bpy.utils.unregister_class(cls)
