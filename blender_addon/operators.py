"""
SKOPE Operators
Export operators and game editor controls
- Export: Scene to .skope, meshes to glTF
- Play: Launch SKOPE engine with current scene
- Hierarchy: Object selection and management
"""

import bpy
import os
import subprocess
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
        if props.component_type in {'STATIC_PROP', 'CHARACTER'}:
            if obj.type == 'MESH' and obj.data:
                objects_to_export.append(obj)
            elif obj.type == 'ARMATURE':
                # Include armature and its mesh children
                objects_to_export.append(obj)

    return objects_to_export


# ============ Play/Stop Operators ============

class SKOPE_OT_Play(Operator):
    """Launch SKOPE engine with current scene"""
    bl_idname = "skope.play"
    bl_label = "Play"
    bl_options = {'REGISTER'}

    action: bpy.props.EnumProperty(
        name="Action",
        items=[
            ('PLAY', 'Play', 'Start the game'),
            ('STOP', 'Stop', 'Stop the game'),
        ],
        default='PLAY',
    )

    # Class variable to track running process
    _process = None

    def execute(self, context):
        settings = context.scene.skope_project
        project_root, assets_path, levels_path = get_project_paths(context)

        if self.action == 'STOP':
            # Stop running engine
            if SKOPE_OT_Play._process is not None:
                SKOPE_OT_Play._process.terminate()
                SKOPE_OT_Play._process = None
                self.report({'INFO'}, "SKOPE Engine stopped")
            return {'FINISHED'}

        # Play action
        if not project_root:
            self.report({'ERROR'}, "Project path not configured!")
            return {'CANCELLED'}

        # Auto-export scene before play
        ensure_directory(levels_path)
        scene_name = bpy.path.clean_name(context.scene.name)
        scene_file = os.path.join(levels_path, f"{scene_name}.skope")

        # Export scene
        bpy.ops.skope.export_scene()

        # Find engine executable
        engine_path = None

        # 1. Check project settings
        if settings.engine_path and os.path.exists(bpy.path.abspath(settings.engine_path)):
            engine_path = bpy.path.abspath(settings.engine_path)
        else:
            # 2. Check common locations
            possible_paths = [
                os.path.join(project_root, "target", "debug", "SKOPE"),
                os.path.join(project_root, "target", "release", "SKOPE"),
                os.path.join(project_root, "SKOPE"),
                os.path.join(project_root, "target", "debug", "SKOPE.exe"),
                os.path.join(project_root, "target", "release", "SKOPE.exe"),
            ]

            for path in possible_paths:
                if os.path.exists(path):
                    engine_path = path
                    break

        if not engine_path:
            self.report({'WARNING'}, "Engine executable not found. Building with cargo...")
            # Try to build
            try:
                result = subprocess.run(
                    ["cargo", "build"],
                    cwd=project_root,
                    capture_output=True,
                    text=True,
                    timeout=120,
                )
                if result.returncode == 0:
                    engine_path = os.path.join(project_root, "target", "debug", "SKOPE")
                else:
                    self.report({'ERROR'}, f"Build failed: {result.stderr[:200]}")
                    return {'CANCELLED'}
            except Exception as e:
                self.report({'ERROR'}, f"Failed to build engine: {e}")
                return {'CANCELLED'}

        # Launch engine
        try:
            env = os.environ.copy()
            env["SKOPE_LEVEL"] = scene_file

            SKOPE_OT_Play._process = subprocess.Popen(
                [engine_path],
                cwd=project_root,
                env=env,
            )

            self.report({'INFO'}, f"SKOPE Engine launched: {scene_name}")
            return {'FINISHED'}

        except Exception as e:
            self.report({'ERROR'}, f"Failed to launch engine: {e}")
            return {'CANCELLED'}


# ============ Hierarchy Operators ============

class SKOPE_OT_SelectObject(Operator):
    """Select object from hierarchy"""
    bl_idname = "skope.select_object"
    bl_label = "Select Object"
    bl_options = {'REGISTER', 'UNDO'}

    object_name: bpy.props.StringProperty(
        name="Object Name",
        default="",
    )

    def execute(self, context):
        obj = context.scene.objects.get(self.object_name)
        if obj is None:
            self.report({'WARNING'}, f"Object not found: {self.object_name}")
            return {'CANCELLED'}

        # Deselect all and select this object
        bpy.ops.object.select_all(action='DESELECT')
        obj.select_set(True)
        context.view_layer.objects.active = obj

        return {'FINISHED'}


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


# ============ Export Operators ============

