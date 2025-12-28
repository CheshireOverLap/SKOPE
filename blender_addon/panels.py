"""
SKOPE UI Panels
Game Editor style UI in Blender - Unified tabbed interface
"""

import bpy
from bpy.types import Panel, UIList


# ============ Main Unified Panel ============

class SKOPE_PT_Main(Panel):
    """SKOPE Editor - Unified tabbed interface"""
    bl_label = "SKOPE Editor"
    bl_idname = "SKOPE_PT_main"
    bl_space_type = 'VIEW_3D'
    bl_region_type = 'UI'
    bl_category = "SKOPE"
    bl_order = 0

    def draw(self, context):
        layout = self.layout
        scene = context.scene

        # Get current tab
        current_tab = getattr(scene, 'skope_editor_tab', 'HIERARCHY')

        # ===== Play Controls - Always visible =====
        play_box = layout.box()
        play_row = play_box.row(align=True)
        play_row.scale_y = 1.6

        play_row.operator("skope.play", text="Play", icon='PLAY')
        stop_op = play_row.operator("skope.play", text="", icon='SNAP_FACE')
        stop_op.action = 'STOP'
        play_row.operator("skope.export_all", text="", icon='EXPORT')

        layout.separator()

        # ===== Tab Buttons =====
        tab_row = layout.row(align=True)
        tab_row.scale_y = 1.2

        # Tab button style
        tab_row.prop_enum(scene, "skope_editor_tab", 'HIERARCHY', icon='OUTLINER')
        tab_row.prop_enum(scene, "skope_editor_tab", 'INSPECTOR', icon='PREFERENCES')
        tab_row.prop_enum(scene, "skope_editor_tab", 'ASSETS', icon='ASSET_MANAGER')
        tab_row.prop_enum(scene, "skope_editor_tab", 'PROJECT', icon='FILE_FOLDER')

        layout.separator()

        # ===== Tab Content =====
        content_box = layout.box()

        if current_tab == 'HIERARCHY':
            self.draw_hierarchy_tab(content_box, context)
        elif current_tab == 'INSPECTOR':
            self.draw_inspector_tab(content_box, context)
        elif current_tab == 'ASSETS':
            self.draw_assets_tab(content_box, context)
        elif current_tab == 'PROJECT':
            self.draw_project_tab(content_box, context)

    # ===== Hierarchy Tab =====
    def draw_hierarchy_tab(self, layout, context):
        scene = context.scene

        # Search and filter
        row = layout.row(align=True)
        row.prop(scene, "skope_hierarchy_filter", text="", icon='VIEWZOOM')
        row.prop(scene, "skope_filter_components_only", text="", icon='FILTER', toggle=True)

        layout.separator()

        # Object list
        filter_text = getattr(scene, 'skope_hierarchy_filter', '').lower()
        components_only = getattr(scene, 'skope_filter_components_only', False)

        def get_depth(obj):
            depth = 0
            parent = obj.parent
            while parent:
                depth += 1
                parent = parent.parent
            return depth

        objects = sorted(scene.objects, key=lambda o: (get_depth(o), o.name))

        displayed = 0
        for obj in objects:
            if filter_text and filter_text not in obj.name.lower():
                continue
            if components_only and obj.skope_component.component_type == 'NONE':
                continue

            displayed += 1
            self.draw_hierarchy_item(layout, obj, context)

        if displayed == 0:
            layout.label(text="No objects found", icon='INFO')

        # Footer
        layout.separator()
        layout.label(text=f"{displayed} objects", icon='OBJECT_DATA')

    def draw_hierarchy_item(self, layout, obj, context):
        row = layout.row(align=True)

        # Indent
        depth = 0
        parent = obj.parent
        while parent:
            depth += 1
            parent = parent.parent
        for _ in range(min(depth, 3)):
            row.label(text="", icon='BLANK1')

        # Icon based on component
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

        is_active = context.object == obj
        op = row.operator("skope.select_object", text=obj.name, icon=icon, emboss=is_active, depress=obj.select_get())
        op.object_name = obj.name

        row.prop(obj, "hide_viewport", text="", icon='HIDE_OFF' if not obj.hide_viewport else 'HIDE_ON', emboss=False)

    # ===== Inspector Tab =====
    def draw_inspector_tab(self, layout, context):
        obj = context.object

        if obj is None:
            layout.label(text="Select an object", icon='HAND')
            return

        # Object header
        header = layout.row(align=True)
        type_icons = {
            'MESH': 'MESH_CUBE', 'LIGHT': 'LIGHT', 'CAMERA': 'CAMERA_DATA',
            'ARMATURE': 'ARMATURE_DATA', 'EMPTY': 'EMPTY_DATA',
        }
        header.label(text="", icon=type_icons.get(obj.type, 'OBJECT_DATA'))
        header.prop(obj, "name", text="")
        header.prop(obj, "hide_viewport", text="", icon='HIDE_OFF' if not obj.hide_viewport else 'HIDE_ON', emboss=False)

        layout.separator()

        # Transform
        self.draw_transform(layout, obj)

        layout.separator()

        # Component
        self.draw_component(layout, obj, context)

    def draw_transform(self, layout, obj):
        col = layout.column(align=True)
        col.label(text="Transform", icon='ORIENTATION_GLOBAL')

        # Position
        split = col.split(factor=0.22, align=True)
        split.label(text="Pos")
        row = split.row(align=True)
        row.prop(obj, "location", index=0, text="")
        row.prop(obj, "location", index=1, text="")
        row.prop(obj, "location", index=2, text="")

        # Rotation
        split = col.split(factor=0.22, align=True)
        split.label(text="Rot")
        row = split.row(align=True)
        row.prop(obj, "rotation_euler", index=0, text="")
        row.prop(obj, "rotation_euler", index=1, text="")
        row.prop(obj, "rotation_euler", index=2, text="")

        # Scale
        split = col.split(factor=0.22, align=True)
        split.label(text="Scale")
        row = split.row(align=True)
        row.prop(obj, "scale", index=0, text="")
        row.prop(obj, "scale", index=1, text="")
        row.prop(obj, "scale", index=2, text="")

    def draw_component(self, layout, obj, context):
        props = obj.skope_component
        comp_type = props.component_type

        comp_icons = {
            'NONE': 'ADD', 'PLAYER_SPAWN': 'ARMATURE_DATA', 'ENEMY_SPAWNER': 'COMMUNITY',
            'STATIC_PROP': 'MESH_CUBE', 'COLLIDER': 'MOD_PHYSICS', 'ITEM_PICKUP': 'GIFT',
            'TRIGGER_ZONE': 'SELECT_SET', 'LIGHT': 'LIGHT', 'CAMERA': 'CAMERA_DATA',
            'CHARACTER': 'OUTLINER_OB_ARMATURE', 'AUDIO_SOURCE': 'SPEAKER',
            'PARTICLE_EMITTER': 'PARTICLES',
        }

        col = layout.column(align=True)
        row = col.row()
        row.label(text="Component", icon=comp_icons.get(comp_type, 'MODIFIER'))
        col.prop(props, "component_type", text="")

        if comp_type == 'NONE':
            col.label(text="Add component to enable", icon='INFO')
            return

        col.separator()

        # Component-specific properties (simplified)
        if comp_type == 'STATIC_PROP':
            row = col.row(align=True)
            row.prop(props, "mesh_name", text="Mesh")
            row.operator("skope.auto_fill_mesh_name", text="", icon='EYEDROPPER')
            col.prop(props, "has_collision")
        elif comp_type == 'ENEMY_SPAWNER':
            col.prop(props, "enemy_type")
            col.prop(props, "enemy_count")
            col.prop(props, "enemy_respawn")
        elif comp_type == 'COLLIDER':
            col.prop(props, "collider_shape")
            col.prop(props, "is_trigger")
        elif comp_type == 'ITEM_PICKUP':
            col.prop(props, "item_id")
            col.prop(props, "item_type")
        elif comp_type == 'TRIGGER_ZONE':
            col.prop(props, "trigger_event")
        elif comp_type == 'LIGHT' and obj.type == 'LIGHT':
            col.prop(obj.data, "energy", text="Intensity")
            col.prop(obj.data, "color")
            col.prop(props, "cast_shadows")
        elif comp_type == 'CAMERA' and obj.type == 'CAMERA':
            col.prop(obj.data, "lens", text="Focal Length")
            col.prop(props, "is_main_camera")
        elif comp_type == 'CHARACTER':
            col.prop(props, "character_id")
            col.prop(props, "max_health")
            col.prop(props, "move_speed")
        elif comp_type == 'AUDIO_SOURCE':
            col.prop(props, "audio_clip")
            col.prop(props, "audio_volume")
            col.prop(props, "audio_loop")
        elif comp_type == 'PARTICLE_EMITTER':
            col.prop(props, "particle_type")
            col.prop(props, "emission_rate")

    # ===== Assets Tab =====
    def draw_assets_tab(self, layout, context):
        import os
        settings = context.scene.skope_project

        project_path = self.get_project_path(context)

        if not project_path:
            layout.label(text="No project path set", icon='ERROR')
            return

        assets_path = os.path.join(project_path, settings.assets_subfolder or "assets/models")

        # Filter
        row = layout.row(align=True)
        row.prop(context.scene, "skope_asset_filter", text="")
        row.operator("skope.refresh_assets", text="", icon='FILE_REFRESH')

        layout.separator()

        if not os.path.exists(assets_path):
            layout.label(text="Assets folder not found", icon='ERROR')
            layout.operator("skope.create_assets_folder", text="Create Folder", icon='NEWFOLDER')
            return

        # Scan assets
        filter_type = getattr(context.scene, 'skope_asset_filter', 'ALL')
        assets = self.scan_assets(assets_path, filter_type)

        if not assets:
            layout.label(text="No assets found", icon='INFO')
            return

        # Grid display
        flow = layout.column_flow(columns=2, align=True)
        for asset in assets[:16]:
            icon_map = {'.glb': 'MESH_DATA', '.gltf': 'MESH_DATA', '.png': 'IMAGE_DATA', '.jpg': 'IMAGE_DATA', '.wav': 'SOUND'}
            icon = icon_map.get(asset['ext'], 'FILE')
            op = flow.operator("skope.import_asset", text=asset['name'][:12], icon=icon)
            op.filepath = asset['path']
            op.asset_name = asset['name']

        if len(assets) > 16:
            layout.label(text=f"... +{len(assets) - 16} more")

        layout.separator()
        layout.label(text=f"{len(assets)} assets", icon='ASSET_MANAGER')

    def get_project_path(self, context):
        import os
        settings = context.scene.skope_project

        env_path = os.environ.get('SKOPE_PROJECT_PATH')
        if env_path and os.path.isdir(env_path):
            return env_path

        if settings.project_path:
            path = bpy.path.abspath(settings.project_path)
            if os.path.isdir(path):
                return path

        if bpy.data.filepath:
            blend_dir = os.path.dirname(bpy.path.abspath(bpy.data.filepath))
            if os.path.exists(os.path.join(blend_dir, 'Cargo.toml')):
                return blend_dir

        return None

    def scan_assets(self, assets_path, filter_type):
        import os
        assets = []
        extensions = {
            'ALL': ('.glb', '.gltf', '.fbx', '.obj', '.png', '.jpg', '.wav', '.ogg'),
            'MODELS': ('.glb', '.gltf', '.fbx', '.obj'),
            'TEXTURES': ('.png', '.jpg', '.jpeg', '.tga'),
            'AUDIO': ('.wav', '.ogg', '.mp3'),
        }
        allowed = extensions.get(filter_type, extensions['ALL'])

        for root, dirs, files in os.walk(assets_path):
            for file in files:
                if file.lower().endswith(allowed):
                    full_path = os.path.join(root, file)
                    name, ext = os.path.splitext(file)
                    assets.append({'name': name, 'ext': ext.lower(), 'path': full_path})

        assets.sort(key=lambda x: x['name'].lower())
        return assets

    # ===== Project Tab =====
    def draw_project_tab(self, layout, context):
        settings = context.scene.skope_project

        # Project path
        col = layout.column(align=True)
        col.label(text="Project Path", icon='FILE_FOLDER')
        col.prop(settings, "project_path", text="")

        layout.separator()

        # Export options
        col = layout.column(align=True)
        col.label(text="Export Settings", icon='EXPORT')
        col.prop(settings, "auto_export_gltf")
        col.prop(settings, "export_textures")

        layout.separator()

        # Paths
        col = layout.column(align=True)
        col.label(text="Folders", icon='FILEBROWSER')
        col.prop(settings, "assets_subfolder", text="Assets")
        col.prop(settings, "levels_subfolder", text="Levels")

        layout.separator()

        # Export buttons
        col = layout.column(align=True)
        col.scale_y = 1.3
        col.operator("skope.export_all", text="Export All", icon='PACKAGE')
        row = col.row(align=True)
        row.operator("skope.export_scene", text="Scene", icon='SCENE_DATA')
        row.operator("skope.export_selected_gltf", text="Selected", icon='MESH_DATA')


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

    def draw_header(self, context):
        self.layout.label(text="", icon='OUTLINER')

    def draw(self, context):
        layout = self.layout
        scene = context.scene

        # Play controls at top - bigger and more prominent
        box = layout.box()
        row = box.row(align=True)
        row.scale_y = 1.8

        # Play button (green-ish)
        play_op = row.operator("skope.play", text="  Play  ", icon='PLAY')

        # Stop button
        stop_op = row.operator("skope.play", text="", icon='SNAP_FACE')
        stop_op.action = 'STOP'

        # Export quick button
        row.operator("skope.export_all", text="", icon='EXPORT')

        layout.separator()

        # Search and filters in one row
        row = layout.row(align=True)
        row.prop(scene, "skope_hierarchy_filter", text="", icon='VIEWZOOM')
        row.prop(scene, "skope_filter_components_only", text="", icon='FILTER', toggle=True)

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

    def draw_header(self, context):
        self.layout.label(text="", icon='PREFERENCES')

    def draw(self, context):
        layout = self.layout
        obj = context.object

        if obj is None:
            box = layout.box()
            box.label(text="Select an object", icon='HAND')
            return

        # Object info header - compact
        header_box = layout.box()
        header_row = header_box.row(align=True)

        # Type icon
        type_icons = {
            'MESH': 'MESH_CUBE',
            'LIGHT': 'LIGHT',
            'CAMERA': 'CAMERA_DATA',
            'ARMATURE': 'ARMATURE_DATA',
            'EMPTY': 'EMPTY_DATA',
            'CURVE': 'CURVE_DATA',
        }
        icon = type_icons.get(obj.type, 'OBJECT_DATA')
        header_row.label(text="", icon=icon)

        # Editable name
        header_row.prop(obj, "name", text="")
        header_row.prop(obj, "hide_viewport", text="", icon='HIDE_OFF' if not obj.hide_viewport else 'HIDE_ON', emboss=False)

        # Transform - collapsible style
        self.draw_transform(layout, obj)

        # Components
        self.draw_components(layout, obj, context)

    def draw_transform(self, layout, obj):
        """Draw transform section - compact Unity/Unreal style"""
        box = layout.box()
        col = box.column(align=True)

        # Header
        row = col.row()
        row.label(text="Transform", icon='ORIENTATION_GLOBAL')

        # Position - single row with XYZ
        split = col.split(factor=0.25, align=True)
        split.label(text="Pos")
        row = split.row(align=True)
        row.prop(obj, "location", index=0, text="X")
        row.prop(obj, "location", index=1, text="Y")
        row.prop(obj, "location", index=2, text="Z")

        # Rotation
        split = col.split(factor=0.25, align=True)
        split.label(text="Rot")
        row = split.row(align=True)
        row.prop(obj, "rotation_euler", index=0, text="X")
        row.prop(obj, "rotation_euler", index=1, text="Y")
        row.prop(obj, "rotation_euler", index=2, text="Z")

        # Scale
        split = col.split(factor=0.25, align=True)
        split.label(text="Scale")
        row = split.row(align=True)
        row.prop(obj, "scale", index=0, text="X")
        row.prop(obj, "scale", index=1, text="Y")
        row.prop(obj, "scale", index=2, text="Z")

    def draw_components(self, layout, obj, context):
        """Draw component section"""
        props = obj.skope_component
        comp_type = props.component_type

        # Component type selector - styled header
        box = layout.box()
        header = box.row()

        # Component icon based on type
        comp_icons = {
            'NONE': 'ADD',
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
        icon = comp_icons.get(comp_type, 'MODIFIER')

        header.label(text="Component", icon=icon)
        header.prop(props, "component_type", text="")

        # Component-specific properties
        if comp_type == 'NONE':
            col = box.column()
            col.label(text="Add a component to make", icon='INFO')
            col.label(text="this object interactive")
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
    bl_label = "Scene"
    bl_idname = "SKOPE_PT_scene_overview"
    bl_space_type = 'VIEW_3D'
    bl_region_type = 'UI'
    bl_category = "SKOPE"
    bl_options = {'DEFAULT_CLOSED'}
    bl_order = 2

    def draw_header(self, context):
        self.layout.label(text="", icon='SCENE_DATA')

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
    bl_options = {'DEFAULT_CLOSED'}
    bl_order = 3

    def draw_header(self, context):
        self.layout.label(text="", icon='FILE_FOLDER')

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


# ============ Asset Browser Panel ============

class SKOPE_PT_AssetBrowser(Panel):
    """Asset Browser - Browse and import project assets"""
    bl_label = "Assets"
    bl_idname = "SKOPE_PT_asset_browser"
    bl_space_type = 'VIEW_3D'
    bl_region_type = 'UI'
    bl_category = "SKOPE"
    bl_options = {'DEFAULT_CLOSED'}
    bl_order = 4

    def draw_header(self, context):
        self.layout.label(text="", icon='ASSET_MANAGER')

    def draw(self, context):
        layout = self.layout
        settings = context.scene.skope_project

        # Get project path
        project_path = self.get_project_path(context)

        if not project_path:
            box = layout.box()
            box.label(text="No project path set", icon='ERROR')
            box.label(text="Set path in Project panel")
            return

        import os
        assets_path = os.path.join(project_path, settings.assets_subfolder or "assets/models")

        # Header with refresh button
        row = layout.row()
        row.label(text="Assets", icon='FILE_FOLDER')
        row.operator("skope.refresh_assets", text="", icon='FILE_REFRESH')

        # Asset type filter
        row = layout.row(align=True)
        row.prop(context.scene, "skope_asset_filter", text="")

        layout.separator()

        # List assets
        if not os.path.exists(assets_path):
            box = layout.box()
            box.label(text=f"Folder not found:", icon='ERROR')
            box.label(text=assets_path[:40] + "...")
            box.operator("skope.create_assets_folder", text="Create Folder", icon='NEWFOLDER')
            return

        # Scan for assets
        filter_type = getattr(context.scene, 'skope_asset_filter', 'ALL')
        assets = self.scan_assets(assets_path, filter_type)

        if not assets:
            box = layout.box()
            box.label(text="No assets found", icon='INFO')
            box.label(text="Export from Blender or add glTF files")
            return

        # Display assets in grid
        box = layout.box()
        flow = box.column_flow(columns=2, align=True)

        for asset in assets[:20]:  # Limit to 20 items
            self.draw_asset_item(flow, asset, project_path)

        if len(assets) > 20:
            box.label(text=f"... and {len(assets) - 20} more", icon='THREE_DOTS')

        # Footer
        layout.label(text=f"{len(assets)} assets", icon='MESH_DATA')

    def get_project_path(self, context):
        """Get project path from various sources"""
        import os
        settings = context.scene.skope_project

        # 1. Environment variable
        env_path = os.environ.get('SKOPE_PROJECT_PATH')
        if env_path and os.path.isdir(env_path):
            return env_path

        # 2. Scene settings
        if settings.project_path:
            path = bpy.path.abspath(settings.project_path)
            if os.path.isdir(path):
                return path

        # 3. Infer from .blend file
        if bpy.data.filepath:
            blend_dir = os.path.dirname(bpy.path.abspath(bpy.data.filepath))
            cargo_toml = os.path.join(blend_dir, 'Cargo.toml')
            if os.path.exists(cargo_toml):
                return blend_dir

        return None

    def scan_assets(self, assets_path, filter_type):
        """Scan assets folder for supported files"""
        import os
        assets = []

        extensions = {
            'ALL': ('.glb', '.gltf', '.fbx', '.obj', '.png', '.jpg', '.jpeg', '.wav', '.ogg', '.mp3'),
            'MODELS': ('.glb', '.gltf', '.fbx', '.obj'),
            'TEXTURES': ('.png', '.jpg', '.jpeg', '.tga', '.bmp'),
            'AUDIO': ('.wav', '.ogg', '.mp3'),
        }

        allowed_ext = extensions.get(filter_type, extensions['ALL'])

        for root, dirs, files in os.walk(assets_path):
            for file in files:
                if file.lower().endswith(allowed_ext):
                    full_path = os.path.join(root, file)
                    rel_path = os.path.relpath(full_path, assets_path)
                    name, ext = os.path.splitext(file)
                    assets.append({
                        'name': name,
                        'ext': ext.lower(),
                        'path': full_path,
                        'rel_path': rel_path,
                    })

        # Sort by name
        assets.sort(key=lambda x: x['name'].lower())
        return assets

    def draw_asset_item(self, layout, asset, project_path):
        """Draw a single asset item"""
        # Icon based on type
        icon_map = {
            '.glb': 'MESH_DATA',
            '.gltf': 'MESH_DATA',
            '.fbx': 'MESH_DATA',
            '.obj': 'MESH_DATA',
            '.png': 'IMAGE_DATA',
            '.jpg': 'IMAGE_DATA',
            '.jpeg': 'IMAGE_DATA',
            '.tga': 'IMAGE_DATA',
            '.wav': 'SOUND',
            '.ogg': 'SOUND',
            '.mp3': 'SOUND',
        }
        icon = icon_map.get(asset['ext'], 'FILE')

        # Button to import
        row = layout.row(align=True)
        op = row.operator("skope.import_asset", text=asset['name'][:15], icon=icon)
        op.filepath = asset['path']
        op.asset_name = asset['name']


class SKOPE_OT_RefreshAssets(bpy.types.Operator):
    """Refresh asset list"""
    bl_idname = "skope.refresh_assets"
    bl_label = "Refresh Assets"

    def execute(self, context):
        # Force panel redraw
        for area in context.screen.areas:
            if area.type == 'VIEW_3D':
                area.tag_redraw()
        self.report({'INFO'}, "Asset list refreshed")
        return {'FINISHED'}


class SKOPE_OT_CreateAssetsFolder(bpy.types.Operator):
    """Create assets folder"""
    bl_idname = "skope.create_assets_folder"
    bl_label = "Create Assets Folder"

    def execute(self, context):
        import os
        settings = context.scene.skope_project
        project_path = os.environ.get('SKOPE_PROJECT_PATH') or bpy.path.abspath(settings.project_path)

        if not project_path:
            self.report({'ERROR'}, "No project path set")
            return {'CANCELLED'}

        assets_path = os.path.join(project_path, settings.assets_subfolder or "assets/models")

        try:
            os.makedirs(assets_path, exist_ok=True)
            self.report({'INFO'}, f"Created: {assets_path}")
        except Exception as e:
            self.report({'ERROR'}, f"Failed to create folder: {e}")
            return {'CANCELLED'}

        return {'FINISHED'}


class SKOPE_OT_ImportAsset(bpy.types.Operator):
    """Import asset into scene"""
    bl_idname = "skope.import_asset"
    bl_label = "Import Asset"
    bl_options = {'REGISTER', 'UNDO'}

    filepath: bpy.props.StringProperty(name="File Path")
    asset_name: bpy.props.StringProperty(name="Asset Name")

    def execute(self, context):
        import os

        if not os.path.exists(self.filepath):
            self.report({'ERROR'}, f"File not found: {self.filepath}")
            return {'CANCELLED'}

        ext = os.path.splitext(self.filepath)[1].lower()

        try:
            if ext in ('.glb', '.gltf'):
                bpy.ops.import_scene.gltf(filepath=self.filepath)
                self.report({'INFO'}, f"Imported: {self.asset_name}")

            elif ext in ('.fbx',):
                bpy.ops.import_scene.fbx(filepath=self.filepath)
                self.report({'INFO'}, f"Imported: {self.asset_name}")

            elif ext in ('.obj',):
                bpy.ops.wm.obj_import(filepath=self.filepath)
                self.report({'INFO'}, f"Imported: {self.asset_name}")

            elif ext in ('.png', '.jpg', '.jpeg', '.tga', '.bmp'):
                # Load image and create image plane or just load
                bpy.data.images.load(self.filepath, check_existing=True)
                self.report({'INFO'}, f"Loaded image: {self.asset_name}")

            elif ext in ('.wav', '.ogg', '.mp3'):
                bpy.data.sounds.load(self.filepath, check_existing=True)
                self.report({'INFO'}, f"Loaded sound: {self.asset_name}")

            else:
                self.report({'WARNING'}, f"Unsupported format: {ext}")
                return {'CANCELLED'}

            # Set imported object as StaticProp by default
            if ext in ('.glb', '.gltf', '.fbx', '.obj'):
                for obj in context.selected_objects:
                    if obj.type == 'MESH':
                        obj.skope_component.component_type = 'STATIC_PROP'
                        obj.skope_component.mesh_name = self.asset_name

        except Exception as e:
            self.report({'ERROR'}, f"Import failed: {e}")
            return {'CANCELLED'}

        return {'FINISHED'}


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
    # Main unified panel (tabbed)
    SKOPE_PT_Main,

    # Asset operators
    SKOPE_OT_RefreshAssets,
    SKOPE_OT_CreateAssetsFolder,
    SKOPE_OT_ImportAsset,

    # Properties panel (for Object Properties area)
    SKOPE_PT_ComponentPanel,
]


