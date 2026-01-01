"""
SKOPE Live Link - WebSocket client for Blender
Provides real-time bidirectional communication between Blender and SKOPE engine.
"""

import bpy
import json
import threading
import queue
import time
from typing import Optional, Dict, Any, List, Callable

# Try to import websocket library
try:
    import websocket
    WEBSOCKET_AVAILABLE = True
except ImportError:
    WEBSOCKET_AVAILABLE = False
    print("SKOPE Live Link: websocket-client not installed. Run: pip install websocket-client")


class LiveLinkClient:
    """WebSocket client for SKOPE Live Link"""

    def __init__(self, host: str = "localhost", port: int = 9999):
        self.host = host
        self.port = port
        self.ws: Optional[websocket.WebSocket] = None
        self.connected = False
        self.running = False

        # Message queues
        self.send_queue: queue.Queue = queue.Queue()
        self.receive_queue: queue.Queue = queue.Queue()

        # Callbacks
        self.on_connect: Optional[Callable] = None
        self.on_disconnect: Optional[Callable] = None
        self.on_error: Optional[Callable[[str], None]] = None

        # Worker thread
        self.thread: Optional[threading.Thread] = None

    def connect(self) -> bool:
        """Connect to SKOPE engine"""
        if not WEBSOCKET_AVAILABLE:
            print("SKOPE Live Link: websocket-client not available")
            return False

        if self.connected:
            return True

        try:
            url = f"ws://{self.host}:{self.port}"
            self.ws = websocket.create_connection(url, timeout=5)
            self.connected = True
            self.running = True

            # Start worker thread
            self.thread = threading.Thread(target=self._worker, daemon=True)
            self.thread.start()

            # Send connected message
            self.send({
                "type": "Connected",
                "client_name": "Blender"
            })

            if self.on_connect:
                self.on_connect()

            print(f"SKOPE Live Link: Connected to {url}")
            return True

        except Exception as e:
            print(f"SKOPE Live Link: Connection failed - {e}")
            if self.on_error:
                self.on_error(str(e))
            return False

    def disconnect(self):
        """Disconnect from SKOPE engine"""
        self.running = False
        self.connected = False

        if self.ws:
            try:
                self.ws.close()
            except:
                pass
            self.ws = None

        if self.on_disconnect:
            self.on_disconnect()

        print("SKOPE Live Link: Disconnected")

    def send(self, message: Dict[str, Any]):
        """Queue a message to send"""
        if self.connected:
            self.send_queue.put(message)

    def receive(self) -> Optional[Dict[str, Any]]:
        """Get a received message (non-blocking)"""
        try:
            return self.receive_queue.get_nowait()
        except queue.Empty:
            return None

    def _worker(self):
        """Background worker for WebSocket I/O"""
        while self.running and self.ws:
            try:
                # Send queued messages
                while not self.send_queue.empty():
                    msg = self.send_queue.get_nowait()
                    self.ws.send(json.dumps(msg))

                # Receive messages (with short timeout)
                self.ws.settimeout(0.1)
                try:
                    data = self.ws.recv()
                    if data:
                        msg = json.loads(data)
                        self.receive_queue.put(msg)

                        # Handle ping
                        if msg.get("type") == "Ping":
                            self.send({"type": "Pong"})
                except websocket.WebSocketTimeoutException:
                    pass

            except Exception as e:
                print(f"SKOPE Live Link: Worker error - {e}")
                self.disconnect()
                break

            time.sleep(0.01)


# Blender operators
class SKOPE_OT_live_link_connect(bpy.types.Operator):
    """Connect to SKOPE engine via WebSocket"""
    bl_idname = "skope.live_link_connect"
    bl_label = "Connect to SKOPE"
    bl_description = "Connect to SKOPE engine for real-time sync"

    def execute(self, context):
        client = get_live_link_client()
        settings = context.scene.skope_settings

        if client.connect():
            self.report({'INFO'}, "Connected to SKOPE engine")
        else:
            self.report({'ERROR'}, "Failed to connect to SKOPE engine")

        return {'FINISHED'}