class SKOPE_OT_ExportScene(Operator):
    """Export scene to SKOPE format (.skope) - uses project settings"""
    bl_idname = "skope.export_scene"
    bl_label = "Export SKOPE Scene"
    bl_options = {'REGISTER', 'UNDO'}

    def execute(self, context):
        """Execute export operation"""
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
                continue

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

            mesh_name = props.mesh_name.strip() if props.mesh_name else None
            if not mesh_name and obj.data and hasattr(obj.data, 'name'):
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
                    'SUN': 'Directional',
                    'AREA': 'Area',
                }
                ron_light_type = light_type_map.get(obj.data.type, 'Point')
                color = obj.data.color

                lines.append("            component: Light(")
                lines.append(f"                light_type: {ron_light_type},")
                lines.append(f"                intensity: {obj.data.energy:.6f},")
                lines.append(f"                color: ({color[0]:.6f}, {color[1]:.6f}, {color[2]:.6f}),")
                lines.append(f"                cast_shadows: {str(props.cast_shadows).lower()},")
                lines.append(f"                shadow_bias: {props.shadow_bias:.6f},")

                if obj.data.type == 'SPOT':
                    lines.append(f"                inner_angle: {obj.data.spot_size * 0.8:.6f},")
                    lines.append(f"                outer_angle: {obj.data.spot_size:.6f},")

                if obj.data.type in {'POINT', 'SPOT'}:
                    lines.append(f"                radius: {getattr(obj.data, 'shadow_soft_size', 10.0):.6f},")

                lines.append("            ),")
            else:
                lines.append("            component: PlayerSpawn,  // Error: Light on non-LIGHT object")

        elif comp_type == 'CAMERA':
            if obj.type == 'CAMERA':
                cam = obj.data
                lines.append("            component: Camera(")
                lines.append(f"                camera_type: {props.camera_type},")
                lines.append(f"                is_main: {str(props.is_main_camera).lower()},")
                lines.append(f"                fov: {cam.lens:.6f},")
                lines.append(f"                near_clip: {cam.clip_start:.6f},")
                lines.append(f"                far_clip: {cam.clip_end:.6f},")
                lines.append("            ),")
            else:
                lines.append("            component: PlayerSpawn,  // Error: Camera on non-CAMERA object")

        elif comp_type == 'CHARACTER':
            lines.append("            component: Character(")
            lines.append(f"                character_id: \"{props.character_id}\",")
            lines.append(f"                character_type: {props.character_type},")
            lines.append(f"                team: {props.team},")
            lines.append(f"                max_health: {props.max_health:.1f},")
            lines.append(f"                move_speed: {props.move_speed:.1f},")
            lines.append("            ),")

        elif comp_type == 'AUDIO_SOURCE':
            lines.append("            component: AudioSource(")
            lines.append(f"                clip: \"{props.audio_clip}\",")
            lines.append(f"                volume: {props.audio_volume:.3f},")
            lines.append(f"                loop_audio: {str(props.audio_loop).lower()},")
            lines.append(f"                spatial: {str(props.audio_spatial).lower()},")
            if props.audio_spatial:
                lines.append(f"                min_distance: {props.audio_min_distance:.1f},")
                lines.append(f"                max_distance: {props.audio_max_distance:.1f},")
            lines.append("            ),")

        elif comp_type == 'PARTICLE_EMITTER':
            lines.append("            component: ParticleEmitter(")
            lines.append(f"                particle_type: {props.particle_type},")
            lines.append(f"                emission_rate: {props.emission_rate:.1f},")
            lines.append(f"                lifetime: {props.particle_lifetime:.2f},")
            lines.append("            ),")

        return lines


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

        ensure_directory(assets_path)

        selected_meshes = [obj for obj in context.selected_objects if obj.type in {'MESH', 'ARMATURE'}]

        if not selected_meshes:
            self.report({'WARNING'}, "No mesh/armature objects selected")
            return {'CANCELLED'}

        exported_count = 0

        for obj in selected_meshes:
            filename = f"{obj.name}.glb"
            filepath = os.path.join(assets_path, filename)

            bpy.ops.object.select_all(action='DESELECT')
            obj.select_set(True)
            context.view_layer.objects.active = obj

            bpy.ops.export_scene.gltf(
                filepath=filepath,
                use_selection=True,
                export_format='GLB',
                export_texcoords=True,
                export_normals=True,
                export_materials='EXPORT' if settings.export_textures else 'NONE',
                export_yup=True,
            )

            props = obj.skope_component
            if props.component_type in {'STATIC_PROP', 'CHARACTER'}:
                props.mesh_name = obj.name

            exported_count += 1
            print(f"[SKOPE] Exported: {filepath}")

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

        ensure_directory(assets_path)
        ensure_directory(levels_path)

        exported_meshes = 0
        exported_entities = 0

        # ===== Step 1: Export glTF meshes =====
        if settings.auto_export_gltf:
            objects_to_export = get_objects_to_export(context)
            exported_mesh_names = set()

            for obj in objects_to_export:
                mesh_name = obj.data.name if obj.data else obj.name

                if mesh_name in exported_mesh_names:
                    props = obj.skope_component
                    if props.component_type in {'STATIC_PROP', 'CHARACTER'}:
                        props.mesh_name = mesh_name
                    continue

                filename = f"{mesh_name}.glb"
                filepath = os.path.join(assets_path, filename)

                bpy.ops.object.select_all(action='DESELECT')
                obj.select_set(True)
                context.view_layer.objects.active = obj

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

                    props = obj.skope_component
                    if props.component_type in {'STATIC_PROP', 'CHARACTER'}:
                        props.mesh_name = mesh_name

                except Exception as e:
                    print(f"[SKOPE] Failed to export {obj.name}: {e}")

        # ===== Step 2: Export .skope scene file =====
        scene_name = bpy.path.clean_name(context.scene.name)
        skope_filepath = os.path.join(levels_path, f"{scene_name}.skope")

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

        exporter = SKOPE_OT_ExportScene()
        skope_content = exporter.generate_skope_content(entities)
        with open(skope_filepath, 'w') as f:
            f.write(skope_content)

        print(f"[SKOPE] Exported scene: {skope_filepath}")

        bpy.ops.object.select_all(action='DESELECT')

        self.report({'INFO'}, f"Exported {exported_meshes} meshes + {exported_entities} entities")
        return {'FINISHED'}


# ============ Registration ============

classes = [
    SKOPE_OT_Play,
    SKOPE_OT_SelectObject,
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
