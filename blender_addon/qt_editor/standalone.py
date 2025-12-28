#!/usr/bin/env python3
"""
SKOPE Qt Editor - Standalone Process
Unity/Unreal-style editor with full Blender integration
"""

import sys
import os
import json
import time
from pathlib import Path

# Add PySide6 lib path
lib_path = Path.home() / ".config/blender/5.0/scripts/addons/skope_exporter/lib"
if lib_path.exists():
    sys.path.insert(0, str(lib_path))

from PySide6.QtWidgets import (
    QApplication, QMainWindow, QDockWidget, QWidget, QVBoxLayout, QHBoxLayout,
    QTreeWidget, QTreeWidgetItem, QPushButton, QLabel, QLineEdit,
    QScrollArea, QFrame, QToolBar, QListWidget, QListWidgetItem,
    QGroupBox, QComboBox, QDoubleSpinBox, QFileDialog, QMenu,
    QAbstractItemView, QStyle, QPlainTextEdit, QCheckBox
)
from datetime import datetime
from PySide6.QtCore import Qt, QTimer, Signal, QSize, QSettings
from PySide6.QtGui import QAction, QKeySequence, QShortcut, QIcon, QPixmap, QPainter, QFont


# ============================================================================
# THEME SYSTEM
# ============================================================================

DARK_THEME = """
QMainWindow, QWidget {
    background-color: #2d2d2d;
    color: #ddd;
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
QLineEdit, QSpinBox, QDoubleSpinBox, QComboBox {
    background-color: #1d1d1d;
    border: 1px solid #555;
    border-radius: 3px;
    padding: 4px;
    color: #ddd;
}
QLineEdit:focus, QDoubleSpinBox:focus {
    border: 1px solid #4a6fa5;
}
QPushButton {
    background-color: #4a6fa5;
    border: none;
    border-radius: 3px;
    padding: 6px 12px;
    font-weight: bold;
    color: white;
}
QPushButton:hover {
    background-color: #5a7fb5;
}
QPushButton:pressed {
    background-color: #3a5f95;
}
QGroupBox {
    font-weight: bold;
    border: 1px solid #555;
    border-radius: 4px;
    margin-top: 8px;
    padding-top: 16px;
    background-color: #2d2d2d;
}
QGroupBox::title {
    subcontrol-origin: margin;
    left: 8px;
    padding: 0 4px;
    color: #aaa;
}
QTreeWidget, QListWidget {
    background-color: #252525;
    border: none;
}
QTreeWidget::item, QListWidget::item {
    padding: 4px;
}
QTreeWidget::item:selected, QListWidget::item:selected {
    background-color: #4a6fa5;
}
QTreeWidget::item:hover, QListWidget::item:hover {
    background-color: #3d3d3d;
}
QScrollBar:vertical {
    background-color: #2d2d2d;
    width: 12px;
}
QScrollBar::handle:vertical {
    background-color: #555;
    border-radius: 6px;
    min-height: 20px;
}
QToolBar {
    background-color: #3d3d3d;
    border: none;
    padding: 4px;
    spacing: 4px;
}
QMenuBar {
    background-color: #3d3d3d;
    color: #ddd;
}
QMenuBar::item:selected {
    background-color: #4a6fa5;
}
QMenu {
    background-color: #2d2d2d;
    border: 1px solid #555;
}
QMenu::item:selected {
    background-color: #4a6fa5;
}
QStatusBar {
    background-color: #3d3d3d;
    color: #888;
}
"""

LIGHT_THEME = """
QMainWindow, QWidget {
    background-color: #f0f0f0;
    color: #333;
}
QDockWidget {
    background-color: #f0f0f0;
    color: #333;
}
QDockWidget::title {
    background-color: #e0e0e0;
    padding: 6px;
    font-weight: bold;
}
QLineEdit, QDoubleSpinBox, QComboBox {
    background-color: #fff;
    border: 1px solid #ccc;
    border-radius: 3px;
    padding: 4px;
    color: #333;
}
QPushButton {
    background-color: #4a6fa5;
    border: none;
    border-radius: 3px;
    padding: 6px 12px;
    font-weight: bold;
    color: white;
}
QGroupBox {
    font-weight: bold;
    border: 1px solid #ccc;
    border-radius: 4px;
    margin-top: 8px;
    padding-top: 16px;
    background-color: #f5f5f5;
}
QTreeWidget, QListWidget {
    background-color: #fff;
    border: 1px solid #ccc;
}
QTreeWidget::item:selected, QListWidget::item:selected {
    background-color: #4a6fa5;
    color: white;
}
QToolBar {
    background-color: #e0e0e0;
    border: none;
    padding: 4px;
}
QMenuBar {
    background-color: #e0e0e0;
}
QStatusBar {
    background-color: #e0e0e0;
}
"""


# ============================================================================
# ICON HELPER
# ============================================================================

def create_emoji_icon(emoji: str, size: int = 24) -> QIcon:
    """Create an icon from an emoji character"""
    pixmap = QPixmap(size, size)
    pixmap.fill(Qt.transparent)
    painter = QPainter(pixmap)
    font = QFont()
    font.setPointSize(int(size * 0.7))
    painter.setFont(font)
    painter.drawText(pixmap.rect(), Qt.AlignCenter, emoji)
    painter.end()
    return QIcon(pixmap)


# Standard icon mapping
ICONS = {
    'play': '▶',
    'stop': '■',
    'save': '💾',
    'open': '📂',
    'new': '📄',
    'export': '📤',
    'undo': '↩',
    'redo': '↪',
    'delete': '🗑',
    'duplicate': '📋',
    'move': '✥',
    'rotate': '🔄',
    'scale': '⤡',
    'cube': '🎲',
    'sphere': '🔵',
    'light': '💡',
    'camera': '📷',
    'empty': '⭕',
    'refresh': '🔃',
    'folder': '📁',
    'settings': '⚙',
    'search': '🔍',
    'add': '➕',
    'focus': '🎯',
}


# ============================================================================
# SYNC FILE PATHS
# ============================================================================

SYNC_DIR = Path.home() / ".config/skope"
SCENE_SYNC_FILE = SYNC_DIR / "scene_sync.json"
COMMANDS_FILE = SYNC_DIR / "commands.json"
SETTINGS_FILE = SYNC_DIR / "editor_settings.json"
RESULT_FILE = SYNC_DIR / "command_result.json"


def send_command(cmd: dict):
    """Send command to Blender"""
    SYNC_DIR.mkdir(parents=True, exist_ok=True)
    with open(COMMANDS_FILE, 'w') as f:
        json.dump(cmd, f)


# ============================================================================
# VECTOR3 EDIT WIDGET
# ============================================================================

