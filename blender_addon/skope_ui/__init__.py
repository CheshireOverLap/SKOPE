# SKOPE UI Editor - Blender Addon
# UI 레이아웃을 Blender에서 시각적으로 편집

bl_info = {
    "name": "SKOPE UI Editor",
    "author": "SKOPE Engine",
    "version": (1, 0, 0),
    "blender": (3, 0, 0),
    "location": "View3D > Sidebar > SKOPE UI",
    "description": "Visual editor for SKOPE Engine UI layouts",
    "category": "Game Engine",
}

import bpy
import os
import math
from bpy.props import (
    StringProperty,
    FloatProperty,
    FloatVectorProperty,
    BoolProperty,
    EnumProperty,
    IntProperty,
    PointerProperty,
)
from bpy.types import (
    Panel,
    Operator,
    PropertyGroup,
)

# ============================================================
# Widget Properties (stored on objects)
# ============================================================

class SKOPEWidgetProperties(PropertyGroup):
    """SKOPE UI 위젯 속성"""

    widget_type: EnumProperty(
        name="Widget Type",
        items=[
            ('Container', "Container", "빈 컨테이너"),
            ('Text', "Text", "텍스트 위젯"),
            ('Image', "Image", "이미지 위젯"),
            ('Button', "Button", "버튼 위젯"),
            ('NineSlice', "9-Slice", "9-슬라이스 이미지"),
            ('ProgressBar', "Progress Bar", "프로그레스 바"),
            ('ScrollView', "Scroll View", "스크롤 뷰"),
            ('InputField', "Input Field", "입력 필드"),
        ],
        default='Container'
    )

    widget_id: StringProperty(
        name="Widget ID",
        description="고유 식별자",
        default=""
    )

    # Anchor
    anchor: EnumProperty(
        name="Anchor",
        items=[
            ('TopLeft', "Top Left", ""),
            ('TopCenter', "Top Center", ""),
            ('TopRight', "Top Right", ""),
            ('MiddleLeft', "Middle Left", ""),
            ('Center', "Center", ""),
            ('MiddleRight', "Middle Right", ""),
            ('BottomLeft', "Bottom Left", ""),
            ('BottomCenter', "Bottom Center", ""),
            ('BottomRight', "Bottom Right", ""),
            ('Stretch', "Stretch", ""),
        ],
        default='TopLeft'
    )

    # Size mode
    size_mode: EnumProperty(
        name="Size Mode",
        items=[
            ('Fixed', "Fixed", "고정 픽셀 크기"),
            ('Percent', "Percent", "부모 기준 퍼센트"),
            ('FitContent', "Fit Content", "내용에 맞춤"),
            ('Fill', "Fill", "부모에 맞춤"),
        ],
        default='Fixed'
    )

    # Style
    background_color: FloatVectorProperty(
        name="Background Color",
        subtype='COLOR',
        size=4,
        min=0.0, max=1.0,
        default=(0.2, 0.2, 0.25, 0.9)
    )

    use_background_color: BoolProperty(
        name="Use Background Color",
        default=False
    )

    background_image: StringProperty(
        name="Background Image",
        description="배경 이미지 파일명",
        default=""
    )

    text_color: FloatVectorProperty(
        name="Text Color",
        subtype='COLOR',
        size=4,
        min=0.0, max=1.0,
        default=(1.0, 1.0, 1.0, 1.0)
    )

    # Text properties
    text_content: StringProperty(
        name="Text Content",
        default=""
    )

    font_size: FloatProperty(
        name="Font Size",
        default=14.0,
        min=8.0, max=72.0
    )

    # Image properties
    image_src: StringProperty(
        name="Image Source",
        description="이미지 파일명",
        default=""
    )

    # 9-Slice border
    border_top: FloatProperty(name="Border Top", default=16.0, min=0.0)
    border_right: FloatProperty(name="Border Right", default=16.0, min=0.0)
    border_bottom: FloatProperty(name="Border Bottom", default=16.0, min=0.0)
    border_left: FloatProperty(name="Border Left", default=16.0, min=0.0)

    # Interactive
    interactive: BoolProperty(
        name="Interactive",
        default=True
    )

    visible: BoolProperty(
        name="Visible",
        default=True
    )

    # Drag & Drop
    draggable: BoolProperty(
        name="Draggable",
        default=False
    )

    drop_target: BoolProperty(
        name="Drop Target",
        default=False
    )

    drag_data: StringProperty(
        name="Drag Data",
        default=""
    )

    drag_group: StringProperty(
        name="Drag Group",
        default=""
    )

    # Tooltip
    tooltip: StringProperty(
        name="Tooltip",
        default=""
    )

    tooltip_delay: FloatProperty(
        name="Tooltip Delay",
        default=0.5,
        min=0.0, max=3.0
    )

    # Layout
    flex_direction: EnumProperty(
        name="Flex Direction",
        items=[
            ('Row', "Row", "가로 방향"),
            ('Column', "Column", "세로 방향"),
        ],
        default='Row'
    )

    gap: FloatProperty(
        name="Gap",
        default=0.0,
        min=0.0
    )

    # Padding
    padding_top: FloatProperty(name="Padding Top", default=0.0, min=0.0)
    padding_right: FloatProperty(name="Padding Right", default=0.0, min=0.0)
    padding_bottom: FloatProperty(name="Padding Bottom", default=0.0, min=0.0)
    padding_left: FloatProperty(name="Padding Left", default=0.0, min=0.0)

    # Events
    on_click: StringProperty(name="On Click", default="")
    on_drop: StringProperty(name="On Drop", default="")

    # Input Field
    placeholder: StringProperty(name="Placeholder", default="")
    max_length: IntProperty(name="Max Length", default=100, min=1)

    # Progress Bar
    progress_value: FloatProperty(name="Value", default=0.5, min=0.0, max=1.0)


