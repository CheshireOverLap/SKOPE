# SKOPE Lua Quick Reference

## 스크립트 템플릿
```lua
local Script = {}
function Script:on_start(ctx) end
function Script:on_update(ctx)
    return { position = {x,y,z}, rotation = {axis,angle} }
end
return Script
```

## Vec3
```lua
SKOPE.Vec3.new(x,y,z) | .zero() | .one() | .up() | .forward()
.add(a,b) | .sub(a,b) | .mul(v,s) | .length(v) | .normalize(v)
.dot(a,b) | .cross(a,b) | .lerp(a,b,t) | .distance(a,b)
```

## Quat
```lua
SKOPE.Quat.identity()
.from_axis_angle(axis, rad) | .from_euler(x,y,z) | .from_euler_deg(x,y,z)
.mul(a,b) | .normalize(q) | .inverse(q) | .slerp(a,b,t)
.rotate_vec3(q,v) | .look_at(forward, up)
```

## Math
```lua
SKOPE.Math.PI | .TAU | .clamp(v,min,max) | .lerp(a,b,t)
.smoothstep(e0,e1,x) | .deg_to_rad(d) | .rad_to_deg(r) | .sign(v)
```

## Time (읽기 전용)
```lua
SKOPE.Time.delta | .elapsed | .frame_count | .fps
```

## Input
```lua
SKOPE.Input.is_key_pressed("W")
.get_axis("horizontal") | .get_axis("vertical")  -- -1 ~ 1
.get_mouse_position() | .get_mouse_delta()
```

## Entity
```lua
SKOPE.Entity.find("Name") | .find_all("Pattern") | .get_all() | .count()
.get_name(id) | .get_transform(id) | .get_position(id)
.distance(id1, id2) | .has_component(id, "Transform")
```

## Audio
```lua
SKOPE.Audio.play("sound.wav", volume, loop)
.play_music("bgm.mp3") | .stop_music() | .stop_all()
.set_volume(v) | .set_music_volume(v) | .set_sfx_volume(v)
```

## Collision
```lua
SKOPE.Collision.on_enter(id, fn) | .on_exit(id, fn)
.get_events() | .get_collisions_with(id) | .is_colliding(a,b)
```

## Debug
```lua
SKOPE.Debug.log(msg) | .warn(msg) | .error(msg)
.draw_line(from, to, color) | .draw_sphere(pos, r, color)
.draw_box(min, max, color) | .draw_point(pos, color, size)
.draw_axis(pos, size)
```

## 색상
```lua
{ r = 1, g = 0, b = 0, a = 1 }  -- RGBA (0~1)
```