class Vector3Edit(QWidget):
    """Vector3 editor widget with X/Y/Z spinboxes"""
    valueChanged = Signal(list)

    def __init__(self, label="", parent=None):
        super().__init__(parent)
        layout = QHBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(4)

        if label:
            lbl = QLabel(label)
            lbl.setFixedWidth(55)
            layout.addWidget(lbl)

        self.spinboxes = []
        colors = [('#e74c3c', 'X'), ('#2ecc71', 'Y'), ('#3498db', 'Z')]

        for color, axis in colors:
            spin = QDoubleSpinBox()
            spin.setRange(-99999, 99999)
            spin.setDecimals(3)
            spin.setSingleStep(0.1)
            spin.setPrefix(f"{axis}: ")
            spin.setStyleSheet(f"QDoubleSpinBox {{ border-left: 3px solid {color}; }}")
            spin.valueChanged.connect(self._on_value_changed)
            layout.addWidget(spin)
            self.spinboxes.append(spin)

    def setValue(self, values):
        for i, v in enumerate(values[:3]):
            self.spinboxes[i].blockSignals(True)
            self.spinboxes[i].setValue(float(v))
            self.spinboxes[i].blockSignals(False)

    def value(self):
        return [s.value() for s in self.spinboxes]

    def _on_value_changed(self):
        self.valueChanged.emit(self.value())


# ============================================================================
# HIERARCHY PANEL
# ============================================================================

class HierarchyTree(QTreeWidget):
    """Custom tree widget with drag-drop for parenting"""
    parent_changed = Signal(str, str)  # child_name, new_parent_name (or None)

    def __init__(self, parent=None):
        super().__init__(parent)
        self.setDragEnabled(True)
        self.setAcceptDrops(True)
        self.setDragDropMode(QAbstractItemView.InternalMove)
        self.setDefaultDropAction(Qt.MoveAction)

    def dropEvent(self, event):
        """Handle drop to change parent"""
        # Get the dropped item
        dropped_item = self.currentItem()
        if not dropped_item:
            event.ignore()
            return

        child_name = dropped_item.data(0, Qt.UserRole)

        # Get target item (new parent)
        target_item = self.itemAt(event.position().toPoint())
        if target_item:
            parent_name = target_item.data(0, Qt.UserRole)
            # Prevent dropping on self
            if parent_name == child_name:
                event.ignore()
                return
        else:
            parent_name = None  # Dropped on empty space = unparent

        # Emit signal to send command
        self.parent_changed.emit(child_name, parent_name)

        # Don't call super - we handle this via Blender
        event.ignore()


class HierarchyPanel(QDockWidget):
    """Scene Hierarchy Panel - Unity-style"""

    object_selected = Signal(str)

    def __init__(self, parent=None):
        super().__init__("Hierarchy", parent)
        self.setFeatures(QDockWidget.DockWidgetMovable | QDockWidget.DockWidgetFloatable)
        self.setMinimumWidth(200)

        widget = QWidget()
        layout = QVBoxLayout(widget)
        layout.setContentsMargins(4, 4, 4, 4)
        layout.setSpacing(4)

        # Search bar
        self.search = QLineEdit()
        self.search.setPlaceholderText("Search objects...")
        self.search.setClearButtonEnabled(True)
        self.search.textChanged.connect(self.filter_objects)
        layout.addWidget(self.search)

        # Tree widget with drag-drop parenting
        self.tree = HierarchyTree()
        self.tree.setHeaderHidden(True)
        self.tree.setIndentation(16)
        self.tree.itemClicked.connect(self.on_item_clicked)
        self.tree.itemDoubleClicked.connect(self.on_item_double_clicked)
        self.tree.setContextMenuPolicy(Qt.CustomContextMenu)
        self.tree.customContextMenuRequested.connect(self.show_context_menu)
        self.tree.parent_changed.connect(self.on_parent_changed)
        layout.addWidget(self.tree)

        # Object count
        self.count_label = QLabel("0 objects")
        self.count_label.setStyleSheet("color: #888; font-size: 11px;")
        layout.addWidget(self.count_label)

        self.setWidget(widget)
        self._last_data = None
        self._last_selected = None
        self._syncing_selection = False  # Prevent selection feedback loop

        # Auto-refresh timer
        self.refresh_timer = QTimer()
        self.refresh_timer.timeout.connect(self.refresh)
        self.refresh_timer.start(300)

    def refresh(self):
        """Refresh from sync file"""
        if not SCENE_SYNC_FILE.exists():
            return

        try:
            with open(SCENE_SYNC_FILE) as f:
                data = json.load(f)

            objects = data.get("objects", [])
            selected = data.get("selected", "")

            # Check if only selection changed (optimize: don't rebuild tree)
            data_without_selected = {k: v for k, v in data.items() if k != "selected"}
            data_str = json.dumps(data_without_selected, sort_keys=True)

            if data_str != self._last_data:
                # Objects changed - rebuild tree
                self._last_data = data_str
                self.tree.clear()

                icon_map = {
                    'MESH': '🎲', 'LIGHT': '💡', 'CAMERA': '📷',
                    'ARMATURE': '🦴', 'EMPTY': '⭕', 'CURVE': '〰️',
                }

                # Build name -> object mapping
                obj_map = {obj.get("name"): obj for obj in objects}

                # Create items for all objects
                item_map = {}
                for obj in objects:
                    name = obj.get("name", "Unknown")
                    obj_type = obj.get("type", "")
                    visible = obj.get("visible", True)
                    prefix = icon_map.get(obj_type, '📦')

                    item = QTreeWidgetItem()
                    display_name = f"{prefix} {name}"
                    if not visible:
                        display_name = f"👁 {display_name}"  # Hidden indicator
                    item.setText(0, display_name)
                    item.setData(0, Qt.UserRole, name)
                    item.setData(0, Qt.UserRole + 1, obj)
                    item_map[name] = item

                # Build hierarchy
                root_items = []
                for obj in objects:
                    name = obj.get("name")
                    parent_name = obj.get("parent")
                    item = item_map.get(name)

                    if parent_name and parent_name in item_map:
                        # Add as child
                        parent_item = item_map[parent_name]
                        parent_item.addChild(item)
                    else:
                        # Add as root
                        root_items.append((name, item))

                # Sort and add root items
                for name, item in sorted(root_items, key=lambda x: x[0]):
                    self.tree.addTopLevelItem(item)

                # Expand all by default
                self.tree.expandAll()

                self.count_label.setText(f"{len(objects)} objects")
                self.filter_objects(self.search.text())

            # Sync selection from Blender to Qt (if changed)
            if selected != self._last_selected:
                self._last_selected = selected
                self._syncing_selection = True  # Prevent feedback

                # Update selection in tree (including nested children)
                self.tree.blockSignals(True)
                self._update_selection_recursive(self.tree.invisibleRootItem(), selected)
                self.tree.blockSignals(False)

                self._syncing_selection = False

        except Exception:
            pass

    def _update_selection_recursive(self, parent_item, selected_name):
        """Recursively update selection state for all items"""
        for i in range(parent_item.childCount()):
            item = parent_item.child(i)
            obj_name = item.data(0, Qt.UserRole)
            should_select = (obj_name == selected_name)
            item.setSelected(should_select)
            if should_select:
                self.tree.scrollToItem(item)
                self.object_selected.emit(obj_name)
            # Recurse into children
            self._update_selection_recursive(item, selected_name)

    def filter_objects(self, text):
        text = text.lower()
        for i in range(self.tree.topLevelItemCount()):
            item = self.tree.topLevelItem(i)
            name = item.text(0).lower()
            item.setHidden(text not in name if text else False)

    def on_item_clicked(self, item, column):
        if self._syncing_selection:
            return  # Avoid feedback loop
        obj_name = item.data(0, Qt.UserRole)
        if obj_name:
            self._last_selected = obj_name  # Update to prevent re-sync
            send_command({"command": "select", "object": obj_name})
            self.object_selected.emit(obj_name)

    def on_item_double_clicked(self, item, column):
        obj_name = item.data(0, Qt.UserRole)
        if obj_name:
            send_command({"command": "focus", "object": obj_name})

    def on_parent_changed(self, child_name, parent_name):
        """Handle drag-drop parenting"""
        send_command({
            "command": "set_parent",
            "child": child_name,
            "parent": parent_name
        })

    def show_context_menu(self, pos):
        item = self.tree.itemAt(pos)
        menu = QMenu(self)

        if item:
            obj_name = item.data(0, Qt.UserRole)
            obj_data = item.data(0, Qt.UserRole + 1)

            menu.addAction("Focus (F)").triggered.connect(
                lambda: send_command({"command": "focus", "object": obj_name}))
            menu.addSeparator()

            # Visibility toggle
            is_visible = obj_data.get("visible", True) if obj_data else True
            vis_text = "Hide" if is_visible else "Show"
            menu.addAction(f"👁 {vis_text}").triggered.connect(
                lambda: send_command({"command": "set_visible", "object": obj_name, "visible": not is_visible}))

            # Parent/Unparent
            has_parent = obj_data.get("parent") if obj_data else None
            if has_parent:
                menu.addAction("⬆ Unparent").triggered.connect(
                    lambda: send_command({"command": "set_parent", "child": obj_name, "parent": None}))

            menu.addSeparator()
            menu.addAction("Duplicate").triggered.connect(
                lambda: send_command({"command": "duplicate", "object": obj_name}))
            menu.addAction("Delete").triggered.connect(
                lambda: send_command({"command": "delete", "object": obj_name}))

        menu.addSeparator()
        add_menu = menu.addMenu("Add")
        add_menu.addAction("Empty").triggered.connect(lambda: send_command({"command": "add", "type": "EMPTY"}))
        add_menu.addAction("Cube").triggered.connect(lambda: send_command({"command": "add", "type": "CUBE"}))
        add_menu.addAction("Sphere").triggered.connect(lambda: send_command({"command": "add", "type": "SPHERE"}))
        add_menu.addAction("Light").triggered.connect(lambda: send_command({"command": "add", "type": "LIGHT"}))
        add_menu.addAction("Camera").triggered.connect(lambda: send_command({"command": "add", "type": "CAMERA"}))

        menu.exec(self.tree.mapToGlobal(pos))