class SKOPE_OT_live_link_disconnect(bpy.types.Operator):
    """Disconnect from SKOPE engine"""
    bl_idname = "skope.live_link_disconnect"
    bl_label = "Disconnect"
    bl_description = "Disconnect from SKOPE engine"

    def execute(self, context):
        client = get_live_link_client()
        client.disconnect()
        self.report({'INFO'}, "Disconnected from SKOPE engine")
        return {'FINISHED'}


class SKOPE_OT_live_link_sync_scene(bpy.types.Operator):
    """Request scene sync from SKOPE engine"""
    bl_idname = "skope.live_link_sync_scene"
    bl_label = "Sync Scene"
    bl_description = "Request current scene data from SKOPE engine"

    def execute(self, context):
        client = get_live_link_client()
        if not client.connected:
            self.report({'ERROR'}, "Not connected to SKOPE engine")
            return {'CANCELLED'}

        client.send({"type": "SceneSync"})
        self.report({'INFO'}, "Scene sync requested")
        return {'FINISHED'}


class SKOPE_OT_live_link_play(bpy.types.Operator):
    """Send play command to SKOPE engine"""
    bl_idname = "skope.live_link_play"
    bl_label = "Play"
    bl_description = "Start game playback in SKOPE engine"

    def execute(self, context):
        client = get_live_link_client()
        if not client.connected:
            self.report({'ERROR'}, "Not connected to SKOPE engine")
            return {'CANCELLED'}

        client.send({"type": "PlayRequest"})
        self.report({'INFO'}, "Play command sent")
        return {'FINISHED'}


class SKOPE_OT_live_link_stop(bpy.types.Operator):
    """Send stop command to SKOPE engine"""
    bl_idname = "skope.live_link_stop"
    bl_label = "Stop"
    bl_description = "Stop game playback in SKOPE engine"

    def execute(self, context):
        client = get_live_link_client()
        if not client.connected:
            self.report({'ERROR'}, "Not connected to SKOPE engine")
            return {'CANCELLED'}

        client.send({"type": "StopRequest"})
        self.report({'INFO'}, "Stop command sent")
        return {'FINISHED'}


class SKOPE_OT_live_link_send_transform(bpy.types.Operator):
    """Send selected object transform to SKOPE engine"""
    bl_idname = "skope.live_link_send_transform"
    bl_label = "Send Transform"
    bl_description = "Send selected object's transform to SKOPE engine"

    def execute(self, context):
        client = get_live_link_client()
        if not client.connected:
            self.report({'ERROR'}, "Not connected to SKOPE engine")
            return {'CANCELLED'}

        obj = context.active_object
        if not obj:
            self.report({'ERROR'}, "No active object")
            return {'CANCELLED'}

        # Get transform
        pos = list(obj.location)
        rot = list(obj.rotation_quaternion)
        scale = list(obj.scale)

        # Send update
        client.send({
            "type": "EntityUpdate",
            "entity": obj.name,
            "position": pos,
            "rotation": rot,
            "scale": scale
        })

        self.report({'INFO'}, f"Transform sent for '{obj.name}'")
        return {'FINISHED'}


