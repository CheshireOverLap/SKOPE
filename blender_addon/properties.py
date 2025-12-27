"""
SKOPE Component Properties
Defines game component properties for Blender objects
Extended with more component types for full game editor experience
"""

import bpy
from bpy.props import (
    EnumProperty,
    StringProperty,
    IntProperty,
    FloatProperty,
    BoolProperty,
    PointerProperty,
    FloatVectorProperty,
)
from bpy.types import PropertyGroup


# ============ Scene-level Project Settings ============

class SKOPE_PG_ProjectSettings(PropertyGroup):
    """Project-level settings for SKOPE export"""

    project_path: StringProperty(
        name="Project Path",
        description="Path to SKOPE project root folder (contains assets/, levels/ etc.)",
        default="",
        subtype='DIR_PATH',
    )

    assets_subfolder: StringProperty(
        name="Assets Subfolder",
        description="Subfolder within project for assets (default: assets/models)",
        default="assets/models",
    )

    levels_subfolder: StringProperty(
        name="Levels Subfolder",
        description="Subfolder within project for level files (default: levels)",
        default="levels",
    )

    auto_export_gltf: BoolProperty(
        name="Auto Export glTF",
        description="Automatically export selected objects as glTF when exporting scene",
        default=True,
    )

    export_textures: BoolProperty(
        name="Export Textures",
        description="Include textures in glTF export",
        default=True,
    )

    # Engine executable path
    engine_path: StringProperty(
        name="Engine Path",
        description="Path to SKOPE engine executable",
        default="",
        subtype='FILE_PATH',
    )


# ============ Component Property Group ============