# ============================================================================
# INSPECTOR PANEL
# ============================================================================

class InspectorPanel(QDockWidget):
    """Inspector Panel - Unity-style property editor"""

    def __init__(self, parent=None):
        super().__init__("Inspector", parent)
        self.setFeatures(QDockWidget.DockWidgetMovable | QDockWidget.DockWidgetFloatable)
        self.setMinimumWidth(280)

        self.scroll = QScrollArea()
        self.scroll.setWidgetResizable(True)
        self.scroll.setHorizontalScrollBarPolicy(Qt.ScrollBarAlwaysOff)

        self.content = QWidget()
        self.layout = QVBoxLayout(self.content)
        self.layout.setContentsMargins(8, 8, 8, 8)
        self.layout.setSpacing(8)
        self.layout.setAlignment(Qt.AlignTop)

        self.placeholder = QLabel("Select an object to inspect")
        self.placeholder.setStyleSheet("color: #888; padding: 40px;")
        self.placeholder.setAlignment(Qt.AlignCenter)
        self.layout.addWidget(self.placeholder)

        self.scroll.setWidget(self.content)
        self.setWidget(self.scroll)

        self.current_object = None
        self._update_pending = False
        self._pending_rename = None  # Pending rename to confirm
        self._pending_style = "border: 2px solid #f39c12;"  # Yellow border for pending
        self._normal_style = ""

        self.refresh_timer = QTimer()
        self.refresh_timer.timeout.connect(self.refresh)
        self.refresh_timer.start(200)

    def refresh(self):
        if not SCENE_SYNC_FILE.exists():
            return

        try:
            with open(SCENE_SYNC_FILE) as f:
                data = json.load(f)

            selected = data.get("selected", "")
            objects = {obj["name"]: obj for obj in data.get("objects", [])}

            if selected and selected in objects:
                obj_data = objects[selected]
                if self.current_object != selected:
                    self.current_object = selected
                    self.rebuild_ui(obj_data)
                elif not self._update_pending:
                    self.update_values(obj_data)
            elif self.current_object:
                self.current_object = None
                self.show_placeholder()

        except Exception:
            pass

    def show_placeholder(self):
        self.clear_layout()
        self.placeholder = QLabel("Select an object to inspect")
        self.placeholder.setStyleSheet("color: #888; padding: 40px;")
        self.placeholder.setAlignment(Qt.AlignCenter)
        self.layout.addWidget(self.placeholder)

    def clear_layout(self):
        while self.layout.count():
            item = self.layout.takeAt(0)
            if item.widget():
                item.widget().deleteLater()

    def rebuild_ui(self, obj_data):
        self.clear_layout()

        name = obj_data.get("name", "Unknown")
        obj_type = obj_data.get("type", "")

        # Header
        header = QFrame()
        header.setStyleSheet("QFrame { background-color: #3d3d3d; border-radius: 4px; padding: 8px; }")
        header_layout = QVBoxLayout(header)
        header_layout.setSpacing(4)

        self.name_edit = QLineEdit(name)
        self.name_edit.setStyleSheet("font-weight: bold; font-size: 14px;")
        self.name_edit.editingFinished.connect(self._on_name_changed)
        header_layout.addWidget(self.name_edit)

        type_label = QLabel(f"Type: {obj_type}")
        type_label.setStyleSheet("color: #888; font-size: 11px;")
        header_layout.addWidget(type_label)

        self.layout.addWidget(header)

        # Transform
        transform_group = QGroupBox("Transform")
        transform_layout = QVBoxLayout(transform_group)
        transform_layout.setSpacing(8)

        self.pos_edit = Vector3Edit("Position")
        self.pos_edit.setValue(obj_data.get("location", [0, 0, 0]))
        self.pos_edit.valueChanged.connect(self._on_position_changed)
        transform_layout.addWidget(self.pos_edit)

        self.rot_edit = Vector3Edit("Rotation")
        rot = obj_data.get("rotation", [0, 0, 0])
        self.rot_edit.setValue([r * 57.2958 for r in rot])
        self.rot_edit.valueChanged.connect(self._on_rotation_changed)
        for spin in self.rot_edit.spinboxes:
            spin.setSuffix("°")
            spin.setRange(-360, 360)
        transform_layout.addWidget(self.rot_edit)

        self.scale_edit = Vector3Edit("Scale")
        self.scale_edit.setValue(obj_data.get("scale", [1, 1, 1]))
        self.scale_edit.valueChanged.connect(self._on_scale_changed)
        transform_layout.addWidget(self.scale_edit)

        # Reset Transform button
        reset_btn = QPushButton("↺ Reset Transform")
        reset_btn.setStyleSheet("background-color: #555; padding: 4px;")
        reset_btn.clicked.connect(self._reset_transform)
        transform_layout.addWidget(reset_btn)

        self.layout.addWidget(transform_group)

        # Add Component button
        add_btn = QPushButton("+ Add Component")
        add_btn.setStyleSheet("background-color: #555; margin-top: 8px;")
        add_btn.clicked.connect(self.show_add_component_menu)
        self.layout.addWidget(add_btn)

        self.layout.addStretch()

    def update_values(self, obj_data):
        if hasattr(self, 'pos_edit'):
            self.pos_edit.setValue(obj_data.get("location", [0, 0, 0]))
        if hasattr(self, 'rot_edit'):
            rot = obj_data.get("rotation", [0, 0, 0])
            self.rot_edit.setValue([r * 57.2958 for r in rot])
        if hasattr(self, 'scale_edit'):
            self.scale_edit.setValue(obj_data.get("scale", [1, 1, 1]))

    def show_add_component_menu(self):
        menu = QMenu(self)
        components = ['PLAYER_SPAWN', 'ENEMY_SPAWNER', 'STATIC_PROP', 'COLLIDER',
                      'ITEM_PICKUP', 'TRIGGER_ZONE', 'AUDIO_SOURCE']
        for comp in components:
            action = menu.addAction(comp)
            action.triggered.connect(lambda checked, c=comp: self._add_component(c))
        menu.exec(self.sender().mapToGlobal(self.sender().rect().bottomLeft()))

    def _add_component(self, comp_type):
        if self.current_object:
            send_command({"command": "set_component", "object": self.current_object, "component": comp_type})

    def _on_name_changed(self):
        if self.current_object and hasattr(self, 'name_edit'):
            new_name = self.name_edit.text().strip()
            if new_name and new_name != self.current_object:
                self._update_pending = True
                self._pending_rename = new_name
                # Show pending style on name field
                self.name_edit.setStyleSheet("font-weight: bold; font-size: 14px; border: 2px solid #f39c12;")
                send_command({"command": "rename", "object": self.current_object, "new_name": new_name})
                # Wait for Blender to confirm via sync update
                QTimer.singleShot(500, self._confirm_rename)

    def _on_position_changed(self, values):
        if self.current_object:
            self._update_pending = True
            self._show_pending_style(self.pos_edit)
            send_command({"command": "set_location", "object": self.current_object, "value": values})
            QTimer.singleShot(300, lambda: self._clear_pending_style(self.pos_edit))

    def _on_rotation_changed(self, values):
        if self.current_object:
            self._update_pending = True
            self._show_pending_style(self.rot_edit)
            rad = [v / 57.2958 for v in values]
            send_command({"command": "set_rotation", "object": self.current_object, "value": rad})
            QTimer.singleShot(300, lambda: self._clear_pending_style(self.rot_edit))

    def _on_scale_changed(self, values):
        if self.current_object:
            self._update_pending = True
            self._show_pending_style(self.scale_edit)
            send_command({"command": "set_scale", "object": self.current_object, "value": values})
            QTimer.singleShot(300, lambda: self._clear_pending_style(self.scale_edit))

    def _show_pending_style(self, widget):
        """Show yellow border on widget to indicate pending change"""
        for spin in widget.spinboxes:
            # Preserve color-coded left border
            current_style = spin.styleSheet()
            if "border-left" in current_style:
                base_color = current_style.split("border-left:")[1].split(";")[0].strip()
                spin.setStyleSheet(f"border: 2px solid #f39c12; border-left: 3px solid {base_color.split()[-1]};")
            else:
                spin.setStyleSheet(self._pending_style)

    def _clear_pending_style(self, widget):
        """Clear pending style and restore normal appearance"""
        self._update_pending = False
        colors = ['#e74c3c', '#2ecc71', '#3498db']  # X, Y, Z colors
        for i, spin in enumerate(widget.spinboxes):
            spin.setStyleSheet(f"QDoubleSpinBox {{ border-left: 3px solid {colors[i]}; }}")

    def _confirm_rename(self):
        """Confirm rename was successful by checking scene data"""
        if not self._pending_rename:
            return

        try:
            if SCENE_SYNC_FILE.exists():
                with open(SCENE_SYNC_FILE) as f:
                    data = json.load(f)

                objects = {obj["name"] for obj in data.get("objects", [])}

                if self._pending_rename in objects:
                    # Rename confirmed - update current_object
                    self.current_object = self._pending_rename
                    self._pending_rename = None
                    self._update_pending = False
                    # Restore normal style
                    if hasattr(self, 'name_edit'):
                        self.name_edit.setStyleSheet("font-weight: bold; font-size: 14px;")
                else:
                    # Rename not confirmed yet - retry
                    QTimer.singleShot(200, self._confirm_rename)
        except Exception:
            self._pending_rename = None
            self._update_pending = False

    def _reset_transform(self):
        """Reset position, rotation, and scale to defaults"""
        if self.current_object:
            self._update_pending = True
            send_command({"command": "set_location", "object": self.current_object, "value": [0, 0, 0]})
            send_command({"command": "set_rotation", "object": self.current_object, "value": [0, 0, 0]})
            send_command({"command": "set_scale", "object": self.current_object, "value": [1, 1, 1]})
            # Update UI immediately
            self.pos_edit.setValue([0, 0, 0])
            self.rot_edit.setValue([0, 0, 0])
            self.scale_edit.setValue([1, 1, 1])
            QTimer.singleShot(300, self._clear_pending)