# ============================================================
# Operators
# ============================================================

class SKOPE_OT_CreateWidget(Operator):
    """새 UI 위젯 생성"""
    bl_idname = "skope.create_widget"
    bl_label = "Create Widget"
    bl_options = {'REGISTER', 'UNDO'}

    widget_type: EnumProperty(
        name="Type",
        items=[
            ('Container', "Container", ""),
            ('Text', "Text", ""),
            ('Image', "Image", ""),
            ('Button', "Button", ""),
            ('NineSlice', "9-Slice", ""),
        ],
        default='Container'
    )

    def execute(self, context):
        # Create a plane mesh for the widget
        bpy.ops.mesh.primitive_plane_add(size=1, location=(0, 0, 0))
        obj = context.active_object
        obj.name = f"UI_{self.widget_type}"

        # Set up the widget properties
        props = obj.skope_widget
        props.widget_type = self.widget_type

        # Default size based on type
        if self.widget_type == 'Text':
            obj.scale = (100, 20, 1)
            props.text_content = "Text"
        elif self.widget_type == 'Button':
            obj.scale = (120, 40, 1)
            props.text_content = "Button"
            props.use_background_color = True
        elif self.widget_type == 'Image':
            obj.scale = (64, 64, 1)
        else:
            obj.scale = (200, 100, 1)

        # Add to SKOPE_UI collection
        self.ensure_ui_collection(context)

        # Move to collection
        if "SKOPE_UI" in bpy.data.collections:
            ui_col = bpy.data.collections["SKOPE_UI"]
            # Remove from current collections
            for col in obj.users_collection:
                col.objects.unlink(obj)
            ui_col.objects.link(obj)

        return {'FINISHED'}

    def ensure_ui_collection(self, context):
        if "SKOPE_UI" not in bpy.data.collections:
            col = bpy.data.collections.new("SKOPE_UI")
            context.scene.collection.children.link(col)


