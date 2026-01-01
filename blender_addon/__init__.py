"""
SKOPE Blender Addon
Export Blender scenes to SKOPE Engine (.skope format)
With Qt-based Unity/Unreal-style editor interface (via BQT)
"""

bl_info = {
    "name": "SKOPE Editor",
    "author": "SKOPE Games",
    "version": (0, 4, 0),
    "blender": (4, 0, 0),
    "location": "3D Viewport Header + SKOPE Menu + Qt Editor",
    "description": "Game editor integration for SKOPE Engine - Unity/Unreal style Qt interface",
    "category": "Game Engine",
}

import bpy
import sys
import os
from pathlib import Path

# Add PySide6 lib path (installed in addon's lib folder)
addon_path = Path(__file__).parent
pyside6_lib = addon_path / "lib"
if pyside6_lib.exists() and str(pyside6_lib) not in sys.path:
    sys.path.insert(0, str(pyside6_lib))

# Add BQT to path
addon_dir = addon_path.parent
bqt_path = addon_dir / "bqt_lib"
if str(bqt_path) not in sys.path:
    sys.path.insert(0, str(bqt_path))

from . import properties
from . import panels
from . import operators
from . import workspace
from . import live_link

# Module registration
modules = [
    properties,
    panels,
    operators,
    workspace,
    live_link,
]

# BQT initialization flag
_bqt_initialized = False


class SKOPE_OT_LaunchQtEditor(bpy.types.Operator):
    """Launch SKOPE Qt Editor Window (Standalone Process)"""
    bl_idname = "skope.launch_qt_editor"
    bl_label = "Open SKOPE Editor"
    bl_description = "Open the Qt-based SKOPE Editor as a separate window"

    _editor_process = None
    _sync_timer = None

    def execute(self, context):
        import subprocess

        # Find standalone editor script
        addon_path = Path(__file__).parent
        editor_script = addon_path / "qt_editor" / "standalone.py"

        if not editor_script.exists():
            self.report({'ERROR'}, f"Editor script not found: {editor_script}")
            return {'CANCELLED'}

        # Check if already running
        if SKOPE_OT_LaunchQtEditor._editor_process is not None:
            if SKOPE_OT_LaunchQtEditor._editor_process.poll() is None:
                self.report({'INFO'}, "SKOPE Editor already running")
                return {'FINISHED'}

        # Get Python from addon's lib (with PySide6)
        lib_path = addon_path / "lib"

        # Launch as separate process using system Python
        try:
            env = os.environ.copy()
            env["PYTHONPATH"] = str(lib_path) + ":" + env.get("PYTHONPATH", "")

            SKOPE_OT_LaunchQtEditor._editor_process = subprocess.Popen(
                ["python3", str(editor_script)],
                env=env,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL
            )

            # Start scene sync
            start_scene_sync()

            self.report({'INFO'}, "SKOPE Editor launched!")
        except Exception as e:
            self.report({'ERROR'}, f"Failed to launch: {e}")
            return {'CANCELLED'}

        return {'FINISHED'}


def sync_scene_to_file():
    """Sync Blender scene to JSON file for Qt Editor"""
    import time
    sync_dir = Path.home() / ".config/skope"
    sync_dir.mkdir(parents=True, exist_ok=True)
    sync_file = sync_dir / "scene_sync.json"

    try:
        import json
        from . import operators
        scene = bpy.context.scene

        # Get selected object name
        selected = ""
        if bpy.context.active_object:
            selected = bpy.context.active_object.name

        # Check engine running status
        engine_running = False
        if hasattr(operators, 'SKOPE_OT_Play'):
            proc = operators.SKOPE_OT_Play._process
            engine_running = proc is not None and proc.poll() is None

        objects = []
        for obj in scene.objects:
            obj_data = {
                "name": obj.name,
                "type": obj.type,
                "parent": obj.parent.name if obj.parent else None,
                "visible": not obj.hide_viewport,
                "location": list(obj.location),
                "rotation": list(obj.rotation_euler),
                "scale": list(obj.scale),
            }
            # Add component type if exists
            if hasattr(obj, 'skope_component'):
                obj_data["component_type"] = obj.skope_component.component_type
            objects.append(obj_data)

        with open(sync_file, 'w') as f:
            json.dump({
                "objects": objects,
                "selected": selected,
                "engine_running": engine_running,
                "timestamp": time.time(),
            }, f)

    except Exception as e:
        print(f"[SKOPE] Scene sync error: {e}")

    return 0.3  # Repeat every 0.3 seconds


def write_command_result(success: bool, message: str):
    """Write command result for Qt Editor feedback"""
    result_file = Path.home() / ".config/skope/command_result.json"
    try:
        import json
        with open(result_file, 'w') as f:
            json.dump({"success": success, "message": message}, f)
    except Exception:
        pass