# ============================================================================
# ASSETS PANEL
# ============================================================================

class AssetItem(QListWidgetItem):
    def __init__(self, name, path, asset_type):
        super().__init__(name)
        self.asset_path = path
        self.asset_type = asset_type
        self.setToolTip(str(path))


class AssetsPanel(QDockWidget):
    """Assets Panel - Project file browser"""

    def __init__(self, parent=None):
        super().__init__("Assets", parent)
        self.setFeatures(QDockWidget.DockWidgetMovable | QDockWidget.DockWidgetFloatable)

        widget = QWidget()
        layout = QVBoxLayout(widget)
        layout.setContentsMargins(4, 4, 4, 4)
        layout.setSpacing(4)

        # Toolbar
        toolbar = QHBoxLayout()

        self.path_label = QLabel("Project: Not Set")
        self.path_label.setStyleSheet("color: #888; font-size: 11px;")
        toolbar.addWidget(self.path_label, 1)

        self.filter_combo = QComboBox()
        self.filter_combo.addItems(['All', 'Models', 'Textures', 'Audio', 'Scripts'])
        self.filter_combo.currentTextChanged.connect(self.refresh)
        self.filter_combo.setFixedWidth(80)
        toolbar.addWidget(self.filter_combo)

        refresh_btn = QPushButton("↻")
        refresh_btn.setFixedWidth(30)
        refresh_btn.clicked.connect(self.refresh)
        toolbar.addWidget(refresh_btn)

        browse_btn = QPushButton("📁")
        browse_btn.setFixedWidth(30)
        browse_btn.setToolTip("Set Project Folder")
        browse_btn.clicked.connect(self.browse_folder)
        toolbar.addWidget(browse_btn)

        layout.addLayout(toolbar)

        # Search bar
        self.search = QLineEdit()
        self.search.setPlaceholderText("🔍 Search assets...")
        self.search.setClearButtonEnabled(True)
        self.search.textChanged.connect(self.filter_assets)
        layout.addWidget(self.search)

        # Breadcrumb
        self.breadcrumb = QLabel("")
        self.breadcrumb.setStyleSheet("color: #aaa; font-size: 11px; padding: 2px;")
        layout.addWidget(self.breadcrumb)

        # Asset list
        self.list = QListWidget()
        self.list.setViewMode(QListWidget.IconMode)
        self.list.setIconSize(QSize(64, 64))
        self.list.setSpacing(8)
        self.list.setResizeMode(QListWidget.Adjust)
        self.list.setDragEnabled(True)
        self.list.itemDoubleClicked.connect(self.on_item_double_clicked)
        self.list.setContextMenuPolicy(Qt.CustomContextMenu)
        self.list.customContextMenuRequested.connect(self.show_context_menu)
        layout.addWidget(self.list)

        # Status
        self.status_label = QLabel("0 items")
        self.status_label.setStyleSheet("color: #888; font-size: 11px;")
        layout.addWidget(self.status_label)

        self.setWidget(widget)

        self.project_path = None
        self.current_path = None
        self.load_settings()
        self.refresh()

    def load_settings(self):
        if SETTINGS_FILE.exists():
            try:
                with open(SETTINGS_FILE) as f:
                    settings = json.load(f)
                path = settings.get("project_path")
                if path and Path(path).exists():
                    self.project_path = Path(path)
                    self.current_path = self.project_path
            except:
                pass

    def save_settings(self):
        SYNC_DIR.mkdir(parents=True, exist_ok=True)
        with open(SETTINGS_FILE, 'w') as f:
            json.dump({"project_path": str(self.project_path) if self.project_path else None}, f)

    def browse_folder(self):
        path = QFileDialog.getExistingDirectory(self, "Select Project Folder")
        if path:
            self.project_path = Path(path)
            self.current_path = self.project_path
            self.save_settings()
            self.refresh()

    def refresh(self):
        self.list.clear()

        if not self.project_path or not self.project_path.exists():
            self.path_label.setText("Project: Not Set")
            self.breadcrumb.setText("")
            self.list.addItem(QListWidgetItem("Click 📁 to set project folder"))
            self.status_label.setText("0 items")
            return

        self.path_label.setText(f"Project: {self.project_path.name}")

        if self.current_path is None:
            self.current_path = self.project_path

        try:
            rel = self.current_path.relative_to(self.project_path)
            self.breadcrumb.setText(f"📁 {rel}" if str(rel) != "." else "📁 /")
        except:
            self.breadcrumb.setText(f"📁 {self.current_path.name}")

        # Parent directory
        if self.current_path != self.project_path:
            parent_item = QListWidgetItem("📁 ..")
            parent_item.setData(Qt.UserRole, "PARENT")
            self.list.addItem(parent_item)

        # Scan directory
        items = []
        ext_map = {
            '.glb': 'model', '.gltf': 'model', '.fbx': 'model', '.obj': 'model', '.blend': 'model',
            '.png': 'texture', '.jpg': 'texture', '.jpeg': 'texture', '.tga': 'texture',
            '.wav': 'audio', '.mp3': 'audio', '.ogg': 'audio',
            '.lua': 'script', '.py': 'script', '.json': 'script',
        }

        try:
            for entry in sorted(self.current_path.iterdir()):
                if entry.name.startswith('.'):
                    continue
                if entry.is_dir():
                    items.append(("dir", entry.name, entry))
                else:
                    ext = entry.suffix.lower()
                    if ext in ext_map:
                        items.append((ext_map[ext], entry.name, entry))
        except PermissionError:
            pass

        icon_map = {"dir": "📁", "model": "🎲", "texture": "🖼️", "audio": "🔊", "script": "📜"}
        filter_type = self.filter_combo.currentText().lower()
        type_map = {"models": "model", "textures": "texture", "audio": "audio", "scripts": "script"}

        for item_type, name, path in items:
            if filter_type != "all" and item_type != "dir" and item_type != type_map.get(filter_type):
                continue

            icon = icon_map.get(item_type, "📄")
            display_name = name[:18] + "..." if len(name) > 18 else name

            item = AssetItem(f"{icon}\n{display_name}", path, item_type)
            item.setTextAlignment(Qt.AlignCenter)
            self.list.addItem(item)

        self.status_label.setText(f"{self.list.count()} items")

    def on_item_double_clicked(self, item):
        if item.data(Qt.UserRole) == "PARENT":
            self.current_path = self.current_path.parent
            self.refresh()
        elif isinstance(item, AssetItem):
            if item.asset_type == "dir":
                self.current_path = item.asset_path
                self.refresh()
            elif item.asset_type == "model":
                send_command({"command": "import", "path": str(item.asset_path)})

    def show_context_menu(self, pos):
        item = self.list.itemAt(pos)
        menu = QMenu(self)

        if isinstance(item, AssetItem) and item.asset_type == "model":
            menu.addAction("Import to Scene").triggered.connect(
                lambda: send_command({"command": "import", "path": str(item.asset_path)}))
            menu.addSeparator()

        menu.addAction("Refresh").triggered.connect(self.refresh)
        menu.exec(self.list.mapToGlobal(pos))

    def filter_assets(self, text):
        """Filter assets by search text"""
        text = text.lower()
        visible_count = 0
        for i in range(self.list.count()):
            item = self.list.item(i)
            # Always show parent directory
            if item.data(Qt.UserRole) == "PARENT":
                item.setHidden(False)
                visible_count += 1
                continue

            # Check if text matches item name
            if isinstance(item, AssetItem):
                name = item.asset_path.name.lower() if item.asset_path else ""
            else:
                name = item.text().lower()

            hidden = text not in name if text else False
            item.setHidden(hidden)
            if not hidden:
                visible_count += 1

        self.status_label.setText(f"{visible_count} / {self.list.count()} items")