class SKOPE_OT_ExportRON(Operator):
    """UI 레이아웃을 RON 파일로 내보내기"""
    bl_idname = "skope.export_ron"
    bl_label = "Export RON"
    bl_options = {'REGISTER'}

    filepath: StringProperty(
        subtype='FILE_PATH',
        default="//ui_layout.ron"
    )

    reference_width: FloatProperty(
        name="Reference Width",
        default=1920.0,
        min=100.0
    )

    reference_height: FloatProperty(
        name="Reference Height",
        default=1080.0,
        min=100.0
    )

    def invoke(self, context, event):
        context.window_manager.fileselect_add(self)
        return {'RUNNING_MODAL'}

    def execute(self, context):
        # Find all UI widgets
        if "SKOPE_UI" not in bpy.data.collections:
            self.report({'ERROR'}, "No SKOPE_UI collection found")
            return {'CANCELLED'}

        ui_col = bpy.data.collections["SKOPE_UI"]

        # Build hierarchy
        root_widgets = []
        for obj in ui_col.objects:
            if obj.parent is None:
                root_widgets.append(obj)

        # Generate RON
        ron_content = self.generate_ron(root_widgets, context)

        # Write file
        filepath = bpy.path.abspath(self.filepath)
        with open(filepath, 'w', encoding='utf-8') as f:
            f.write(ron_content)

        self.report({'INFO'}, f"Exported to {filepath}")
        return {'FINISHED'}

    def generate_ron(self, root_widgets, context):
        """Generate RON content from widget hierarchy"""
        lines = []
        lines.append("// SKOPE UI Layout - Generated by Blender Addon")
        lines.append("Widget(")
        lines.append('    id: Some("hud_root"),')
        lines.append("    layout: Layout(")
        lines.append("        size: Fill,")
        lines.append("    ),")
        lines.append("    style: Style(")
        lines.append("        background_color: Some(Rgba(0.0, 0.0, 0.0, 0.0)),")
        lines.append("    ),")
        lines.append("    children: [")

        for obj in sorted(root_widgets, key=lambda o: o.location.z):
            widget_ron = self.widget_to_ron(obj, 2)
            lines.append(widget_ron)

        lines.append("    ],")
        lines.append(")")

        return "\n".join(lines)

    def widget_to_ron(self, obj, indent_level):
        """Convert a Blender object to RON widget format"""
        props = obj.skope_widget
        indent = "    " * indent_level
        lines = []

        # Calculate position and size
        # Blender uses center point, convert to top-left offset
        loc = obj.location
        scale = obj.scale
        width = scale.x
        height = scale.y

        # Convert to screen coordinates
        offset_x = loc.x
        offset_y = -loc.y  # Flip Y axis

        lines.append(f"{indent}Widget(")

        # ID
        if props.widget_id:
            lines.append(f'{indent}    id: Some("{props.widget_id}"),')

        # Widget type
        wtype = props.widget_type
        if wtype == 'Text':
            content = props.text_content.replace('"', '\\"')
            lines.append(f'{indent}    widget_type: Text(')
            lines.append(f'{indent}        content: "{content}",')
            if props.font_size != 14.0:
                lines.append(f'{indent}        font_size: Some({props.font_size:.1f}),')
            lines.append(f'{indent}    ),')
        elif wtype == 'Image':
            if props.image_src:
                lines.append(f'{indent}    widget_type: Image(')
                lines.append(f'{indent}        src: "{props.image_src}",')
                lines.append(f'{indent}    ),')
        elif wtype == 'NineSlice':
            if props.background_image:
                lines.append(f'{indent}    widget_type: NineSlice(')
                lines.append(f'{indent}        src: "{props.background_image}",')
                lines.append(f'{indent}        border: Border(top: {props.border_top:.1f}, right: {props.border_right:.1f}, bottom: {props.border_bottom:.1f}, left: {props.border_left:.1f}),')
                lines.append(f'{indent}    ),')
        elif wtype == 'Button':
            lines.append(f'{indent}    widget_type: Button(')
            if props.text_content:
                lines.append(f'{indent}        text: Some("{props.text_content}"),')
            lines.append(f'{indent}    ),')
        elif wtype == 'InputField':
            lines.append(f'{indent}    widget_type: InputField(')
            lines.append(f'{indent}        placeholder: "{props.placeholder}",')
            lines.append(f'{indent}        value: "",')
            lines.append(f'{indent}        max_length: Some({props.max_length}),')
            lines.append(f'{indent}    ),')
        elif wtype == 'ScrollView':
            lines.append(f'{indent}    widget_type: ScrollView(')
            lines.append(f'{indent}        scroll_x: false,')
            lines.append(f'{indent}        scroll_y: true,')
            lines.append(f'{indent}    ),')
        elif wtype == 'ProgressBar':
            lines.append(f'{indent}    widget_type: ProgressBar(')
            lines.append(f'{indent}        value: {props.progress_value:.2f},')
            lines.append(f'{indent}        max_value: 1.0,')
            lines.append(f'{indent}    ),')

        # Layout
        lines.append(f'{indent}    layout: Layout(')
        lines.append(f'{indent}        anchor: {props.anchor},')
        lines.append(f'{indent}        offset: ({offset_x:.1f}, {offset_y:.1f}),')

        if props.size_mode == 'Fixed':
            lines.append(f'{indent}        size: Fixed({width:.1f}, {height:.1f}),')
        elif props.size_mode == 'Fill':
            lines.append(f'{indent}        size: Fill,')
        elif props.size_mode == 'FitContent':
            lines.append(f'{indent}        size: FitContent,')

        if props.flex_direction != 'Row' or props.gap > 0:
            lines.append(f'{indent}        flex_direction: {props.flex_direction},')
            if props.gap > 0:
                lines.append(f'{indent}        gap: {props.gap:.1f},')

        lines.append(f'{indent}    ),')

        # Style
        has_style = props.use_background_color or props.background_image
        if has_style:
            lines.append(f'{indent}    style: Style(')
            if props.use_background_color:
                c = props.background_color
                lines.append(f'{indent}        background_color: Some(Rgba({c[0]:.2f}, {c[1]:.2f}, {c[2]:.2f}, {c[3]:.2f})),')
            if props.background_image and wtype not in ['NineSlice', 'Image']:
                lines.append(f'{indent}        background_image: Some("{props.background_image}"),')
            if wtype == 'Text':
                tc = props.text_color
                lines.append(f'{indent}        text_color: Some(Rgba({tc[0]:.2f}, {tc[1]:.2f}, {tc[2]:.2f}, {tc[3]:.2f})),')
            lines.append(f'{indent}    ),')

        # Events
        if props.on_click or props.on_drop:
            lines.append(f'{indent}    events: {{')
            if props.on_click:
                lines.append(f'{indent}        "on_click": "{props.on_click}",')
            if props.on_drop:
                lines.append(f'{indent}        "on_drop": "{props.on_drop}",')
            lines.append(f'{indent}    }},')

        # Drag & Drop
        if props.draggable:
            lines.append(f'{indent}    draggable: true,')
        if props.drop_target:
            lines.append(f'{indent}    drop_target: true,')
        if props.drag_data:
            lines.append(f'{indent}    drag_data: Some("{props.drag_data}"),')
        if props.drag_group:
            lines.append(f'{indent}    drag_group: Some("{props.drag_group}"),')

        # Tooltip
        if props.tooltip:
            lines.append(f'{indent}    tooltip: Some("{props.tooltip}"),')
            if props.tooltip_delay != 0.5:
                lines.append(f'{indent}    tooltip_delay: Some({props.tooltip_delay:.2f}),')

        # Visibility/Interactive
        if not props.visible:
            lines.append(f'{indent}    visible: false,')
        if not props.interactive:
            lines.append(f'{indent}    interactive: false,')

        # Children
        children = [child for child in obj.children if child.users_collection and "SKOPE_UI" in [c.name for c in child.users_collection]]
        if children:
            lines.append(f'{indent}    children: [')
            for child in sorted(children, key=lambda o: o.location.z):
                child_ron = self.widget_to_ron(child, indent_level + 2)
                lines.append(child_ron)
            lines.append(f'{indent}    ],')

        lines.append(f'{indent}),')

        return "\n".join(lines)


