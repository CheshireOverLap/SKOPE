"""
SKOPE Editor Main Window
Unity/Unreal-style dockable editor interface
"""

import bpy
from PySide6.QtWidgets import (
    QMainWindow, QDockWidget, QWidget, QVBoxLayout, QHBoxLayout,
    QTreeWidget, QTreeWidgetItem, QPushButton, QLabel, QLineEdit,
    QScrollArea, QFrame, QSplitter, QToolBar, QStatusBar,
    QApplication, QComboBox, QSpinBox, QDoubleSpinBox, QCheckBox,
    QListWidget, QListWidgetItem, QGridLayout, QGroupBox
)
from PySide6.QtCore import Qt, QTimer, Signal, QSize
from PySide6.QtGui import QIcon, QAction, QFont

# Global editor instance
_editor_instance = None


class HierarchyPanel(QDockWidget):
    """Scene Hierarchy Panel - like Unity's Hierarchy"""

    object_selected = Signal(str)  # Emits object name when selected

    def __init__(self, parent=None):
        super().__init__("Hierarchy", parent)
        self.setFeatures(QDockWidget.DockWidgetMovable | QDockWidget.DockWidgetFloatable)

        widget = QWidget()
        layout = QVBoxLayout(widget)
        layout.setContentsMargins(4, 4, 4, 4)
        layout.setSpacing(4)

        # Search bar
        self.search = QLineEdit()
        self.search.setPlaceholderText("Search...")
        self.search.textChanged.connect(self.filter_objects)
        layout.addWidget(self.search)

        # Tree widget
        self.tree = QTreeWidget()
        self.tree.setHeaderHidden(True)
        self.tree.setIndentation(16)
        self.tree.itemClicked.connect(self.on_item_clicked)
        self.tree.setStyleSheet("""
            QTreeWidget {
                background-color: #2d2d2d;
                border: none;
            }
            QTreeWidget::item {
                padding: 4px;
            }
            QTreeWidget::item:selected {
                background-color: #4a6fa5;
            }
            QTreeWidget::item:hover {
                background-color: #3d3d3d;
            }
        """)
        layout.addWidget(self.tree)

        # Object count
        self.count_label = QLabel("0 objects")
        self.count_label.setStyleSheet("color: #888;")
        layout.addWidget(self.count_label)

        self.setWidget(widget)

        # Auto-refresh timer
        self.refresh_timer = QTimer()
        self.refresh_timer.timeout.connect(self.refresh)
        self.refresh_timer.start(1000)  # Refresh every second

    def refresh(self):
        """Refresh hierarchy from Blender scene"""
        try:
            self.tree.clear()

            # Get scene objects
            scene = bpy.context.scene
            root_objects = [obj for obj in scene.objects if obj.parent is None]

            for obj in sorted(root_objects, key=lambda x: x.name):
                self._add_object_item(obj, self.tree)

            self.count_label.setText(f"{len(scene.objects)} objects")
            self.filter_objects(self.search.text())

        except Exception as e:
            print(f"[SKOPE Qt] Hierarchy refresh error: {e}")

    def _add_object_item(self, obj, parent):
        """Add object to tree with children"""
        item = QTreeWidgetItem()
        item.setText(0, obj.name)
        item.setData(0, Qt.UserRole, obj.name)

        # Icon based on type
        icon_map = {
            'MESH': '🎲',
            'LIGHT': '💡',
            'CAMERA': '📷',
            'ARMATURE': '🦴',
            'EMPTY': '⭕',
            'CURVE': '〰️',
        }
        prefix = icon_map.get(obj.type, '📦')
        item.setText(0, f"{prefix} {obj.name}")

        # Check for SKOPE component
        if hasattr(obj, 'skope_component') and obj.skope_component.component_type != 'NONE':
            item.setText(0, f"{prefix} {obj.name} ●")

        if isinstance(parent, QTreeWidget):
            parent.addTopLevelItem(item)
        else:
            parent.addChild(item)

        # Add children
        for child in obj.children:
            self._add_object_item(child, item)

        item.setExpanded(True)

    def filter_objects(self, text):
        """Filter tree items by search text"""
        text = text.lower()
        for i in range(self.tree.topLevelItemCount()):
            item = self.tree.topLevelItem(i)
            self._filter_item(item, text)

    def _filter_item(self, item, text):
        """Recursively filter items"""
        name = item.text(0).lower()
        matches = text in name if text else True

        # Check children
        child_visible = False
        for i in range(item.childCount()):
            child = item.child(i)
            if self._filter_item(child, text):
                child_visible = True

        visible = matches or child_visible
        item.setHidden(not visible)
        return visible

    def on_item_clicked(self, item, column):
        """Handle item click - select in Blender"""
        obj_name = item.data(0, Qt.UserRole)
        if obj_name:
            try:
                obj = bpy.context.scene.objects.get(obj_name)
                if obj:
                    bpy.ops.object.select_all(action='DESELECT')
                    obj.select_set(True)
                    bpy.context.view_layer.objects.active = obj
                    self.object_selected.emit(obj_name)
            except Exception as e:
                print(f"[SKOPE Qt] Selection error: {e}")


