"""
SKOPE Operators
Export operators for converting Blender scenes to .skope format
Phase 9: Added glTF export and one-click export
"""

import bpy
import os
from bpy.types import Operator
from bpy_extras.io_utils import ExportHelper


# ============ Helper Functions ============

def get_project_paths(context):
    """Get project paths from scene settings"""
    settings = context.scene.skope_project
    if not settings.project_path:
        return None, None, None

    project_root = bpy.path.abspath(settings.project_path)
    assets_path = os.path.join(project_root, settings.assets_subfolder)
    levels_path = os.path.join(project_root, settings.levels_subfolder)

    return project_root, assets_path, levels_path


def ensure_directory(path):
    """Create directory if it doesn't exist"""
    if not os.path.exists(path):
        os.makedirs(path)
        return True
    return False


def get_objects_to_export(context):
    """Get list of objects with SKOPE components that need glTF export"""
    objects_to_export = []

    for obj in context.scene.objects:
        props = obj.skope_component
        if props.component_type == 'STATIC_PROP':
            # Check if object has mesh data
            if obj.type == 'MESH' and obj.data:
                objects_to_export.append(obj)

    return objects_to_export


# ============ Helper Operators ============

class SKOPE_OT_AutoFillMeshName(Operator):
    """Auto-fill mesh name from object's mesh data"""
    bl_idname = "skope.auto_fill_mesh_name"
    bl_label = "Auto-fill from Object Mesh"
    bl_options = {'REGISTER', 'UNDO'}

    def execute(self, context):
        obj = context.object
        if obj is None:
            self.report({'WARNING'}, "No object selected")
            return {'CANCELLED'}

        props = obj.skope_component

        if obj.data and hasattr(obj.data, 'name'):
            props.mesh_name = obj.data.name
            self.report({'INFO'}, f"Set mesh name to: {obj.data.name}")
            return {'FINISHED'}
        else:
            self.report({'WARNING'}, "Object has no mesh data")
            return {'CANCELLED'}


# ============ Export Operator ============