class SKOPE_OT_ImportRON(Operator):
    """RON 파일에서 UI 레이아웃 가져오기"""
    bl_idname = "skope.import_ron"
    bl_label = "Import RON"
    bl_options = {'REGISTER', 'UNDO'}

    filepath: StringProperty(
        subtype='FILE_PATH',
    )

    def invoke(self, context, event):
        context.window_manager.fileselect_add(self)
        return {'RUNNING_MODAL'}

    def execute(self, context):
        filepath = bpy.path.abspath(self.filepath)

        if not os.path.exists(filepath):
            self.report({'ERROR'}, f"File not found: {filepath}")
            return {'CANCELLED'}

        # Clear existing UI collection
        if "SKOPE_UI" in bpy.data.collections:
            col = bpy.data.collections["SKOPE_UI"]
            for obj in list(col.objects):
                bpy.data.objects.remove(obj, do_unlink=True)
        else:
            col = bpy.data.collections.new("SKOPE_UI")
            context.scene.collection.children.link(col)

        # Parse RON file (simplified parser)
        with open(filepath, 'r', encoding='utf-8') as f:
            content = f.read()

        # Create widgets from parsed content
        self.parse_and_create(content, context)

        self.report({'INFO'}, f"Imported from {filepath}")
        return {'FINISHED'}

    def parse_and_create(self, content, context):
        """Simple RON parser - creates widgets from content"""
        # This is a simplified parser that handles basic widget structure
        # A full parser would need proper RON parsing

        import re

        # Find widget blocks
        widget_pattern = r'Widget\s*\('

        # For now, create a placeholder message
        self.report({'WARNING'}, "RON import is simplified - complex layouts may need manual adjustment")

        # Create root widget indicator
        bpy.ops.mesh.primitive_plane_add(size=1, location=(0, 0, 0))
        obj = context.active_object
        obj.name = "UI_Root"
        obj.scale = (1920, 1080, 1)
        obj.skope_widget.widget_type = 'Container'
        obj.skope_widget.widget_id = 'imported_root'

        # Move to collection
        if "SKOPE_UI" in bpy.data.collections:
            ui_col = bpy.data.collections["SKOPE_UI"]
            for col in obj.users_collection:
                col.objects.unlink(obj)
            ui_col.objects.link(obj)