def check_editor_commands():
    """Check for commands from Qt Editor"""
    cmd_file = Path.home() / ".config/skope/commands.json"

    if not cmd_file.exists():
        return 0.1

    result_success = True
    result_message = "OK"

    try:
        import json
        with open(cmd_file) as f:
            cmd = json.load(f)

        command = cmd.get("command", "")

        # Selection
        if command == "select":
            obj_name = cmd.get("object")
            if obj_name and obj_name in bpy.context.scene.objects:
                bpy.ops.object.select_all(action='DESELECT')
                obj = bpy.context.scene.objects[obj_name]
                obj.select_set(True)
                bpy.context.view_layer.objects.active = obj
            else:
                result_success = False
                result_message = f"Object not found: {obj_name}"

        # Focus (zoom to object)
        elif command == "focus":
            obj_name = cmd.get("object")
            if obj_name and obj_name in bpy.context.scene.objects:
                bpy.ops.object.select_all(action='DESELECT')
                obj = bpy.context.scene.objects[obj_name]
                obj.select_set(True)
                bpy.context.view_layer.objects.active = obj
                for area in bpy.context.screen.areas:
                    if area.type == 'VIEW_3D':
                        for region in area.regions:
                            if region.type == 'WINDOW':
                                with bpy.context.temp_override(area=area, region=region):
                                    bpy.ops.view3d.view_selected()
                                break
            else:
                result_success = False
                result_message = f"Object not found: {obj_name}"

        # Transform commands
        elif command == "set_location":
            obj_name = cmd.get("object")
            value = cmd.get("value", [0, 0, 0])
            if obj_name and obj_name in bpy.context.scene.objects:
                obj = bpy.context.scene.objects[obj_name]
                obj.location = value
            else:
                result_success = False
                result_message = f"Object not found: {obj_name}"

        elif command == "set_rotation":
            obj_name = cmd.get("object")
            value = cmd.get("value", [0, 0, 0])
            if obj_name and obj_name in bpy.context.scene.objects:
                obj = bpy.context.scene.objects[obj_name]
                obj.rotation_euler = value
            else:
                result_success = False
                result_message = f"Object not found: {obj_name}"

        elif command == "set_scale":
            obj_name = cmd.get("object")
            value = cmd.get("value", [1, 1, 1])
            if obj_name and obj_name in bpy.context.scene.objects:
                obj = bpy.context.scene.objects[obj_name]
                obj.scale = value
            else:
                result_success = False
                result_message = f"Object not found: {obj_name}"

        # Rename
        elif command == "rename":
            obj_name = cmd.get("object")
            new_name = cmd.get("new_name")
            if obj_name and new_name and obj_name in bpy.context.scene.objects:
                obj = bpy.context.scene.objects[obj_name]
                obj.name = new_name
                result_message = f"Renamed to: {new_name}"
            else:
                result_success = False
                result_message = f"Object not found: {obj_name}"

        # Duplicate
        elif command == "duplicate":
            obj_name = cmd.get("object")
            if obj_name and obj_name in bpy.context.scene.objects:
                bpy.ops.object.select_all(action='DESELECT')
                obj = bpy.context.scene.objects[obj_name]
                obj.select_set(True)
                bpy.context.view_layer.objects.active = obj
                bpy.ops.object.duplicate()
                result_message = f"Duplicated: {obj_name}"
            else:
                result_success = False
                result_message = f"Object not found: {obj_name}"

        # Delete
        elif command == "delete":
            obj_name = cmd.get("object")
            if obj_name and obj_name in bpy.context.scene.objects:
                bpy.ops.object.select_all(action='DESELECT')
                obj = bpy.context.scene.objects[obj_name]
                obj.select_set(True)
                bpy.ops.object.delete()
                result_message = f"Deleted: {obj_name}"
            else:
                result_success = False
                result_message = f"Object not found: {obj_name}"

        # Add objects
        elif command == "add":
            obj_type = cmd.get("type", "EMPTY")
            if obj_type == "EMPTY":
                bpy.ops.object.empty_add()
            elif obj_type == "CUBE":
                bpy.ops.mesh.primitive_cube_add()
            elif obj_type == "SPHERE":
                bpy.ops.mesh.primitive_uv_sphere_add()
            elif obj_type == "CYLINDER":
                bpy.ops.mesh.primitive_cylinder_add()
            elif obj_type == "PLANE":
                bpy.ops.mesh.primitive_plane_add()
            elif obj_type == "LIGHT" or obj_type == "POINT_LIGHT":
                bpy.ops.object.light_add(type='POINT')
            elif obj_type == "SUN_LIGHT":
                bpy.ops.object.light_add(type='SUN')
            elif obj_type == "SPOT_LIGHT":
                bpy.ops.object.light_add(type='SPOT')
            elif obj_type == "CAMERA":
                bpy.ops.object.camera_add()
            result_message = f"Added: {obj_type}"

        # Import model
        elif command == "import":
            filepath = cmd.get("path", "")
            if filepath and os.path.exists(filepath):
                ext = os.path.splitext(filepath)[1].lower()
                if ext in ['.glb', '.gltf']:
                    bpy.ops.import_scene.gltf(filepath=filepath)
                elif ext == '.fbx':
                    bpy.ops.import_scene.fbx(filepath=filepath)
                elif ext == '.obj':
                    bpy.ops.wm.obj_import(filepath=filepath)
                result_message = f"Imported: {os.path.basename(filepath)}"
                print(f"[SKOPE] Imported: {filepath}")
            else:
                result_success = False
                result_message = f"File not found: {filepath}"

        # Set component
        elif command == "set_component":
            obj_name = cmd.get("object")
            comp_type = cmd.get("component")
            if obj_name and comp_type and obj_name in bpy.context.scene.objects:
                obj = bpy.context.scene.objects[obj_name]
                if hasattr(obj, 'skope_component'):
                    obj.skope_component.component_type = comp_type
                    result_message = f"Component set: {comp_type}"
            else:
                result_success = False
                result_message = f"Object not found: {obj_name}"

        # Set parent
        elif command == "set_parent":
            child_name = cmd.get("child")
            parent_name = cmd.get("parent")  # None means unparent
            if child_name and child_name in bpy.context.scene.objects:
                child_obj = bpy.context.scene.objects[child_name]
                if parent_name and parent_name in bpy.context.scene.objects:
                    parent_obj = bpy.context.scene.objects[parent_name]
                    child_obj.parent = parent_obj
                    # Keep transform
                    child_obj.matrix_parent_inverse = parent_obj.matrix_world.inverted()
                    result_message = f"Parented to: {parent_name}"
                else:
                    child_obj.parent = None
                    result_message = f"Unparented: {child_name}"
            else:
                result_success = False
                result_message = f"Object not found: {child_name}"

        # Set visibility
        elif command == "set_visible":
            obj_name = cmd.get("object")
            visible = cmd.get("visible", True)
            if obj_name and obj_name in bpy.context.scene.objects:
                obj = bpy.context.scene.objects[obj_name]
                obj.hide_viewport = not visible
                obj.hide_render = not visible
            else:
                result_success = False
                result_message = f"Object not found: {obj_name}"

        # Transform tools
        elif command == "tool":
            tool_type = cmd.get("type", "")
            if tool_type == "MOVE":
                bpy.ops.transform.translate('INVOKE_DEFAULT')
            elif tool_type == "ROTATE":
                bpy.ops.transform.rotate('INVOKE_DEFAULT')
            elif tool_type == "SCALE":
                bpy.ops.transform.resize('INVOKE_DEFAULT')

        # Undo/Redo
        elif command == "undo":
            bpy.ops.ed.undo()
        elif command == "redo":
            bpy.ops.ed.redo()

        # File operations
        elif command == "save":
            bpy.ops.wm.save_mainfile()
            result_message = "File saved"
        elif command == "open":
            bpy.ops.wm.open_mainfile('INVOKE_DEFAULT')
        elif command == "new_scene":
            bpy.ops.wm.read_homefile()
            result_message = "New scene created"

        # Play/Stop
        elif command == "play":
            try:
                bpy.ops.skope.play()
            except Exception as e:
                result_success = False
                result_message = f"Play failed: {e}"
        elif command == "stop":
            try:
                bpy.ops.skope.play(action='STOP')
            except Exception as e:
                result_success = False
                result_message = f"Stop failed: {e}"

        # Export
        elif command == "export":
            try:
                bpy.ops.skope.export_all()
                result_message = "Export completed"
            except Exception as e:
                result_success = False
                result_message = f"Export failed: {e}"

        # Write result for major operations (skip silent ones like select/focus)
        if command in ("rename", "duplicate", "delete", "add", "import", "set_component",
                       "set_parent", "save", "new_scene", "play", "stop", "export"):
            write_command_result(result_success, result_message)

        # Clear command file
        cmd_file.unlink()

    except Exception as e:
        print(f"[SKOPE] Command error: {e}")
        write_command_result(False, str(e))
        try:
            cmd_file.unlink()
        except:
            pass

    return 0.1  # Check every 0.1 seconds


_sync_started = False


def start_scene_sync():
    """Start scene sync timers"""
    global _sync_started
    if not _sync_started:
        bpy.app.timers.register(sync_scene_to_file, first_interval=0.1)
        bpy.app.timers.register(check_editor_commands, first_interval=0.2)
        _sync_started = True
        print("[SKOPE] Scene sync started")


def register():
    """Register all addon classes"""
    for module in modules:
        module.register()

    # Register Qt editor operator
    bpy.utils.register_class(SKOPE_OT_LaunchQtEditor)

    print("SKOPE Editor addon registered - Use SKOPE menu to open Qt Editor!")


def unregister():
    """Unregister all addon classes"""
    try:
        bpy.utils.unregister_class(SKOPE_OT_LaunchQtEditor)
    except:
        pass

    for module in reversed(modules):
        module.unregister()

    print("SKOPE Editor addon unregistered")


if __name__ == "__main__":
    register()