class InspectorPanel(QDockWidget):
    """Inspector Panel - like Unity's Inspector"""

    def __init__(self, parent=None):
        super().__init__("Inspector", parent)
        self.setFeatures(QDockWidget.DockWidgetMovable | QDockWidget.DockWidgetFloatable)

        self.scroll = QScrollArea()
        self.scroll.setWidgetResizable(True)
        self.scroll.setStyleSheet("QScrollArea { border: none; }")

        self.content = QWidget()
        self.layout = QVBoxLayout(self.content)
        self.layout.setContentsMargins(8, 8, 8, 8)
        self.layout.setSpacing(8)
        self.layout.setAlignment(Qt.AlignTop)

        self.scroll.setWidget(self.content)
        self.setWidget(self.scroll)

        self.current_object = None

        # Auto-refresh timer
        self.refresh_timer = QTimer()
        self.refresh_timer.timeout.connect(self.refresh)
        self.refresh_timer.start(500)

    def refresh(self):
        """Refresh inspector for selected object"""
        try:
            obj = bpy.context.active_object

            if obj != self.current_object:
                self.current_object = obj
                self.rebuild_ui()
        except:
            pass

    def rebuild_ui(self):
        """Rebuild inspector UI for current object"""
        # Clear existing widgets
        while self.layout.count():
            item = self.layout.takeAt(0)
            if item.widget():
                item.widget().deleteLater()

        if not self.current_object:
            label = QLabel("No object selected")
            label.setStyleSheet("color: #888; padding: 20px;")
            label.setAlignment(Qt.AlignCenter)
            self.layout.addWidget(label)
            return

        obj = self.current_object

        # Object Header
        header = QFrame()
        header.setStyleSheet("""
            QFrame {
                background-color: #3d3d3d;
                border-radius: 4px;
                padding: 8px;
            }
        """)
        header_layout = QHBoxLayout(header)

        name_edit = QLineEdit(obj.name)
        name_edit.setStyleSheet("font-weight: bold; font-size: 14px;")
        header_layout.addWidget(name_edit)

        self.layout.addWidget(header)

        # Transform Section
        self.add_transform_section(obj)

        # Component Section
        self.add_component_section(obj)

        # Spacer
        self.layout.addStretch()

    def add_transform_section(self, obj):
        """Add transform properties"""
        group = QGroupBox("Transform")
        group.setStyleSheet("""
            QGroupBox {
                font-weight: bold;
                border: 1px solid #555;
                border-radius: 4px;
                margin-top: 8px;
                padding-top: 16px;
            }
            QGroupBox::title {
                subcontrol-origin: margin;
                left: 8px;
                padding: 0 4px;
            }
        """)
        layout = QGridLayout(group)

        # Position
        layout.addWidget(QLabel("Position"), 0, 0)
        for i, axis in enumerate(['X', 'Y', 'Z']):
            spin = QDoubleSpinBox()
            spin.setRange(-9999, 9999)
            spin.setDecimals(3)
            spin.setValue(obj.location[i])
            spin.setPrefix(f"{axis}: ")
            layout.addWidget(spin, 0, i + 1)

        # Rotation
        layout.addWidget(QLabel("Rotation"), 1, 0)
        for i, axis in enumerate(['X', 'Y', 'Z']):
            spin = QDoubleSpinBox()
            spin.setRange(-360, 360)
            spin.setDecimals(1)
            spin.setValue(obj.rotation_euler[i] * 57.2958)  # Rad to deg
            spin.setPrefix(f"{axis}: ")
            spin.setSuffix("°")
            layout.addWidget(spin, 1, i + 1)

        # Scale
        layout.addWidget(QLabel("Scale"), 2, 0)
        for i, axis in enumerate(['X', 'Y', 'Z']):
            spin = QDoubleSpinBox()
            spin.setRange(0.001, 9999)
            spin.setDecimals(3)
            spin.setValue(obj.scale[i])
            spin.setPrefix(f"{axis}: ")
            layout.addWidget(spin, 2, i + 1)

        self.layout.addWidget(group)

    def add_component_section(self, obj):
        """Add SKOPE component properties"""
        if not hasattr(obj, 'skope_component'):
            return

        props = obj.skope_component
        comp_type = props.component_type

        group = QGroupBox("SKOPE Component")
        group.setStyleSheet("""
            QGroupBox {
                font-weight: bold;
                border: 1px solid #4a6fa5;
                border-radius: 4px;
                margin-top: 8px;
                padding-top: 16px;
            }
        """)
        layout = QVBoxLayout(group)

        # Component type
        type_combo = QComboBox()
        types = ['NONE', 'PLAYER_SPAWN', 'ENEMY_SPAWNER', 'STATIC_PROP',
                 'COLLIDER', 'ITEM_PICKUP', 'TRIGGER_ZONE', 'LIGHT',
                 'CAMERA', 'CHARACTER', 'AUDIO_SOURCE', 'PARTICLE_EMITTER']
        type_combo.addItems(types)
        type_combo.setCurrentText(comp_type)
        layout.addWidget(type_combo)

        if comp_type != 'NONE':
            # Add component-specific properties
            info = QLabel(f"Component: {comp_type}")
            info.setStyleSheet("color: #4a6fa5;")
            layout.addWidget(info)

        self.layout.addWidget(group)