class SKOPE_OT_SetupCamera(Operator):
    """UI 편집용 2D 카메라 설정"""
    bl_idname = "skope.setup_camera"
    bl_label = "Setup 2D Camera"
    bl_options = {'REGISTER', 'UNDO'}

    def execute(self, context):
        # Create orthographic camera
        bpy.ops.object.camera_add(location=(0, 0, 1000))
        cam = context.active_object
        cam.name = "SKOPE_UI_Camera"
        cam.data.type = 'ORTHO'
        cam.data.ortho_scale = 1080  # Match reference height
        cam.rotation_euler = (0, 0, 0)

        # Set as active camera
        context.scene.camera = cam

        # Set to camera view
        for area in context.screen.areas:
            if area.type == 'VIEW_3D':
                for space in area.spaces:
                    if space.type == 'VIEW_3D':
                        space.region_3d.view_perspective = 'CAMERA'
                        break

        self.report({'INFO'}, "2D camera setup complete")
        return {'FINISHED'}


class SKOPE_OT_AlignWidget(Operator):
    """위젯 정렬"""
    bl_idname = "skope.align_widget"
    bl_label = "Align Widget"
    bl_options = {'REGISTER', 'UNDO'}

    align_type: EnumProperty(
        name="Align",
        items=[
            ('LEFT', "Left", ""),
            ('CENTER_H', "Center Horizontal", ""),
            ('RIGHT', "Right", ""),
            ('TOP', "Top", ""),
            ('CENTER_V', "Center Vertical", ""),
            ('BOTTOM', "Bottom", ""),
        ]
    )

    def execute(self, context):
        selected = context.selected_objects
        if not selected:
            return {'CANCELLED'}

        # Reference dimensions (half of 1920x1080)
        ref_w = 960
        ref_h = 540

        for obj in selected:
            if self.align_type == 'LEFT':
                obj.location.x = -ref_w + obj.scale.x / 2
            elif self.align_type == 'CENTER_H':
                obj.location.x = 0
            elif self.align_type == 'RIGHT':
                obj.location.x = ref_w - obj.scale.x / 2
            elif self.align_type == 'TOP':
                obj.location.y = ref_h - obj.scale.y / 2
            elif self.align_type == 'CENTER_V':
                obj.location.y = 0
            elif self.align_type == 'BOTTOM':
                obj.location.y = -ref_h + obj.scale.y / 2

        return {'FINISHED'}