# Modal operator for continuous sync
class SKOPE_OT_live_link_auto_sync(bpy.types.Operator):
    """Automatically sync transforms to SKOPE engine"""
    bl_idname = "skope.live_link_auto_sync"
    bl_label = "Auto Sync"
    bl_description = "Continuously sync selected objects to SKOPE engine"

    _timer = None
    _running = False
    _last_transforms: Dict[str, tuple] = {}

    def modal(self, context, event):
        if event.type == 'TIMER':
            client = get_live_link_client()
            if not client.connected:
                self.cancel(context)
                return {'CANCELLED'}

            # Check for transform changes
            for obj in context.selected_objects:
                current = (
                    tuple(obj.location),
                    tuple(obj.rotation_quaternion),
                    tuple(obj.scale)
                )
                last = self._last_transforms.get(obj.name)

                if last != current:
                    self._last_transforms[obj.name] = current
                    client.send({
                        "type": "EntityUpdate",
                        "entity": obj.name,
                        "position": list(obj.location),
                        "rotation": list(obj.rotation_quaternion),
                        "scale": list(obj.scale)
                    })

            # Process incoming messages
            while True:
                msg = client.receive()
                if not msg:
                    break
                self._handle_message(context, msg)

        elif event.type == 'ESC':
            self.cancel(context)
            return {'CANCELLED'}

        return {'PASS_THROUGH'}

    def _handle_message(self, context, msg: Dict[str, Any]):
        """Handle incoming message from SKOPE"""
        msg_type = msg.get("type")

        if msg_type == "ScriptError":
            # Show script error in Blender
            file_name = msg.get("file", "unknown")
            line = msg.get("line", 0)
            message = msg.get("message", "Unknown error")
            print(f"SKOPE Script Error in {file_name}:{line}: {message}")

        elif msg_type == "SceneData":
            # Could update Blender scene from engine data
            pass

        elif msg_type == "Log":
            level = msg.get("level", "INFO")
            message = msg.get("message", "")
            print(f"SKOPE [{level}]: {message}")

    def execute(self, context):
        if SKOPE_OT_live_link_auto_sync._running:
            self.report({'WARNING'}, "Auto sync already running")
            return {'CANCELLED'}

        client = get_live_link_client()
        if not client.connected:
            self.report({'ERROR'}, "Not connected to SKOPE engine")
            return {'CANCELLED'}

        SKOPE_OT_live_link_auto_sync._running = True
        SKOPE_OT_live_link_auto_sync._last_transforms = {}

        wm = context.window_manager
        self._timer = wm.event_timer_add(0.1, window=context.window)
        wm.modal_handler_add(self)

        self.report({'INFO'}, "Auto sync started (ESC to stop)")
        return {'RUNNING_MODAL'}

    def cancel(self, context):
        SKOPE_OT_live_link_auto_sync._running = False
        if self._timer:
            wm = context.window_manager
            wm.event_timer_remove(self._timer)
        self.report({'INFO'}, "Auto sync stopped")


# UI Panel
class SKOPE_PT_live_link(bpy.types.Panel):
    """SKOPE Live Link panel"""
    bl_label = "Live Link"
    bl_idname = "SKOPE_PT_live_link"
    bl_space_type = 'VIEW_3D'
    bl_region_type = 'UI'
    bl_category = 'SKOPE'
    bl_options = {'DEFAULT_CLOSED'}

    def draw(self, context):
        layout = self.layout
        client = get_live_link_client()

        # Connection status
        box = layout.box()
        row = box.row()
        if client.connected:
            row.label(text="Status: Connected", icon='CHECKMARK')
        else:
            row.label(text="Status: Disconnected", icon='X')

        # Connection buttons
        row = box.row(align=True)
        if client.connected:
            row.operator("skope.live_link_disconnect", icon='CANCEL')
        else:
            row.operator("skope.live_link_connect", icon='PLAY')

        if not client.connected:
            return

        layout.separator()

        # Playback controls
        box = layout.box()
        box.label(text="Playback", icon='PLAY')
        row = box.row(align=True)
        row.operator("skope.live_link_play", icon='PLAY')
        row.operator("skope.live_link_stop", icon='SNAP_FACE')  # STOP icon

        layout.separator()

        # Sync controls
        box = layout.box()
        box.label(text="Sync", icon='FILE_REFRESH')
        box.operator("skope.live_link_sync_scene")
        box.operator("skope.live_link_send_transform")

        row = box.row()
        if SKOPE_OT_live_link_auto_sync._running:
            row.label(text="Auto Sync: Running", icon='REC')
        else:
            row.operator("skope.live_link_auto_sync")


# Global client instance
_live_link_client: Optional[LiveLinkClient] = None


def get_live_link_client() -> LiveLinkClient:
    """Get or create the global Live Link client"""
    global _live_link_client
    if _live_link_client is None:
        _live_link_client = LiveLinkClient()
    return _live_link_client


# Registration
classes = [
    SKOPE_OT_live_link_connect,
    SKOPE_OT_live_link_disconnect,
    SKOPE_OT_live_link_sync_scene,
    SKOPE_OT_live_link_play,
    SKOPE_OT_live_link_stop,
    SKOPE_OT_live_link_send_transform,
    SKOPE_OT_live_link_auto_sync,
    SKOPE_PT_live_link,
]


def register():
    for cls in classes:
        bpy.utils.register_class(cls)


def unregister():
    # Disconnect on unregister
    client = get_live_link_client()
    client.disconnect()

    for cls in reversed(classes):
        bpy.utils.unregister_class(cls)