class AssetsPanel(QDockWidget):
    """Assets Panel - Project asset browser"""

    def __init__(self, parent=None):
        super().__init__("Assets", parent)
        self.setFeatures(QDockWidget.DockWidgetMovable | QDockWidget.DockWidgetFloatable)

        widget = QWidget()
        layout = QVBoxLayout(widget)
        layout.setContentsMargins(4, 4, 4, 4)

        # Toolbar
        toolbar = QHBoxLayout()

        self.filter_combo = QComboBox()
        self.filter_combo.addItems(['All', 'Models', 'Textures', 'Audio'])
        toolbar.addWidget(self.filter_combo)

        refresh_btn = QPushButton("↻")
        refresh_btn.setMaximumWidth(30)
        refresh_btn.clicked.connect(self.refresh)
        toolbar.addWidget(refresh_btn)

        toolbar.addStretch()
        layout.addLayout(toolbar)

        # Asset grid
        self.list = QListWidget()
        self.list.setViewMode(QListWidget.IconMode)
        self.list.setIconSize(QSize(64, 64))
        self.list.setSpacing(8)
        self.list.setStyleSheet("""
            QListWidget {
                background-color: #2d2d2d;
                border: none;
            }
            QListWidget::item {
                background-color: #3d3d3d;
                border-radius: 4px;
                padding: 4px;
            }
            QListWidget::item:selected {
                background-color: #4a6fa5;
            }
        """)
        layout.addWidget(self.list)

        self.setWidget(widget)
        self.refresh()

    def refresh(self):
        """Refresh asset list"""
        self.list.clear()

        import os

        # Try to find assets folder
        project_path = os.environ.get('SKOPE_PROJECT_PATH', '')
        if not project_path and bpy.data.filepath:
            project_path = os.path.dirname(bpy.path.abspath(bpy.data.filepath))

        if not project_path:
            item = QListWidgetItem("No project path set")
            self.list.addItem(item)
            return

        assets_path = os.path.join(project_path, 'assets', 'models')

        if not os.path.exists(assets_path):
            item = QListWidgetItem(f"Assets folder not found")
            self.list.addItem(item)
            return

        # Scan for assets
        extensions = ('.glb', '.gltf', '.fbx', '.obj', '.png', '.jpg')
        for root, dirs, files in os.walk(assets_path):
            for file in files:
                if file.lower().endswith(extensions):
                    name = os.path.splitext(file)[0]
                    item = QListWidgetItem(name[:15])
                    self.list.addItem(item)