# ============================================================================
# CONSOLE PANEL
# ============================================================================

LOG_FILE = SYNC_DIR / "editor.log"


class ConsolePanel(QDockWidget):
    """Console/Log Panel - Unity-style output window"""

    def __init__(self, parent=None):
        super().__init__("Console", parent)
        self.setFeatures(QDockWidget.DockWidgetMovable | QDockWidget.DockWidgetFloatable)

        widget = QWidget()
        layout = QVBoxLayout(widget)
        layout.setContentsMargins(4, 4, 4, 4)
        layout.setSpacing(4)

        # Toolbar
        toolbar = QHBoxLayout()

        clear_btn = QPushButton("Clear")
        clear_btn.setFixedWidth(60)
        clear_btn.clicked.connect(self.clear_log)
        toolbar.addWidget(clear_btn)

        toolbar.addStretch()

        # Filter checkboxes
        self.show_info = QCheckBox("Info")
        self.show_info.setChecked(True)
        self.show_info.stateChanged.connect(self.apply_filter)
        toolbar.addWidget(self.show_info)

        self.show_warn = QCheckBox("Warning")
        self.show_warn.setChecked(True)
        self.show_warn.stateChanged.connect(self.apply_filter)
        toolbar.addWidget(self.show_warn)

        self.show_error = QCheckBox("Error")
        self.show_error.setChecked(True)
        self.show_error.stateChanged.connect(self.apply_filter)
        toolbar.addWidget(self.show_error)

        layout.addLayout(toolbar)

        # Log view
        self.log_view = QPlainTextEdit()
        self.log_view.setReadOnly(True)
        self.log_view.setLineWrapMode(QPlainTextEdit.NoWrap)
        self.log_view.setStyleSheet("""
            QPlainTextEdit {
                font-family: 'Consolas', 'Monaco', 'DejaVu Sans Mono', monospace;
                font-size: 11px;
            }
        """)
        layout.addWidget(self.log_view)

        # Status
        self.status_label = QLabel("0 messages")
        self.status_label.setStyleSheet("color: #888; font-size: 11px;")
        layout.addWidget(self.status_label)

        self.setWidget(widget)

        # Store all logs for filtering
        self.all_logs = []

        # Log file position
        self._log_pos = 0

        # Refresh timer for log file
        self.refresh_timer = QTimer()
        self.refresh_timer.timeout.connect(self.check_log_file)
        self.refresh_timer.start(500)

        # Command result checker
        self.result_timer = QTimer()
        self.result_timer.timeout.connect(self.check_command_result)
        self.result_timer.start(200)

        # Add initial message
        self.add_log("INFO", "SKOPE Editor Console initialized")

    def add_log(self, level: str, message: str):
        """Add a log entry"""
        timestamp = datetime.now().strftime("%H:%M:%S")
        entry = {"time": timestamp, "level": level, "message": message}
        self.all_logs.append(entry)
        self.apply_filter()

    def format_log_entry(self, entry: dict) -> str:
        """Format a log entry with color"""
        level = entry["level"]
        colors = {
            "INFO": "#888",
            "WARNING": "#f39c12",
            "ERROR": "#e74c3c",
            "SUCCESS": "#27ae60",
        }
        color = colors.get(level, "#888")
        return f'<span style="color:{color}">[{entry["time"]}] [{level}] {entry["message"]}</span>'

    def apply_filter(self):
        """Apply log level filter"""
        filtered = []
        for entry in self.all_logs:
            level = entry["level"]
            if level == "INFO" and not self.show_info.isChecked():
                continue
            if level == "WARNING" and not self.show_warn.isChecked():
                continue
            if level == "ERROR" and not self.show_error.isChecked():
                continue
            filtered.append(self.format_log_entry(entry))

        self.log_view.clear()
        self.log_view.appendHtml("<br>".join(filtered))
        self.status_label.setText(f"{len(filtered)} / {len(self.all_logs)} messages")

        # Auto-scroll to bottom
        scrollbar = self.log_view.verticalScrollBar()
        scrollbar.setValue(scrollbar.maximum())

    def clear_log(self):
        """Clear all logs"""
        self.all_logs = []
        self.log_view.clear()
        self.status_label.setText("0 messages")
        self.add_log("INFO", "Console cleared")

    def check_log_file(self):
        """Check for new log entries from engine"""
        if not LOG_FILE.exists():
            return

        try:
            with open(LOG_FILE) as f:
                f.seek(self._log_pos)
                new_lines = f.readlines()
                self._log_pos = f.tell()

            for line in new_lines:
                line = line.strip()
                if not line:
                    continue

                # Parse log level from line
                level = "INFO"
                if "[WARN]" in line or "[WARNING]" in line:
                    level = "WARNING"
                elif "[ERROR]" in line or "[ERR]" in line:
                    level = "ERROR"
                elif "[SUCCESS]" in line:
                    level = "SUCCESS"

                # Remove level tags from message
                message = line
                for tag in ["[INFO]", "[WARN]", "[WARNING]", "[ERROR]", "[ERR]", "[SUCCESS]"]:
                    message = message.replace(tag, "")

                self.add_log(level, message.strip())

        except Exception:
            pass

    def check_command_result(self):
        """Check for command execution results from Blender"""
        if not RESULT_FILE.exists():
            return

        try:
            with open(RESULT_FILE) as f:
                result = json.load(f)
            RESULT_FILE.unlink()

            success = result.get("success", True)
            message = result.get("message", "")

            if success:
                self.add_log("SUCCESS", message)
            else:
                self.add_log("ERROR", message)

        except Exception:
            pass

    def log_command(self, command: str, status: str = "INFO"):
        """Log a command execution"""
        self.add_log(status, f"Command: {command}")


