"""
SKOPE Operators
Export operators for converting Blender scenes to .skope format
"""

import bpy
from bpy.types import Operator
from bpy_extras.io_utils import ExportHelper


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

class SKOPE_OT_ExportScene(Operator, ExportHelper):
    """Export scene to SKOPE format (.skope)"""
    bl_idname = "skope.export_scene"
    bl_label = "Export SKOPE Scene"
    bl_options = {'REGISTER', 'UNDO'}

    # File extension
    filename_ext = ".skope"
    filter_glob: bpy.props.StringProperty(
        default="*.skope",
        options={'HIDDEN'},
    )

    def execute(self, context):
        """Execute export operation"""
        filepath = self.filepath

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

# ============ Registration ============

classes = [
    SKOPE_OT_AutoFillMeshName,
    SKOPE_OT_ExportScene,
]

def register():
    """Register operator classes"""
    for cls in classes:
        bpy.utils.register_class(cls)

def unregister():
    """Unregister operator classes"""
    for cls in reversed(classes):
        bpy.utils.unregister_class(cls)