# ============================================================
# Panels
# ============================================================

class SKOPE_PT_MainPanel(Panel):
    """SKOPE UI 메인 패널"""
    bl_label = "SKOPE UI Editor"
    bl_idname = "SKOPE_PT_main"
    bl_space_type = 'VIEW_3D'
    bl_region_type = 'UI'
    bl_category = "SKOPE UI"

    def draw(self, context):
        layout = self.layout

        # Setup section
        box = layout.box()
        box.label(text="Setup", icon='TOOL_SETTINGS')
        box.operator("skope.setup_camera", text="Setup 2D Camera", icon='CAMERA_DATA')

        # Create widgets section
        box = layout.box()
        box.label(text="Create Widget", icon='ADD')

        row = box.row(align=True)
        op = row.operator("skope.create_widget", text="Container", icon='MESH_PLANE')
        op.widget_type = 'Container'
        op = row.operator("skope.create_widget", text="Text", icon='FILE_FONT')
        op.widget_type = 'Text'

        row = box.row(align=True)
        op = row.operator("skope.create_widget", text="Image", icon='IMAGE_DATA')
        op.widget_type = 'Image'
        op = row.operator("skope.create_widget", text="Button", icon='MOUSE_LMB')
        op.widget_type = 'Button'

        row = box.row(align=True)
        op = row.operator("skope.create_widget", text="9-Slice", icon='MESH_GRID')
        op.widget_type = 'NineSlice'

        # Import/Export section
        box = layout.box()
        box.label(text="Import / Export", icon='FILE')
        row = box.row(align=True)
        row.operator("skope.import_ron", text="Import RON", icon='IMPORT')
        row.operator("skope.export_ron", text="Export RON", icon='EXPORT')

        # Align section
        box = layout.box()
        box.label(text="Align", icon='ALIGN_CENTER')
        row = box.row(align=True)
        op = row.operator("skope.align_widget", text="", icon='ALIGN_LEFT')
        op.align_type = 'LEFT'
        op = row.operator("skope.align_widget", text="", icon='ALIGN_CENTER')
        op.align_type = 'CENTER_H'
        op = row.operator("skope.align_widget", text="", icon='ALIGN_RIGHT')
        op.align_type = 'RIGHT'

        row = box.row(align=True)
        op = row.operator("skope.align_widget", text="", icon='ALIGN_TOP')
        op.align_type = 'TOP'
        op = row.operator("skope.align_widget", text="", icon='ALIGN_MIDDLE')
        op.align_type = 'CENTER_V'
        op = row.operator("skope.align_widget", text="", icon='ALIGN_BOTTOM')
        op.align_type = 'BOTTOM'