# ============================================================================
# MAIN WINDOW
# ============================================================================

class SKOPEEditorWindow(QMainWindow):
    """Main SKOPE Editor Window"""

    def __init__(self):
        super().__init__()

        self.setWindowTitle("SKOPE Editor")
        self.setMinimumSize(900, 600)
        self.resize(1280, 800)

        # Set window icon
        icon_path = Path(__file__).parent / "icon.png"
        if icon_path.exists():
            self.setWindowIcon(QIcon(str(icon_path)))

        self.settings = QSettings("SKOPE", "Editor")
        self.is_dark_theme = self.settings.value("dark_theme", True, type=bool)
        self.apply_theme()

        self.setup_menubar()
        self.setup_toolbar()

        # Central widget
        central = QWidget()
        central_layout = QVBoxLayout(central)
        central_layout.setAlignment(Qt.AlignCenter)
        label = QLabel("3D Viewport\n(Use Blender's 3D View)")
        label.setStyleSheet("font-size: 18px; color: #666;")
        label.setAlignment(Qt.AlignCenter)
        central_layout.addWidget(label)
        self.setCentralWidget(central)

        # Status bar with multiple sections
        self.setup_statusbar()

        # Dock panels
        self.hierarchy = HierarchyPanel(self)
        self.addDockWidget(Qt.LeftDockWidgetArea, self.hierarchy)

        self.inspector = InspectorPanel(self)
        self.addDockWidget(Qt.RightDockWidgetArea, self.inspector)

        self.assets = AssetsPanel(self)
        self.addDockWidget(Qt.BottomDockWidgetArea, self.assets)

        # Console panel - tabbed with Assets
        self.console = ConsolePanel(self)
        self.addDockWidget(Qt.BottomDockWidgetArea, self.console)
        self.tabifyDockWidget(self.assets, self.console)
        self.assets.raise_()  # Assets panel on top by default

        # Restore geometry
        geometry = self.settings.value("geometry")
        if geometry:
            self.restoreGeometry(geometry)
        state = self.settings.value("windowState")
        if state:
            self.restoreState(state)

        self.hierarchy.object_selected.connect(self.on_object_selected)

        # Setup global shortcuts
        self.setup_shortcuts()

        # Status bar update timer
        self.status_timer = QTimer()
        self.status_timer.timeout.connect(self.update_statusbar)
        self.status_timer.start(500)

    def setup_statusbar(self):
        """Setup status bar with multiple sections"""
        statusbar = self.statusBar()

        # Connection status
        self.connection_label = QLabel("● Blender")
        self.connection_label.setStyleSheet("color: #27ae60; padding: 0 8px;")
        statusbar.addPermanentWidget(self.connection_label)

        # Object count
        self.object_count_label = QLabel("0 objects")
        self.object_count_label.setStyleSheet("color: #888; padding: 0 8px;")
        statusbar.addPermanentWidget(self.object_count_label)

        # Selected object
        self.selected_label = QLabel("No selection")
        self.selected_label.setStyleSheet("color: #888; padding: 0 8px;")
        statusbar.addPermanentWidget(self.selected_label)

        statusbar.showMessage("SKOPE Editor Ready")

    def update_statusbar(self):
        """Update status bar info"""
        connected = False
        engine_running = False
        obj_count = 0
        selected = ""

        if SCENE_SYNC_FILE.exists():
            try:
                with open(SCENE_SYNC_FILE) as f:
                    data = json.load(f)

                # Check timestamp for connection status
                timestamp = data.get("timestamp", 0)
                age = time.time() - timestamp
                connected = age < 1.5  # Connected if synced within 1.5 seconds

                engine_running = data.get("engine_running", False)
                obj_count = len(data.get("objects", []))
                selected = data.get("selected", "")
            except Exception:
                pass

        # Update connection/engine status
        if engine_running:
            self.connection_label.setText("▶ Running")
            self.connection_label.setStyleSheet("color: #27ae60; padding: 0 8px; font-weight: bold;")
            # Sync play button state
            if not self.play_btn.isChecked():
                self.play_btn.blockSignals(True)
                self.play_btn.setChecked(True)
                self._update_play_button_style(True)
                self.play_btn.blockSignals(False)
        elif connected:
            self.connection_label.setText("● Connected")
            self.connection_label.setStyleSheet("color: #3498db; padding: 0 8px;")
            # Sync play button state
            if self.play_btn.isChecked():
                self.play_btn.blockSignals(True)
                self.play_btn.setChecked(False)
                self._update_play_button_style(False)
                self.play_btn.blockSignals(False)
        else:
            self.connection_label.setText("○ Disconnected")
            self.connection_label.setStyleSheet("color: #e74c3c; padding: 0 8px;")

        # Update object count
        self.object_count_label.setText(f"{obj_count} objects")

        # Update selected
        if selected:
            self.selected_label.setText(f"Selected: {selected}")
            self.selected_label.setStyleSheet("color: #4a6fa5; padding: 0 8px;")
        else:
            self.selected_label.setText("No selection")
            self.selected_label.setStyleSheet("color: #888; padding: 0 8px;")

    def setup_menubar(self):
        menubar = self.menuBar()

        # File menu
        file_menu = menubar.addMenu("&File")

        new_action = file_menu.addAction(create_emoji_icon(ICONS['new']), "&New Scene")
        new_action.setShortcut(QKeySequence.New)
        new_action.triggered.connect(lambda: send_command({"command": "new_scene"}))

        open_action = file_menu.addAction(create_emoji_icon(ICONS['open']), "&Open...")
        open_action.setShortcut(QKeySequence.Open)
        open_action.triggered.connect(lambda: send_command({"command": "open"}))

        save_action = file_menu.addAction(create_emoji_icon(ICONS['save']), "&Save")
        save_action.setShortcut(QKeySequence.Save)
        save_action.triggered.connect(lambda: send_command({"command": "save"}))

        file_menu.addSeparator()

        export_action = file_menu.addAction(create_emoji_icon(ICONS['export']), "&Export All")
        export_action.setShortcut("Ctrl+E")
        export_action.triggered.connect(self.on_export)

        file_menu.addSeparator()

        exit_action = file_menu.addAction("E&xit")
        exit_action.setShortcut(QKeySequence.Quit)
        exit_action.triggered.connect(self.close)

        # Edit menu
        edit_menu = menubar.addMenu("&Edit")

        undo_action = edit_menu.addAction(create_emoji_icon(ICONS['undo']), "&Undo")
        undo_action.setShortcut(QKeySequence.Undo)
        undo_action.triggered.connect(lambda: send_command({"command": "undo"}))

        redo_action = edit_menu.addAction(create_emoji_icon(ICONS['redo']), "&Redo")
        redo_action.setShortcut(QKeySequence.Redo)
        redo_action.triggered.connect(lambda: send_command({"command": "redo"}))

        edit_menu.addSeparator()

        delete_action = edit_menu.addAction(create_emoji_icon(ICONS['delete']), "&Delete")
        delete_action.setShortcut(QKeySequence.Delete)
        delete_action.triggered.connect(self.on_delete)

        duplicate_action = edit_menu.addAction(create_emoji_icon(ICONS['duplicate']), "D&uplicate")
        duplicate_action.setShortcut("Ctrl+D")
        duplicate_action.triggered.connect(self.on_duplicate)

        # View menu
        view_menu = menubar.addMenu("&View")

        theme_action = view_menu.addAction("Toggle &Dark/Light Theme")
        theme_action.setShortcut("Ctrl+T")
        theme_action.triggered.connect(self.toggle_theme)

        view_menu.addSeparator()

        focus_action = view_menu.addAction(create_emoji_icon(ICONS['focus']), "&Focus Selected")
        focus_action.setShortcut("F")
        focus_action.triggered.connect(self.on_focus)

        view_menu.addSeparator()
        view_menu.addAction("Reset Layout", self.reset_layout)

        # GameObject menu
        go_menu = menubar.addMenu("&GameObject")

        empty_action = go_menu.addAction(create_emoji_icon(ICONS['empty']), "Create &Empty")
        empty_action.setShortcut("Ctrl+Shift+E")
        empty_action.triggered.connect(lambda: send_command({"command": "add", "type": "EMPTY"}))

        go_menu.addSeparator()

        primitives = go_menu.addMenu(create_emoji_icon(ICONS['cube']), "3D &Object")
        primitives.addAction(create_emoji_icon(ICONS['cube']), "Cube").triggered.connect(
            lambda: send_command({"command": "add", "type": "CUBE"}))
        primitives.addAction(create_emoji_icon(ICONS['sphere']), "Sphere").triggered.connect(
            lambda: send_command({"command": "add", "type": "SPHERE"}))
        primitives.addAction("Plane").triggered.connect(
            lambda: send_command({"command": "add", "type": "PLANE"}))

        lights = go_menu.addMenu(create_emoji_icon(ICONS['light']), "&Light")
        lights.addAction("Point Light").triggered.connect(
            lambda: send_command({"command": "add", "type": "POINT_LIGHT"}))
        lights.addAction("Sun Light").triggered.connect(
            lambda: send_command({"command": "add", "type": "SUN_LIGHT"}))

        cam_action = go_menu.addAction(create_emoji_icon(ICONS['camera']), "&Camera")
        cam_action.triggered.connect(lambda: send_command({"command": "add", "type": "CAMERA"}))

        # Help menu
        help_menu = menubar.addMenu("&Help")
        help_menu.addAction("&About SKOPE")

    def setup_toolbar(self):
        toolbar = QToolBar("Main")
        toolbar.setMovable(False)
        toolbar.setIconSize(QSize(28, 28))
        self.addToolBar(toolbar)

        # Play/Stop toggle button
        self.play_btn = QPushButton("▶ Play")
        self.play_btn.setCheckable(True)
        self._update_play_button_style(False)
        self.play_btn.setShortcut("F5")
        self.play_btn.setToolTip("Play/Stop Game (F5)")
        self.play_btn.toggled.connect(self.on_play_toggled)
        toolbar.addWidget(self.play_btn)

        toolbar.addSeparator()

        # Transform tools
        move_btn = QPushButton("✥ Move")
        move_btn.setShortcut("G")
        move_btn.setToolTip("Move Tool (G)")
        move_btn.clicked.connect(lambda: send_command({"command": "tool", "type": "MOVE"}))
        toolbar.addWidget(move_btn)

        rotate_btn = QPushButton("🔄 Rotate")
        rotate_btn.setShortcut("R")
        rotate_btn.setToolTip("Rotate Tool (R)")
        rotate_btn.clicked.connect(lambda: send_command({"command": "tool", "type": "ROTATE"}))
        toolbar.addWidget(rotate_btn)

        scale_btn = QPushButton("⤡ Scale")
        scale_btn.setShortcut("S")
        scale_btn.setToolTip("Scale Tool (S)")
        scale_btn.clicked.connect(lambda: send_command({"command": "tool", "type": "SCALE"}))
        toolbar.addWidget(scale_btn)

        toolbar.addSeparator()

        # Export button
        export_btn = QPushButton("📤 Export")
        export_btn.setToolTip("Export All (Ctrl+E)")
        export_btn.clicked.connect(self.on_export)
        toolbar.addWidget(export_btn)

    def setup_shortcuts(self):
        """Setup global keyboard shortcuts"""
        # Delete selected object
        QShortcut(QKeySequence.Delete, self, self.on_delete)

        # Duplicate
        QShortcut(QKeySequence("Ctrl+D"), self, self.on_duplicate)

        # Focus on selected
        QShortcut(QKeySequence("F"), self, self.on_focus)

        # Save
        QShortcut(QKeySequence.Save, self, lambda: send_command({"command": "save"}))

        # Undo/Redo
        QShortcut(QKeySequence.Undo, self, lambda: send_command({"command": "undo"}))
        QShortcut(QKeySequence.Redo, self, lambda: send_command({"command": "redo"}))

    def apply_theme(self):
        self.setStyleSheet(DARK_THEME if self.is_dark_theme else LIGHT_THEME)

    def toggle_theme(self):
        self.is_dark_theme = not self.is_dark_theme
        self.settings.setValue("dark_theme", self.is_dark_theme)
        self.apply_theme()

    def reset_layout(self):
        self.addDockWidget(Qt.LeftDockWidgetArea, self.hierarchy)
        self.addDockWidget(Qt.RightDockWidgetArea, self.inspector)
        self.addDockWidget(Qt.BottomDockWidgetArea, self.assets)
        self.addDockWidget(Qt.BottomDockWidgetArea, self.console)
        self.tabifyDockWidget(self.assets, self.console)
        self.assets.raise_()

    def on_object_selected(self, obj_name):
        self.statusBar().showMessage(f"Selected: {obj_name}")

    def on_play_toggled(self, checked):
        """Handle Play/Stop toggle button"""
        if checked:
            self.on_play()
        else:
            self.on_stop()

    def on_play(self):
        """Start the game engine"""
        send_command({"command": "play"})
        self._update_play_button_style(True)
        self.statusBar().showMessage("Starting engine...")
        self.console.add_log("INFO", "Starting SKOPE Engine...")

    def on_stop(self):
        """Stop the game engine"""
        send_command({"command": "stop"})
        self._update_play_button_style(False)
        self.statusBar().showMessage("Engine stopped")
        self.console.add_log("INFO", "Engine stopped")

    def _update_play_button_style(self, is_running: bool):
        """Update play button appearance based on engine state"""
        if is_running:
            self.play_btn.setText("■ Stop")
            self.play_btn.setStyleSheet("""
                QPushButton { background-color: #e74c3c; padding: 8px 16px; font-weight: bold; color: white; }
                QPushButton:hover { background-color: #c0392b; }
            """)
        else:
            self.play_btn.setText("▶ Play")
            self.play_btn.setStyleSheet("""
                QPushButton { background-color: #4CAF50; padding: 8px 16px; font-weight: bold; color: white; }
                QPushButton:hover { background-color: #5CBF60; }
            """)

    def on_export(self):
        send_command({"command": "export"})
        self.statusBar().showMessage("Exporting...")

    def on_delete(self):
        """Delete the currently selected object"""
        if self.inspector.current_object:
            send_command({"command": "delete", "object": self.inspector.current_object})
            self.statusBar().showMessage(f"Deleted: {self.inspector.current_object}")

    def on_duplicate(self):
        """Duplicate the currently selected object"""
        if self.inspector.current_object:
            send_command({"command": "duplicate", "object": self.inspector.current_object})
            self.statusBar().showMessage(f"Duplicated: {self.inspector.current_object}")

    def on_focus(self):
        """Focus on the currently selected object in Blender's viewport"""
        if self.inspector.current_object:
            send_command({"command": "focus", "object": self.inspector.current_object})
            self.statusBar().showMessage(f"Focused: {self.inspector.current_object}")

    def closeEvent(self, event):
        self.settings.setValue("geometry", self.saveGeometry())
        self.settings.setValue("windowState", self.saveState())
        event.accept()


# ============================================================================
# MAIN
# ============================================================================

def main():
    app = QApplication(sys.argv)
    app.setApplicationName("SKOPE Editor")
    app.setOrganizationName("SKOPE Games")

    # Set application icon
    icon_path = Path(__file__).parent / "icon.png"
    if icon_path.exists():
        app.setWindowIcon(QIcon(str(icon_path)))

    window = SKOPEEditorWindow()
    window.show()

    sys.exit(app.exec())


if __name__ == "__main__":
    main()
