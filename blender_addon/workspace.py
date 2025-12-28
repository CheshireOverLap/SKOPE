"""
SKOPE Workspace Setup
Creates Unity/Unreal-style editor layout automatically

Layout:
┌──────────┬─────────────────────────┬──────────┐
│ SKOPE    │                         │PROPERTIES│
│ Hierarchy│      3D VIEWPORT        │          │
│ (N-Panel)│                         │          │
├──────────┴─────────────────────────┴──────────┤
│              TIMELINE / DOPE SHEET            │
└───────────────────────────────────────────────┘
"""

import bpy
from bpy.app.handlers import persistent

WORKSPACE_NAME = "SKOPE"


def create_skope_workspace():
    """Create SKOPE workspace with game editor layout"""

    # Check if already exists
    if WORKSPACE_NAME in bpy.data.workspaces:
        return bpy.data.workspaces[WORKSPACE_NAME]

    # Duplicate Layout workspace as base
    try:
        # Find Layout workspace to duplicate
        base_ws = None
        for ws in bpy.data.workspaces:
            if ws.name == "Layout":
                base_ws = ws
                break

        if base_ws is None:
            base_ws = bpy.context.workspace

        # Create new workspace
        bpy.ops.workspace.duplicate({'workspace': base_ws})

        # Find the newly created workspace and rename it
        for ws in bpy.data.workspaces:
            if ws.name.startswith("Layout.") or ws.name == "Layout.001":
                ws.name = WORKSPACE_NAME
                return ws

        return bpy.context.workspace

    except Exception as e:
        print(f"[SKOPE] Could not create workspace: {e}")
        return bpy.context.workspace


class SKOPE_OT_SetupLayout(bpy.types.Operator):
    """Setup SKOPE editor layout - Unity/Unreal style"""
    bl_idname = "skope.setup_layout"
    bl_label = "Setup SKOPE Layout"
    bl_options = {'REGISTER'}

    def execute(self, context):
        screen = context.screen

        # Find 3D View area
        view3d_area = None
        for area in screen.areas:
            if area.type == 'VIEW_3D':
                view3d_area = area
                break

        if not view3d_area:
            self.report({'ERROR'}, "No 3D View found")
            return {'CANCELLED'}

        # Step 1: Split 3D View horizontally (create bottom area for timeline)
        try:
            with context.temp_override(area=view3d_area):
                bpy.ops.screen.area_split(direction='HORIZONTAL', factor=0.8)
        except Exception as e:
            print(f"[SKOPE] Horizontal split failed: {e}")

        # Step 2: Find the new bottom area and change to DOPESHEET
        for area in screen.areas:
            # The bottom area after split
            if area.type == 'VIEW_3D' and area.y < view3d_area.y:
                area.type = 'DOPESHEET_EDITOR'
                break

        # Step 3: Find main 3D View again and split vertically (create left panel)
        view3d_area = None
        for area in screen.areas:
            if area.type == 'VIEW_3D':
                view3d_area = area
                break

        if view3d_area:
            try:
                with context.temp_override(area=view3d_area):
                    bpy.ops.screen.area_split(direction='VERTICAL', factor=0.2)
            except Exception as e:
                print(f"[SKOPE] Vertical split (left) failed: {e}")

        # Step 4: The left area becomes Outliner
        for area in screen.areas:
            if area.type == 'VIEW_3D' and area.width < 400:
                area.type = 'OUTLINER'
                break

        # Step 5: Split right side for Properties
        view3d_area = None
        for area in screen.areas:
            if area.type == 'VIEW_3D':
                view3d_area = area
                break

        if view3d_area:
            try:
                with context.temp_override(area=view3d_area):
                    bpy.ops.screen.area_split(direction='VERTICAL', factor=0.75)
            except Exception as e:
                print(f"[SKOPE] Vertical split (right) failed: {e}")

        # Step 6: The right area becomes Properties
        # Find the rightmost VIEW_3D and change to PROPERTIES
        rightmost_view3d = None
        max_x = 0
        for area in screen.areas:
            if area.type == 'VIEW_3D' and area.x > max_x:
                max_x = area.x
                rightmost_view3d = area

        if rightmost_view3d and len([a for a in screen.areas if a.type == 'VIEW_3D']) > 1:
            rightmost_view3d.type = 'PROPERTIES'

        # Step 7: Open N-Panel in remaining 3D View(s) and set to SKOPE tab
        for area in screen.areas:
            if area.type == 'VIEW_3D':
                for space in area.spaces:
                    if space.type == 'VIEW_3D':
                        space.show_region_ui = True  # Open N-Panel

        self.report({'INFO'}, "SKOPE Layout applied!")
        return {'FINISHED'}