class SKOPE_OT_ExportScene(Operator):
    """Export scene to SKOPE format (.skope) - uses project settings"""
    bl_idname = "skope.export_scene"
    bl_label = "Export SKOPE Scene"
    bl_options = {'REGISTER', 'UNDO'}

    def execute(self, context):
        """Execute export operation"""
        # Use project path settings
        settings = context.scene.skope_project
        project_root, _, levels_path = get_project_paths(context)

        if not project_root:
            self.report({'ERROR'}, "Project path not configured!")
            return {'CANCELLED'}

        ensure_directory(levels_path)

        scene_name = bpy.path.clean_name(context.scene.name)
        filepath = os.path.join(levels_path, f"{scene_name}.skope")

        # Collect entities from scene
        entities = []

        for obj in context.scene.objects:
            props = obj.skope_component
            comp_type = props.component_type

            if comp_type == 'NONE':
                continue  # Skip objects without components

            # Build entity data
            entity_data = self.build_entity_data(obj, props, comp_type)
            if entity_data:
                entities.append(entity_data)

        # Generate .skope file content
        skope_content = self.generate_skope_content(entities)

        # Write to file
        with open(filepath, 'w') as f:
            f.write(skope_content)

        self.report({'INFO'}, f"Exported {len(entities)} entities to {filepath}")
        return {'FINISHED'}

    def build_entity_data(self, obj, props, comp_type):
        """Build entity data dictionary"""
        return {
            'name': obj.name,
            'position': tuple(obj.location),
            'rotation': tuple(obj.rotation_euler),
            'scale': tuple(obj.scale),
            'component_type': comp_type,
            'props': props,
            'obj': obj,
        }

    def generate_skope_content(self, entities):
        """Generate RON format .skope file content"""
        lines = [
            "// SKOPE Scene exported from Blender",
            "// Format: RON (Rusty Object Notation)",
            "",
            "(",
            "    entities: [",
        ]

        for entity in entities:
            lines.append("        (")
            lines.append(f"            name: \"{entity['name']}\",")

            # Position
            pos = entity['position']
            lines.append(f"            position: (x: {pos[0]:.6f}, y: {pos[1]:.6f}, z: {pos[2]:.6f}),")

            # Rotation (Euler angles in radians)
            rot = entity['rotation']
            lines.append(f"            rotation: (x: {rot[0]:.6f}, y: {rot[1]:.6f}, z: {rot[2]:.6f}),")

            # Scale
            scale = entity['scale']
            lines.append(f"            scale: (x: {scale[0]:.6f}, y: {scale[1]:.6f}, z: {scale[2]:.6f}),")

            # Component (RON enum format)
            comp_lines = self.generate_component_ron(entity)
            for line in comp_lines:
                lines.append(line)

            lines.append("        ),")

        lines.append("    ],")
        lines.append(")")

        return "\n".join(lines)

    def generate_component_ron(self, entity):
        """Generate component RON based on type"""
        comp_type = entity['component_type']
        props = entity['props']
        obj = entity['obj']
        lines = []

        if comp_type == 'PLAYER_SPAWN':
            lines.append("            component: PlayerSpawn,")

        elif comp_type == 'ENEMY_SPAWNER':
            lines.append("            component: EnemySpawner(")
            lines.append(f"                enemy_type: \"{props.enemy_type}\",")
            lines.append(f"                enemy_count: {props.enemy_count},")
            lines.append(f"                enemy_respawn: {str(props.enemy_respawn).lower()},")
            lines.append("            ),")

        elif comp_type == 'STATIC_PROP':
            lines.append("            component: StaticProp(")
            lines.append(f"                has_collision: {str(props.has_collision).lower()},")

            # Use explicit mesh_name if provided, otherwise fall back to object's mesh data name
            mesh_name = None
            if props.mesh_name and props.mesh_name.strip():
                mesh_name = props.mesh_name.strip()
            elif obj.data and hasattr(obj.data, 'name'):
                mesh_name = obj.data.name

            if mesh_name:
                lines.append(f"                mesh: Some(\"{mesh_name}\"),")
            else:
                lines.append("                mesh: None,")
            lines.append("            ),")

        elif comp_type == 'COLLIDER':
            lines.append("            component: Collider(")
            lines.append(f"                collider_shape: {props.collider_shape},")  # BOX/SPHERE/MESH
            lines.append(f"                is_trigger: {str(props.is_trigger).lower()},")
            lines.append("            ),")

        elif comp_type == 'ITEM_PICKUP':
            lines.append("            component: ItemPickup(")
            lines.append(f"                item_id: \"{props.item_id}\",")
            lines.append(f"                item_type: {props.item_type},")  # WEAPON/GRIMOIRE/CONSUMABLE
            lines.append("            ),")

        elif comp_type == 'TRIGGER_ZONE':
            lines.append("            component: TriggerZone(")
            lines.append(f"                trigger_event: \"{props.trigger_event}\",")
            lines.append("            ),")

        elif comp_type == 'LIGHT':
            if obj.type == 'LIGHT':
                # Map Blender light types to RON enum
                light_type_map = {
                    'POINT': 'Point',
                    'SPOT': 'Spot',
                    'SUN': 'Sun',
                    'AREA': 'Area',
                }
                ron_light_type = light_type_map.get(obj.data.type, 'Point')

                color = obj.data.color
                lines.append("            component: Light(")
                lines.append(f"                light_type: {ron_light_type},")
                lines.append(f"                light_energy: {obj.data.energy:.6f},")
                lines.append(f"                light_color: ({color[0]:.6f}, {color[1]:.6f}, {color[2]:.6f}),")
                lines.append("            ),")
            else:
                lines.append("            component: PlayerSpawn,  // Error: Light component on non-LIGHT object")

        return lines


# ============ glTF Export Operators ============