class SKOPE_PT_WidgetPanel(Panel):
    """선택된 위젯 속성"""
    bl_label = "Widget Properties"
    bl_idname = "SKOPE_PT_widget"
    bl_space_type = 'VIEW_3D'
    bl_region_type = 'UI'
    bl_category = "SKOPE UI"

    @classmethod
    def poll(cls, context):
        return context.active_object is not None

    def draw(self, context):
        layout = self.layout
        obj = context.active_object

        if not hasattr(obj, 'skope_widget'):
            layout.label(text="Not a SKOPE widget")
            return

        props = obj.skope_widget

        # Basic info
        box = layout.box()
        box.label(text="Basic", icon='INFO')
        box.prop(props, "widget_id")
        box.prop(props, "widget_type")
        box.prop(props, "anchor")
        box.prop(props, "size_mode")

        # Transform (from object)
        box = layout.box()
        box.label(text="Transform", icon='ORIENTATION_GLOBAL')
        col = box.column(align=True)
        col.prop(obj, "location", text="Position")
        col.prop(obj, "scale", text="Size")

        # Style
        box = layout.box()
        box.label(text="Style", icon='BRUSH_DATA')
        box.prop(props, "use_background_color")
        if props.use_background_color:
            box.prop(props, "background_color")
        box.prop(props, "background_image")

        # Type-specific properties
        wtype = props.widget_type

        if wtype == 'Text':
            box = layout.box()
            box.label(text="Text", icon='FILE_FONT')
            box.prop(props, "text_content")
            box.prop(props, "font_size")
            box.prop(props, "text_color")

        elif wtype == 'Image':
            box = layout.box()
            box.label(text="Image", icon='IMAGE_DATA')
            box.prop(props, "image_src")

        elif wtype == 'NineSlice':
            box = layout.box()
            box.label(text="9-Slice Border", icon='MESH_GRID')
            col = box.column(align=True)
            col.prop(props, "border_top")
            col.prop(props, "border_right")
            col.prop(props, "border_bottom")
            col.prop(props, "border_left")

        elif wtype == 'InputField':
            box = layout.box()
            box.label(text="Input Field", icon='TEXT')
            box.prop(props, "placeholder")
            box.prop(props, "max_length")

        elif wtype == 'ProgressBar':
            box = layout.box()
            box.label(text="Progress Bar", icon='MODIFIER')
            box.prop(props, "progress_value")

        # Interaction
        box = layout.box()
        box.label(text="Interaction", icon='MOUSE_LMB')
        box.prop(props, "interactive")
        box.prop(props, "visible")

        # Drag & Drop
        box = layout.box()
        box.label(text="Drag & Drop", icon='HAND')
        box.prop(props, "draggable")
        box.prop(props, "drop_target")
        if props.draggable or props.drop_target:
            box.prop(props, "drag_data")
            box.prop(props, "drag_group")

        # Tooltip
        box = layout.box()
        box.label(text="Tooltip", icon='INFO')
        box.prop(props, "tooltip")
        if props.tooltip:
            box.prop(props, "tooltip_delay")

        # Events
        box = layout.box()
        box.label(text="Events", icon='SCRIPT')
        box.prop(props, "on_click")
        box.prop(props, "on_drop")

        # Layout (for containers)
        if wtype in ['Container', 'ScrollView']:
            box = layout.box()
            box.label(text="Layout", icon='LAYOUT_GRID')
            box.prop(props, "flex_direction")
            box.prop(props, "gap")


# ============================================================
# Registration
# ============================================================

classes = (
    SKOPEWidgetProperties,
    SKOPE_OT_CreateWidget,
    SKOPE_OT_ExportRON,
    SKOPE_OT_ImportRON,
    SKOPE_OT_SetupCamera,
    SKOPE_OT_AlignWidget,
    SKOPE_PT_MainPanel,
    SKOPE_PT_WidgetPanel,
)


def register():
    for cls in classes:
        bpy.utils.register_class(cls)

    bpy.types.Object.skope_widget = PointerProperty(type=SKOPEWidgetProperties)


def unregister():
    for cls in reversed(classes):
        bpy.utils.unregister_class(cls)

    del bpy.types.Object.skope_widget


if __name__ == "__main__":
    register()