def switch_to_skope_workspace():
    """Switch to SKOPE workspace"""
    if WORKSPACE_NAME in bpy.data.workspaces:
        bpy.context.window.workspace = bpy.data.workspaces[WORKSPACE_NAME]
        return True
    return False


@persistent
def on_load_post(dummy):
    """Called after a blend file is loaded"""
    # Create SKOPE workspace if it doesn't exist
    if WORKSPACE_NAME not in bpy.data.workspaces:
        # Use timer to delay workspace creation
        bpy.app.timers.register(delayed_workspace_setup, first_interval=0.5)


def delayed_workspace_setup():
    """Delayed workspace setup (called by timer)"""
    try:
        if WORKSPACE_NAME not in bpy.data.workspaces:
            create_skope_workspace()
            print(f"[SKOPE] Created '{WORKSPACE_NAME}' workspace")
    except Exception as e:
        print(f"[SKOPE] Workspace creation error: {e}")

    # Return None to not repeat the timer
    return None


_skope_menu_draw_func = None

def draw_skope_menu(self, context):
    layout = self.layout
    layout.separator()
    layout.menu("SKOPE_MT_main_menu", text="SKOPE")

def add_skope_menu():
    """Add SKOPE menu to top bar"""
    global _skope_menu_draw_func
    if _skope_menu_draw_func is None:
        _skope_menu_draw_func = draw_skope_menu
        bpy.types.TOPBAR_MT_editor_menus.append(_skope_menu_draw_func)

def remove_skope_menu():
    """Remove SKOPE menu from top bar"""
    global _skope_menu_draw_func
    if _skope_menu_draw_func is not None:
        try:
            bpy.types.TOPBAR_MT_editor_menus.remove(_skope_menu_draw_func)
        except:
            pass
        _skope_menu_draw_func = None


class SKOPE_MT_MainMenu(bpy.types.Menu):
    """SKOPE main menu"""
    bl_idname = "SKOPE_MT_main_menu"
    bl_label = "SKOPE"

    def draw(self, context):
        layout = self.layout

        # Qt Editor (main feature)
        layout.operator("skope.launch_qt_editor", text="Open SKOPE Editor", icon='WINDOW')

        layout.separator()

        # Play controls
        layout.operator("skope.play", text="Play", icon='PLAY')
        layout.operator("skope.play", text="Stop", icon='SNAP_FACE').action = 'STOP'

        layout.separator()

        # Layout
        layout.operator("skope.setup_layout", text="Apply Blender Layout", icon='WORKSPACE')

        layout.separator()

        # Export
        layout.operator("skope.export_all", text="Export All", icon='EXPORT')
        layout.operator("skope.export_scene", text="Export Scene", icon='SCENE_DATA')


class SKOPE_OT_SwitchWorkspace(bpy.types.Operator):
    """Switch to SKOPE workspace layout"""
    bl_idname = "skope.switch_workspace"
    bl_label = "Switch to SKOPE Layout"
    bl_options = {'REGISTER'}

    def execute(self, context):
        if WORKSPACE_NAME not in bpy.data.workspaces:
            create_skope_workspace()

        switch_to_skope_workspace()
        self.report({'INFO'}, f"Switched to {WORKSPACE_NAME} workspace")
        return {'FINISHED'}


# Play button in header
def draw_play_button(self, context):
    """Draw play button in 3D View header"""
    if context.area.type == 'VIEW_3D':
        layout = self.layout
        row = layout.row(align=True)
        row.separator()
        row.operator("skope.play", text="", icon='PLAY')
        row.operator("skope.play", text="", icon='SNAP_FACE').action = 'STOP'


classes = [
    SKOPE_MT_MainMenu,
    SKOPE_OT_SetupLayout,
    SKOPE_OT_SwitchWorkspace,
]


def register():
    """Register workspace classes and handlers"""
    for cls in classes:
        bpy.utils.register_class(cls)

    # Add load handler
    if on_load_post not in bpy.app.handlers.load_post:
        bpy.app.handlers.load_post.append(on_load_post)

    # Add menu
    add_skope_menu()

    # Add play button to 3D View header
    bpy.types.VIEW3D_HT_header.append(draw_play_button)

    # Create workspace on first run (delayed)
    bpy.app.timers.register(delayed_workspace_setup, first_interval=1.0)


def unregister():
    """Unregister workspace classes and handlers"""
    # Remove play button from header
    try:
        bpy.types.VIEW3D_HT_header.remove(draw_play_button)
    except:
        pass

    # Remove SKOPE menu
    remove_skope_menu()

    # Remove load handler
    if on_load_post in bpy.app.handlers.load_post:
        bpy.app.handlers.load_post.remove(on_load_post)

    for cls in reversed(classes):
        try:
            bpy.utils.unregister_class(cls)
        except:
            pass