def register():
    """Register panel classes"""
    for cls in classes:
        try:
            bpy.utils.register_class(cls)
        except ValueError:
            pass

    # Tab selector property
    if not hasattr(bpy.types.Scene, 'skope_editor_tab'):
        bpy.types.Scene.skope_editor_tab = bpy.props.EnumProperty(
            name="Editor Tab",
            description="Current SKOPE Editor tab",
            items=[
                ('HIERARCHY', 'Hierarchy', 'Scene hierarchy', 'OUTLINER', 0),
                ('INSPECTOR', 'Inspector', 'Object inspector', 'PREFERENCES', 1),
                ('ASSETS', 'Assets', 'Asset browser', 'ASSET_MANAGER', 2),
                ('PROJECT', 'Project', 'Project settings', 'FILE_FOLDER', 3),
            ],
            default='HIERARCHY',
        )

    # Scene properties for hierarchy
    if not hasattr(bpy.types.Scene, 'skope_hierarchy_filter'):
        bpy.types.Scene.skope_hierarchy_filter = bpy.props.StringProperty(
            name="Filter",
            description="Filter objects by name",
            default="",
        )
    if not hasattr(bpy.types.Scene, 'skope_filter_components_only'):
        bpy.types.Scene.skope_filter_components_only = bpy.props.BoolProperty(
            name="Components Only",
            description="Show only objects with SKOPE components",
            default=False,
        )
    if not hasattr(bpy.types.Scene, 'skope_asset_filter'):
        bpy.types.Scene.skope_asset_filter = bpy.props.EnumProperty(
            name="Asset Filter",
            description="Filter assets by type",
            items=[
                ('ALL', 'All', 'Show all assets'),
                ('MODELS', 'Models', 'Show only 3D models (glTF, FBX, OBJ)'),
                ('TEXTURES', 'Textures', 'Show only textures (PNG, JPG)'),
                ('AUDIO', 'Audio', 'Show only audio files (WAV, OGG, MP3)'),
            ],
            default='ALL',
        )


def unregister():
    """Unregister panel classes"""
    if hasattr(bpy.types.Scene, 'skope_editor_tab'):
        del bpy.types.Scene.skope_editor_tab
    if hasattr(bpy.types.Scene, 'skope_hierarchy_filter'):
        del bpy.types.Scene.skope_hierarchy_filter
    if hasattr(bpy.types.Scene, 'skope_filter_components_only'):
        del bpy.types.Scene.skope_filter_components_only
    if hasattr(bpy.types.Scene, 'skope_asset_filter'):
        del bpy.types.Scene.skope_asset_filter

    for cls in reversed(classes):
        try:
            bpy.utils.unregister_class(cls)
        except:
            pass