class SKOPE_OT_ExportSelectedGltf(Operator):
    """Export selected objects as glTF to project assets folder"""
    bl_idname = "skope.export_selected_gltf"
    bl_label = "Export Selected as glTF"
    bl_options = {'REGISTER', 'UNDO'}

    def execute(self, context):
        settings = context.scene.skope_project
        project_root, assets_path, _ = get_project_paths(context)

        if not project_root:
            self.report({'ERROR'}, "Project path not configured!")
            return {'CANCELLED'}

        # Ensure assets directory exists
        ensure_directory(assets_path)

        # Get selected mesh objects
        selected_meshes = [obj for obj in context.selected_objects if obj.type == 'MESH']

        if not selected_meshes:
            self.report({'WARNING'}, "No mesh objects selected")
            return {'CANCELLED'}

        exported_count = 0

        for obj in selected_meshes:
            # Use object name for file
            filename = f"{obj.name}.glb"
            filepath = os.path.join(assets_path, filename)

            # Deselect all, select only this object
            bpy.ops.object.select_all(action='DESELECT')
            obj.select_set(True)
            context.view_layer.objects.active = obj

            # Export as glTF
            bpy.ops.export_scene.gltf(
                filepath=filepath,
                use_selection=True,
                export_format='GLB',
                export_texcoords=True,
                export_normals=True,
                export_materials='EXPORT' if settings.export_textures else 'NONE',
                export_yup=True,
            )

            # Update mesh_name in component if it's a StaticProp
            props = obj.skope_component
            if props.component_type == 'STATIC_PROP':
                props.mesh_name = obj.name  # Will match the glTF filename

            exported_count += 1
            print(f"[SKOPE] Exported: {filepath}")

        # Restore selection
        for obj in selected_meshes:
            obj.select_set(True)

        self.report({'INFO'}, f"Exported {exported_count} objects to {assets_path}")
        return {'FINISHED'}