class SKOPEEditorWindow(QMainWindow):
    """Main SKOPE Editor Window"""

    def __init__(self, parent=None):
        super().__init__(parent)

        self.setWindowTitle("SKOPE Editor")
        self.setMinimumSize(800, 600)
        self.resize(1200, 800)

        # Dark theme
        self.setStyleSheet("""
            QMainWindow {
                background-color: #2d2d2d;
            }
            QDockWidget {
                background-color: #2d2d2d;
                color: #ddd;
            }
            QDockWidget::title {
                background-color: #3d3d3d;
                padding: 6px;
                font-weight: bold;
            }
            QWidget {
                color: #ddd;
                font-size: 12px;
            }
            QLineEdit, QSpinBox, QDoubleSpinBox, QComboBox {
                background-color: #1d1d1d;
                border: 1px solid #555;
                border-radius: 3px;
                padding: 4px;
            }
            QPushButton {
                background-color: #4a6fa5;
                border: none;
                border-radius: 3px;
                padding: 6px 12px;
                font-weight: bold;
            }
            QPushButton:hover {
                background-color: #5a7fb5;
            }
            QPushButton:pressed {
                background-color: #3a5f95;
            }
            QGroupBox {
                background-color: #2d2d2d;
            }
        """)

        # Central widget (placeholder for 3D view info)
        central = QWidget()
        central_layout = QVBoxLayout(central)
        central_layout.setAlignment(Qt.AlignCenter)

        label = QLabel("3D Viewport\n(Use Blender's 3D View)")
        label.setStyleSheet("font-size: 18px; color: #666;")
        label.setAlignment(Qt.AlignCenter)
        central_layout.addWidget(label)

        self.setCentralWidget(central)

        # Toolbar
        self.setup_toolbar()

        # Status bar
        self.statusBar().showMessage("SKOPE Editor Ready")

        # Dock panels
        self.hierarchy = HierarchyPanel(self)
        self.addDockWidget(Qt.LeftDockWidgetArea, self.hierarchy)

        self.inspector = InspectorPanel(self)
        self.addDockWidget(Qt.RightDockWidgetArea, self.inspector)

        self.assets = AssetsPanel(self)
        self.addDockWidget(Qt.BottomDockWidgetArea, self.assets)

        # Connect signals
        self.hierarchy.object_selected.connect(self.on_object_selected)

    def setup_toolbar(self):
        """Setup main toolbar"""
        toolbar = QToolBar("Main Toolbar")
        toolbar.setMovable(False)
        toolbar.setStyleSheet("""
            QToolBar {
                background-color: #3d3d3d;
                border: none;
                padding: 4px;
                spacing: 4px;
            }
        """)
        self.addToolBar(toolbar)

        # Play button
        play_btn = QPushButton("▶ Play")
        play_btn.setStyleSheet("""
            QPushButton {
                background-color: #4CAF50;
                padding: 8px 16px;
            }
            QPushButton:hover {
                background-color: #5CBF60;
            }
        """)
        play_btn.clicked.connect(self.on_play)
        toolbar.addWidget(play_btn)

        # Stop button
        stop_btn = QPushButton("■ Stop")
        stop_btn.clicked.connect(self.on_stop)
        toolbar.addWidget(stop_btn)

        toolbar.addSeparator()

        # Export button
        export_btn = QPushButton("Export All")
        export_btn.clicked.connect(self.on_export)
        toolbar.addWidget(export_btn)

    def on_object_selected(self, obj_name):
        """Handle object selection from hierarchy"""
        self.statusBar().showMessage(f"Selected: {obj_name}")
        self.inspector.refresh()

    def on_play(self):
        """Launch SKOPE engine"""
        try:
            bpy.ops.skope.play()
            self.statusBar().showMessage("SKOPE Engine launched!")
        except Exception as e:
            self.statusBar().showMessage(f"Play failed: {e}")

    def on_stop(self):
        """Stop SKOPE engine"""
        try:
            bpy.ops.skope.play(action='STOP')
            self.statusBar().showMessage("SKOPE Engine stopped")
        except Exception as e:
            self.statusBar().showMessage(f"Stop failed: {e}")

    def on_export(self):
        """Export all assets"""
        try:
            bpy.ops.skope.export_all()
            self.statusBar().showMessage("Export complete!")
        except Exception as e:
            self.statusBar().showMessage(f"Export failed: {e}")

    def closeEvent(self, event):
        """Handle window close"""
        global _editor_instance
        _editor_instance = None
        event.accept()


def launch_editor():
    """Launch or focus the SKOPE Editor window"""
    global _editor_instance

    try:
        # Get Qt application
        app = QApplication.instance()
        if app is None:
            print("[SKOPE Qt] No QApplication instance - BQT not initialized?")
            return None

        # Create or show editor
        if _editor_instance is None or not _editor_instance.isVisible():
            parent = getattr(app, 'blender_widget', None)
            _editor_instance = SKOPEEditorWindow(parent)
            _editor_instance.show()
        else:
            _editor_instance.raise_()
            _editor_instance.activateWindow()

        return _editor_instance

    except Exception as e:
        print(f"[SKOPE Qt] Failed to launch editor: {e}")
        import traceback
        traceback.print_exc()
        return None
