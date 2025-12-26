"""
SKOPE Component Properties
Defines game component properties for Blender objects
"""

import bpy
from bpy.props import (
    EnumProperty,
    StringProperty,
    IntProperty,
    FloatProperty,
    BoolProperty,
)
from bpy.types import PropertyGroup

# ============ Component Property Group ============

class SKOPE_PG_ComponentProperties(PropertyGroup):
    """Properties for SKOPE game components"""

    # Component type selector
    component_type: EnumProperty(
        name="Component Type",
        description="Type of game component to attach to this object",
        items=[
            ('NONE', 'None', 'No component'),
            ('PLAYER_SPAWN', 'Player Spawn', 'Player starting position'),
            ('ENEMY_SPAWNER', 'Enemy Spawner', 'Spawn enemies at this location'),
            ('STATIC_PROP', 'Static Prop', 'Static background object'),
            ('COLLIDER', 'Collider', 'Collision volume'),
            ('ITEM_PICKUP', 'Item Pickup', 'Pickup item location'),
            ('TRIGGER_ZONE', 'Trigger Zone', 'Event trigger area'),
            ('LIGHT', 'Light', 'Game light source'),
        ],
        default='NONE',
    )

    # ===== EnemySpawner Properties =====
    enemy_type: StringProperty(
        name="Enemy Type",
        description="Type of enemy to spawn (e.g., 'goblin_basic')",
        default="goblin_basic",
    )

    enemy_count: IntProperty(
        name="Count",
        description="Number of enemies to spawn",
        default=5,
        min=1,
        max=100,
    )

    enemy_respawn: BoolProperty(
        name="Respawn",
        description="Whether enemies respawn after death",
        default=False,
    )

    # ===== StaticProp Properties =====
    has_collision: BoolProperty(
        name="Has Collision",
        description="Whether this prop has collision",
        default=True,
    )

    mesh_name: StringProperty(
        name="Mesh Name",
        description="Name of the mesh to use in engine (e.g., 'Cube', 'Sphere'). Leave empty to use object's mesh data name",
        default="",
    )

    # ===== Collider Properties =====
    collider_shape: EnumProperty(
        name="Shape",
        description="Collider shape",
        items=[
            ('BOX', 'Box', 'Box collider'),
            ('SPHERE', 'Sphere', 'Sphere collider'),
            ('MESH', 'Mesh', 'Mesh collider (convex hull)'),
        ],
        default='BOX',
    )

    is_trigger: BoolProperty(
        name="Is Trigger",
        description="Whether this is a trigger (no physical collision)",
        default=False,
    )

    # ===== ItemPickup Properties =====
    item_id: StringProperty(
        name="Item ID",
        description="Unique identifier for this item",
        default="item_health_potion",
    )

    item_type: EnumProperty(
        name="Item Type",
        description="Type of item",
        items=[
            ('WEAPON', 'Weapon', 'Weapon item'),
            ('GRIMOIRE', 'Grimoire', 'Grimoire (spellbook)'),
            ('CONSUMABLE', 'Consumable', 'Consumable item'),
        ],
        default='CONSUMABLE',
    )

    # ===== TriggerZone Properties =====
    trigger_event: StringProperty(
        name="Event",
        description="Event to trigger (e.g., 'level_complete', 'spawn_boss')",
        default="custom_event",
    )

# ============ Registration ============

classes = [
    SKOPE_PG_ComponentProperties,
]

def register():
    """Register property classes"""
    for cls in classes:
        bpy.utils.register_class(cls)

    # Attach to Object
    bpy.types.Object.skope_component = bpy.props.PointerProperty(
        type=SKOPE_PG_ComponentProperties
    )

def unregister():
    """Unregister property classes"""
    del bpy.types.Object.skope_component

    for cls in reversed(classes):
        bpy.utils.unregister_class(cls)