class SKOPE_OT_ExportAll(Operator):
    """One-click export: glTF meshes + .skope scene file"""
    bl_idname = "skope.export_all"
    bl_label = "Export All (glTF + Scene)"
    bl_options = {'REGISTER', 'UNDO'}

    def execute(self, context):
        settings = context.scene.skope_project
        project_root, assets_path, levels_path = get_project_paths(context)

        if not project_root:
            self.report({'ERROR'}, "Project path not configured!")
            return {'CANCELLED'}

        # Ensure directories exist
        ensure_directory(assets_path)
        ensure_directory(levels_path)

        exported_meshes = 0
        exported_entities = 0

        # ===== Step 1: Export glTF meshes =====
        if settings.auto_export_gltf:
            objects_to_export = get_objects_to_export(context)

            # Track unique meshes (avoid duplicating same mesh data)
            exported_mesh_names = set()

            for obj in objects_to_export:
                mesh_name = obj.data.name if obj.data else obj.name

                # Skip if we already exported this mesh data
                if mesh_name in exported_mesh_names:
                    # Just update the component's mesh_name reference
                    props = obj.skope_component
                    if props.component_type == 'STATIC_PROP':
                        props.mesh_name = mesh_name
                    continue

                filename = f"{mesh_name}.glb"
                filepath = os.path.join(assets_path, filename)

                # Deselect all, select only this object
                bpy.ops.object.select_all(action='DESELECT')
                obj.select_set(True)
                context.view_layer.objects.active = obj

                # Export as glTF
                try:
                    bpy.ops.export_scene.gltf(
                        filepath=filepath,
                        use_selection=True,
                        export_format='GLB',
                        export_texcoords=True,
                        export_normals=True,
                        export_materials='EXPORT' if settings.export_textures else 'NONE',
                        export_yup=True,
                    )

                    exported_mesh_names.add(mesh_name)
                    exported_meshes += 1
                    print(f"[SKOPE] Exported mesh: {filepath}")

                    # Update mesh_name in component
                    props = obj.skope_component
                    if props.component_type == 'STATIC_PROP':
                        props.mesh_name = mesh_name

                except Exception as e:
                    print(f"[SKOPE] Failed to export {obj.name}: {e}")

        # ===== Step 2: Export .skope scene file =====
        scene_name = bpy.path.clean_name(context.scene.name)
        skope_filepath = os.path.join(levels_path, f"{scene_name}.skope")

        # Collect entities
        entities = []
        for obj in context.scene.objects:
            props = obj.skope_component
            comp_type = props.component_type

            if comp_type == 'NONE':
                continue

            entity_data = {
                'name': obj.name,
                'position': tuple(obj.location),
                'rotation': tuple(obj.rotation_euler),
                'scale': tuple(obj.scale),
                'component_type': comp_type,
                'props': props,
                'obj': obj,
            }
            entities.append(entity_data)
            exported_entities += 1

        # Generate and write .skope content
        skope_content = self.generate_skope_content(entities)
        with open(skope_filepath, 'w') as f:
            f.write(skope_content)

        print(f"[SKOPE] Exported scene: {skope_filepath}")

        # Deselect all
        bpy.ops.object.select_all(action='DESELECT')

        self.report({'INFO'},
            f"Exported {exported_meshes} meshes + {exported_entities} entities")
        return {'FINISHED'}

    def generate_skope_content(self, entities):
        """Generate RON format .skope file content"""
        lines = [
            "// SKOPE Scene exported from Blender",
            "// Format: RON (Rusty Object Notation)",
            "",
            "(",
            "    entities: [",
        ]

        for entity in entities:
            lines.append("        (")
            lines.append(f"            name: \"{entity['name']}\",")

            pos = entity['position']
            lines.append(f"            position: (x: {pos[0]:.6f}, y: {pos[1]:.6f}, z: {pos[2]:.6f}),")

            rot = entity['rotation']
            lines.append(f"            rotation: (x: {rot[0]:.6f}, y: {rot[1]:.6f}, z: {rot[2]:.6f}),")

            scale = entity['scale']
            lines.append(f"            scale: (x: {scale[0]:.6f}, y: {scale[1]:.6f}, z: {scale[2]:.6f}),")

            comp_lines = self.generate_component_ron(entity)
            for line in comp_lines:
                lines.append(line)

            lines.append("        ),")

        lines.append("    ],")
        lines.append(")")

        return "\n".join(lines)

    def generate_component_ron(self, entity):
        """Generate component RON based on type (same as ExportScene)"""
        comp_type = entity['component_type']
        props = entity['props']
        obj = entity['obj']
        lines = []

        if comp_type == 'PLAYER_SPAWN':
            lines.append("            component: PlayerSpawn,")

        elif comp_type == 'ENEMY_SPAWNER':
            lines.append("            component: EnemySpawner(")
            lines.append(f"                enemy_type: \"{props.enemy_type}\",")
            lines.append(f"                enemy_count: {props.enemy_count},")
            lines.append(f"                enemy_respawn: {str(props.enemy_respawn).lower()},")
            lines.append("            ),")

        elif comp_type == 'STATIC_PROP':
            lines.append("            component: StaticProp(")
            lines.append(f"                has_collision: {str(props.has_collision).lower()},")

            mesh_name = None
            if props.mesh_name and props.mesh_name.strip():
                mesh_name = props.mesh_name.strip()
            elif obj.data and hasattr(obj.data, 'name'):
                mesh_name = obj.data.name

            if mesh_name:
                lines.append(f"                mesh: Some(\"{mesh_name}\"),")
            else:
                lines.append("                mesh: None,")
            lines.append("            ),")

        elif comp_type == 'COLLIDER':
            lines.append("            component: Collider(")
            lines.append(f"                collider_shape: {props.collider_shape},")
            lines.append(f"                is_trigger: {str(props.is_trigger).lower()},")
            lines.append("            ),")

        elif comp_type == 'ITEM_PICKUP':
            lines.append("            component: ItemPickup(")
            lines.append(f"                item_id: \"{props.item_id}\",")
            lines.append(f"                item_type: {props.item_type},")
            lines.append("            ),")

        elif comp_type == 'TRIGGER_ZONE':
            lines.append("            component: TriggerZone(")
            lines.append(f"                trigger_event: \"{props.trigger_event}\",")
            lines.append("            ),")

        elif comp_type == 'LIGHT':
            if obj.type == 'LIGHT':
                light_type_map = {
                    'POINT': 'Point',
                    'SPOT': 'Spot',
                    'SUN': 'Sun',
                    'AREA': 'Area',
                }
                ron_light_type = light_type_map.get(obj.data.type, 'Point')
                color = obj.data.color
                lines.append("            component: Light(")
                lines.append(f"                light_type: {ron_light_type},")
                lines.append(f"                light_energy: {obj.data.energy:.6f},")
                lines.append(f"                light_color: ({color[0]:.6f}, {color[1]:.6f}, {color[2]:.6f}),")
                lines.append("            ),")
            else:
                lines.append("            component: PlayerSpawn,  // Error: Light on non-LIGHT object")

        return lines


# ============ Registration ============

classes = [
    SKOPE_OT_AutoFillMeshName,
    SKOPE_OT_ExportScene,
    SKOPE_OT_ExportSelectedGltf,
    SKOPE_OT_ExportAll,
]

def register():
    """Register operator classes"""
    for cls in classes:
        bpy.utils.register_class(cls)

def unregister():
    """Unregister operator classes"""
    for cls in reversed(classes):
        bpy.utils.unregister_class(cls)
