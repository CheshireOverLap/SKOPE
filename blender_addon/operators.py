"""
SKOPE Operators
Export operators for converting Blender scenes to .skope format
"""

import bpy
from bpy.types import Operator
from bpy_extras.io_utils import ExportHelper
import os

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
        entity = {
            'name': obj.name,
            'type': comp_type,
            'position': tuple(obj.location),
            'rotation': tuple(obj.rotation_euler),
            'scale': tuple(obj.scale),
        }

        # Add component-specific data
        if comp_type == 'ENEMY_SPAWNER':
            entity['enemy_type'] = props.enemy_type
            entity['enemy_count'] = props.enemy_count
            entity['enemy_respawn'] = props.enemy_respawn

        elif comp_type == 'STATIC_PROP':
            entity['has_collision'] = props.has_collision
            if obj.data and hasattr(obj.data, 'name'):
                entity['mesh'] = obj.data.name

        elif comp_type == 'COLLIDER':
            entity['collider_shape'] = props.collider_shape
            entity['is_trigger'] = props.is_trigger

        elif comp_type == 'ITEM_PICKUP':
            entity['item_id'] = props.item_id
            entity['item_type'] = props.item_type

        elif comp_type == 'TRIGGER_ZONE':
            entity['trigger_event'] = props.trigger_event

        elif comp_type == 'LIGHT':
            if obj.type == 'LIGHT':
                entity['light_type'] = obj.data.type
                entity['light_energy'] = obj.data.energy
                entity['light_color'] = tuple(obj.data.color)

        return entity

    def generate_skope_content(self, entities):
        """Generate RON format .skope file content"""
        lines = []
        lines.append("// SKOPE Scene exported from Blender")
        lines.append("// Auto-generated - do not edit manually\n")
        lines.append("Scene(")
        lines.append("    entities: [")

        for entity in entities:
            lines.append("        Entity(")
            lines.append(f"            name: \"{entity['name']}\",")
            lines.append(f"            component_type: \"{entity['type']}\",")

            # Transform
            pos = entity['position']
            lines.append(f"            position: Vec3(x: {pos[0]:.3f}, y: {pos[1]:.3f}, z: {pos[2]:.3f}),")

            # Component-specific fields
            if entity['type'] == 'ENEMY_SPAWNER':
                lines.append(f"            enemy_type: \"{entity['enemy_type']}\",")
                lines.append(f"            enemy_count: {entity['enemy_count']},")
                lines.append(f"            enemy_respawn: {str(entity['enemy_respawn']).lower()},")

            elif entity['type'] == 'STATIC_PROP':
                lines.append(f"            has_collision: {str(entity['has_collision']).lower()},")
                if 'mesh' in entity:
                    lines.append(f"            mesh: \"{entity['mesh']}\",")

            elif entity['type'] == 'COLLIDER':
                lines.append(f"            collider_shape: \"{entity['collider_shape']}\",")
                lines.append(f"            is_trigger: {str(entity['is_trigger']).lower()},")

            elif entity['type'] == 'ITEM_PICKUP':
                lines.append(f"            item_id: \"{entity['item_id']}\",")
                lines.append(f"            item_type: \"{entity['item_type']}\",")

            elif entity['type'] == 'TRIGGER_ZONE':
                lines.append(f"            trigger_event: \"{entity['trigger_event']}\",")

            lines.append("        ),")

        lines.append("    ],")
        lines.append(")")

        return "\n".join(lines)

# ============ Registration ============

classes = [
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