class SKOPE_PG_ComponentProperties(PropertyGroup):
    """Properties for SKOPE game components"""

    # Component type selector (extended)
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
            ('CAMERA', 'Camera', 'Game camera'),
            ('CHARACTER', 'Character', 'Playable or NPC character'),
            ('AUDIO_SOURCE', 'Audio Source', '3D audio emitter'),
            ('PARTICLE_EMITTER', 'Particle Emitter', 'Particle system'),
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

    # ===== Shading Model =====
    shading_model: EnumProperty(
        name="Shading Model",
        description="How this object should be shaded in the engine",
        items=[
            ('STANDARD', 'Standard PBR', 'Standard physically-based rendering'),
            ('FACE', 'Face', 'Stylized face shading with normal flattening'),
            ('SKIN', 'Skin', 'Subsurface scattering for skin'),
            ('EYE', 'Eye', 'Eye shading with parallax'),
            ('HAIR_CARD', 'Hair Card', 'Hair card shading'),
            ('HAIR_STRAND', 'Hair Strand', 'Individual hair strand shading'),
        ],
        default='STANDARD',
    )

    # ===== Outline Settings =====
    enable_outline: BoolProperty(
        name="Enable Outline",
        description="Draw outline around this object",
        default=True,
    )

    outline_thickness: FloatProperty(
        name="Outline Thickness",
        description="Outline thickness multiplier",
        default=1.0,
        min=0.0,
        max=5.0,
    )

    outline_color: FloatVectorProperty(
        name="Outline Color",
        description="Color of the outline",
        subtype='COLOR',
        default=(0.02, 0.01, 0.01),
        min=0.0,
        max=1.0,
    )

    # ===== Physics Properties =====
    physics_type: EnumProperty(
        name="Physics Type",
        description="Physics body type",
        items=[
            ('NONE', 'None', 'No physics simulation'),
            ('STATIC', 'Static', 'Immovable collider'),
            ('DYNAMIC', 'Dynamic', 'Fully simulated rigid body'),
            ('KINEMATIC', 'Kinematic', 'Script-controlled, affects others'),
        ],
        default='STATIC',
    )

    mass: FloatProperty(
        name="Mass",
        description="Mass in kg (only for dynamic bodies)",
        default=1.0,
        min=0.001,
        max=10000.0,
    )

    friction: FloatProperty(
        name="Friction",
        description="Surface friction coefficient",
        default=0.5,
        min=0.0,
        max=2.0,
    )

    restitution: FloatProperty(
        name="Restitution",
        description="Bounciness (0 = no bounce, 1 = perfect bounce)",
        default=0.0,
        min=0.0,
        max=1.0,
    )

    # ===== Tags & Layers =====
    tags: StringProperty(
        name="Tags",
        description="Comma-separated tags for gameplay logic (e.g., 'enemy,boss,destructible')",
        default="",
    )

    layer: IntProperty(
        name="Layer",
        description="Collision/render layer (0-31)",
        default=0,
        min=0,
        max=31,
    )

    # ===== Collider Properties =====
    collider_shape: EnumProperty(
        name="Shape",
        description="Collider shape",
        items=[
            ('BOX', 'Box', 'Box collider'),
            ('SPHERE', 'Sphere', 'Sphere collider'),
            ('CAPSULE', 'Capsule', 'Capsule collider'),
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
            ('KEY', 'Key', 'Key item'),
            ('ARMOR', 'Armor', 'Armor piece'),
        ],
        default='CONSUMABLE',
    )

    # ===== TriggerZone Properties =====
    trigger_event: StringProperty(
        name="Event",
        description="Event to trigger (e.g., 'level_complete', 'spawn_boss')",
        default="custom_event",
    )

    # ===== Light Properties (SKOPE specific) =====
    cast_shadows: BoolProperty(
        name="Cast Shadows",
        description="Whether this light casts shadows",
        default=True,
    )

    shadow_bias: FloatProperty(
        name="Shadow Bias",
        description="Bias for shadow mapping to prevent artifacts",
        default=0.001,
        min=0.0,
        max=0.1,
        precision=4,
    )

    # ===== Camera Properties =====
    camera_type: EnumProperty(
        name="Camera Type",
        description="Type of game camera",
        items=[
            ('MAIN', 'Main Camera', 'Main game camera'),
            ('CUTSCENE', 'Cutscene Camera', 'Camera for cutscenes'),
            ('DEBUG', 'Debug Camera', 'Debug/free camera'),
        ],
        default='MAIN',
    )

    is_main_camera: BoolProperty(
        name="Main Camera",
        description="Is this the main game camera",
        default=False,
    )

    # ===== Character Properties =====
    character_id: StringProperty(
        name="Character ID",
        description="Unique identifier for this character",
        default="player_character",
    )

    character_type: EnumProperty(
        name="Character Type",
        description="Type of character",
        items=[
            ('PLAYER', 'Player', 'Playable character'),
            ('NPC', 'NPC', 'Non-playable character'),
            ('ENEMY', 'Enemy', 'Enemy character'),
            ('BOSS', 'Boss', 'Boss character'),
        ],
        default='PLAYER',
    )

    team: EnumProperty(
        name="Team",
        description="Character's team affiliation",
        items=[
            ('PLAYER', 'Player', 'Player team'),
            ('ENEMY', 'Enemy', 'Enemy team'),
            ('NEUTRAL', 'Neutral', 'Neutral/no team'),
        ],
        default='PLAYER',
    )

    max_health: FloatProperty(
        name="Max Health",
        description="Maximum health points",
        default=100.0,
        min=1.0,
        max=10000.0,
    )

    move_speed: FloatProperty(
        name="Move Speed",
        description="Movement speed",
        default=5.0,
        min=0.0,
        max=100.0,
    )

    # ===== Audio Source Properties =====
    audio_clip: StringProperty(
        name="Audio Clip",
        description="Name or path of audio file to play",
        default="",
    )

    audio_volume: FloatProperty(
        name="Volume",
        description="Audio volume (0.0 - 1.0)",
        default=1.0,
        min=0.0,
        max=1.0,
    )

    audio_loop: BoolProperty(
        name="Loop",
        description="Loop audio playback",
        default=False,
    )

    audio_spatial: BoolProperty(
        name="3D Spatial",
        description="Enable 3D spatial audio",
        default=True,
    )

    audio_min_distance: FloatProperty(
        name="Min Distance",
        description="Distance at which audio is at full volume",
        default=1.0,
        min=0.0,
        max=100.0,
    )

    audio_max_distance: FloatProperty(
        name="Max Distance",
        description="Distance at which audio fades to zero",
        default=50.0,
        min=0.0,
        max=1000.0,
    )

    # ===== Particle Emitter Properties =====
    particle_type: EnumProperty(
        name="Particle Type",
        description="Type of particle effect",
        items=[
            ('FIRE', 'Fire', 'Fire particles'),
            ('SMOKE', 'Smoke', 'Smoke particles'),
            ('SPARKS', 'Sparks', 'Spark particles'),
            ('DUST', 'Dust', 'Dust particles'),
            ('MAGIC', 'Magic', 'Magic/spell particles'),
            ('CUSTOM', 'Custom', 'Custom particle system'),
        ],
        default='FIRE',
    )

    emission_rate: FloatProperty(
        name="Emission Rate",
        description="Particles emitted per second",
        default=10.0,
        min=0.0,
        max=1000.0,
    )

    particle_lifetime: FloatProperty(
        name="Lifetime",
        description="Particle lifetime in seconds",
        default=2.0,
        min=0.1,
        max=30.0,
    )


# ============ Registration ============

classes = [
    SKOPE_PG_ProjectSettings,
    SKOPE_PG_ComponentProperties,
]


def register():
    """Register property classes"""
    for cls in classes:
        bpy.utils.register_class(cls)

    # Attach ProjectSettings to Scene
    bpy.types.Scene.skope_project = bpy.props.PointerProperty(
        type=SKOPE_PG_ProjectSettings
    )

    # Attach ComponentProperties to Object
    bpy.types.Object.skope_component = bpy.props.PointerProperty(
        type=SKOPE_PG_ComponentProperties
    )


def unregister():
    """Unregister property classes"""
    del bpy.types.Object.skope_component
    del bpy.types.Scene.skope_project

    for cls in reversed(classes):
        bpy.utils.unregister_class(cls)
