# SKOPE Lua API Reference

SKOPE 엔진의 Lua 스크립팅 API 문서입니다.

## 목차
- [시작하기](#시작하기)
- [스크립트 구조](#스크립트-구조)
- [SKOPE.Vec3](#skopevec3)
- [SKOPE.Quat](#skopequat)
- [SKOPE.Math](#skopemath)
- [SKOPE.Time](#skopetime)
- [SKOPE.Input](#skopeinput)
- [SKOPE.Entity](#skopeentity)
- [SKOPE.Transform](#skopetransform)
- [SKOPE.Audio](#skopeaudio)
- [SKOPE.Collision](#skopecollision)
- [SKOPE.Debug](#skopedebug)

---

## 시작하기

### 스크립트 파일 위치
```
assets/scripts/
├── player.lua
├── enemy.lua
└── game_manager.lua
```

### 핫 리로드
스크립트 파일을 저장하면 자동으로 리로드됩니다. 게임을 재시작할 필요 없습니다.

---

## 스크립트 구조

모든 스크립트는 테이블을 반환해야 합니다:

```lua
local MyScript = {}

-- 스크립트 시작 시 한 번 호출
function MyScript:on_start(ctx)
    print("Script started!")
    self.speed = 5.0
end

-- 매 프레임 호출
function MyScript:on_update(ctx)
    -- ctx.delta_time: 프레임 시간 (초)
    -- ctx.position: 현재 위치 {x, y, z}
    -- ctx.rotation: 현재 회전 (쿼터니언)
    -- ctx.scale: 현재 스케일 {x, y, z}

    -- Transform 업데이트를 반환
    return {
        position = { x = 0, y = 0, z = 0 },
        rotation = { axis = "y", angle = 1.57 },
        scale = { x = 1, y = 1, z = 1 }
    }
end

-- 스크립트 제거 시 호출
function MyScript:on_destroy()
    print("Script destroyed!")
end

return MyScript
```

### 컨텍스트 (ctx)

| 필드 | 타입 | 설명 |
|------|------|------|
| `entity_id` | number | 현재 엔티티 ID |
| `delta_time` | number | 프레임 델타 시간 (초) |
| `position` | Vec3 | 현재 위치 |
| `rotation` | Quat | 현재 회전 |
| `scale` | Vec3 | 현재 스케일 |

---

## SKOPE.Vec3

3D 벡터 연산 API

### 생성

```lua
-- 새 벡터 생성
local v = SKOPE.Vec3.new(1, 2, 3)

-- 특수 벡터
local zero = SKOPE.Vec3.zero()      -- {0, 0, 0}
local one = SKOPE.Vec3.one()        -- {1, 1, 1}
local up = SKOPE.Vec3.up()          -- {0, 1, 0}
local forward = SKOPE.Vec3.forward() -- {0, 0, -1}
```

### 연산

```lua
local a = SKOPE.Vec3.new(1, 0, 0)
local b = SKOPE.Vec3.new(0, 1, 0)

-- 덧셈/뺄셈
local sum = SKOPE.Vec3.add(a, b)    -- {1, 1, 0}
local diff = SKOPE.Vec3.sub(a, b)   -- {1, -1, 0}

-- 스칼라 곱
local scaled = SKOPE.Vec3.mul(a, 2.0)  -- {2, 0, 0}

-- 길이
local len = SKOPE.Vec3.length(a)    -- 1.0

-- 정규화
local norm = SKOPE.Vec3.normalize(a)

-- 내적/외적
local dot = SKOPE.Vec3.dot(a, b)    -- 0.0
local cross = SKOPE.Vec3.cross(a, b) -- {0, 0, 1}

-- 보간
local lerped = SKOPE.Vec3.lerp(a, b, 0.5)  -- {0.5, 0.5, 0}

-- 거리
local dist = SKOPE.Vec3.distance(a, b)
```

---

## SKOPE.Quat

쿼터니언 (회전) API

### 생성

```lua
-- 단위 쿼터니언 (회전 없음)
local q = SKOPE.Quat.identity()

-- 축-각도로 생성 (라디안)
local rot = SKOPE.Quat.from_axis_angle(
    SKOPE.Vec3.up(),  -- 회전축
    math.pi / 2       -- 90도
)

-- 오일러 각도로 생성 (라디안)
local euler = SKOPE.Quat.from_euler(0, 1.57, 0)

-- 오일러 각도로 생성 (도)
local euler_deg = SKOPE.Quat.from_euler_deg(0, 90, 0)

-- 방향 바라보기
local look = SKOPE.Quat.look_at(
    SKOPE.Vec3.new(0, 0, -1),  -- forward
    SKOPE.Vec3.up()             -- up
)
```

### 연산

```lua
local a = SKOPE.Quat.from_euler_deg(0, 45, 0)
local b = SKOPE.Quat.from_euler_deg(0, 45, 0)

-- 쿼터니언 곱 (회전 합성)
local combined = SKOPE.Quat.mul(a, b)  -- 90도 회전

-- 정규화
local norm = SKOPE.Quat.normalize(a)

-- 역원
local inv = SKOPE.Quat.inverse(a)

-- 구면 보간 (부드러운 회전)
local slerped = SKOPE.Quat.slerp(a, b, 0.5)

-- 벡터 회전
local rotated = SKOPE.Quat.rotate_vec3(a, SKOPE.Vec3.forward())
```

---

## SKOPE.Math

수학 유틸리티

### 상수

```lua
SKOPE.Math.PI   -- 3.14159...
SKOPE.Math.TAU  -- 6.28318... (2 * PI)
SKOPE.Math.E    -- 2.71828...
```

### 함수

```lua
-- 값 제한
local clamped = SKOPE.Math.clamp(value, 0, 1)

-- 선형 보간
local lerped = SKOPE.Math.lerp(0, 100, 0.5)  -- 50

-- 부드러운 보간
local smooth = SKOPE.Math.smoothstep(0, 1, 0.5)

-- 각도 변환
local rad = SKOPE.Math.deg_to_rad(90)   -- 1.5708
local deg = SKOPE.Math.rad_to_deg(3.14) -- 180

-- 부호
local s = SKOPE.Math.sign(-5)  -- -1
```

---

## SKOPE.Time

시간 정보 (읽기 전용, 매 프레임 자동 업데이트)

```lua
-- 프레임 델타 시간 (초)
local dt = SKOPE.Time.delta

-- 게임 시작 후 경과 시간 (초)
local elapsed = SKOPE.Time.elapsed

-- 총 프레임 수
local frames = SKOPE.Time.frame_count

-- 현재 FPS
local fps = SKOPE.Time.fps
```

### 예제: 시간 기반 이동

```lua
function MyScript:on_update(ctx)
    local speed = 5.0
    local movement = speed * SKOPE.Time.delta

    return {
        position = {
            x = ctx.position.x + movement,
            y = ctx.position.y,
            z = ctx.position.z
        }
    }
end
```

---

## SKOPE.Input

입력 처리 API

### 키보드

```lua
-- 키 눌림 확인
if SKOPE.Input.is_key_pressed("W") then
    -- 앞으로 이동
end

-- 축 입력 (-1 ~ 1)
local h = SKOPE.Input.get_axis("horizontal")  -- A/D 또는 Left/Right
local v = SKOPE.Input.get_axis("vertical")    -- W/S 또는 Up/Down
```

### 마우스

```lua
-- 마우스 위치
local pos = SKOPE.Input.get_mouse_position()
print(pos.x, pos.y)

-- 마우스 이동량 (이번 프레임)
local delta = SKOPE.Input.get_mouse_delta()
print(delta.x, delta.y)
```

### 지원 키 목록

| 키 | 이름 |
|-----|------|
| W, A, S, D | 이동 |
| Space | 점프 |
| Shift | 달리기 |
| E | 상호작용 |
| Escape | 메뉴 |
| 1-9 | 숫자 |
| Up, Down, Left, Right | 화살표 |

### 예제: WASD 이동

```lua
function MyScript:on_update(ctx)
    local speed = 5.0 * SKOPE.Time.delta

    local h = SKOPE.Input.get_axis("horizontal")
    local v = SKOPE.Input.get_axis("vertical")

    return {
        position = {
            x = ctx.position.x + h * speed,
            y = ctx.position.y,
            z = ctx.position.z + v * speed
        }
    }
end
```

---

## SKOPE.Entity

엔티티 조회 및 조작

### 엔티티 찾기

```lua
-- 이름으로 찾기
local player_id = SKOPE.Entity.find("Player")

-- 패턴으로 여러 개 찾기
local enemies = SKOPE.Entity.find_all("Enemy")  -- "Enemy1", "Enemy2" 등

-- 모든 엔티티
local all = SKOPE.Entity.get_all()

-- 엔티티 개수
local count = SKOPE.Entity.count()
```

### 엔티티 정보

```lua
local id = SKOPE.Entity.find("Player")

-- 이름 가져오기
local name = SKOPE.Entity.get_name(id)

-- Transform 가져오기
local transform = SKOPE.Entity.get_transform(id)
-- transform.position, transform.rotation, transform.scale

-- 개별 컴포넌트
local pos = SKOPE.Entity.get_position(id)
local rot = SKOPE.Entity.get_rotation(id)
local scale = SKOPE.Entity.get_scale(id)

-- 두 엔티티 사이 거리
local dist = SKOPE.Entity.distance(id1, id2)

-- 컴포넌트 존재 확인
if SKOPE.Entity.has_component(id, "Transform") then
    -- ...
end
```

### 예제: 가장 가까운 적 찾기

```lua
function MyScript:find_nearest_enemy()
    local my_id = ctx.entity_id
    local enemies = SKOPE.Entity.find_all("Enemy")

    local nearest = nil
    local min_dist = math.huge

    for _, enemy_id in ipairs(enemies) do
        local dist = SKOPE.Entity.distance(my_id, enemy_id)
        if dist and dist < min_dist then
            min_dist = dist
            nearest = enemy_id
        end
    end

    return nearest, min_dist
end
```

---

## SKOPE.Transform

Transform 생성 헬퍼

```lua
-- 새 Transform 생성
local t = SKOPE.Transform.new(
    SKOPE.Vec3.new(0, 5, 0),      -- position
    SKOPE.Quat.identity(),         -- rotation
    SKOPE.Vec3.one()               -- scale
)

-- 기본 Transform
local identity = SKOPE.Transform.identity()
```

---

## SKOPE.Audio

오디오 재생 API

### 효과음

```lua
-- 효과음 재생
SKOPE.Audio.play("jump.wav")

-- 볼륨 지정 (0.0 ~ 1.0)
SKOPE.Audio.play("explosion.wav", 0.8)

-- 루프 재생
SKOPE.Audio.play("engine.wav", 1.0, true)

-- 모든 효과음 정지
SKOPE.Audio.stop_all()
```

### 배경 음악

```lua
-- 음악 재생 (자동 루프)
SKOPE.Audio.play_music("bgm_battle.mp3")

-- 음악 정지
SKOPE.Audio.stop_music()
```

### 볼륨 조절

```lua
-- 마스터 볼륨
SKOPE.Audio.set_volume(0.8)

-- 음악 볼륨
SKOPE.Audio.set_music_volume(0.5)

-- 효과음 볼륨
SKOPE.Audio.set_sfx_volume(1.0)
```

---

## SKOPE.Collision

충돌 감지 API

### 이벤트 핸들러

```lua
function MyScript:on_start(ctx)
    -- 충돌 시작 시 호출
    SKOPE.Collision.on_enter(ctx.entity_id, function(other_id)
        local name = SKOPE.Entity.get_name(other_id)
        print("Collided with: " .. name)
    end)

    -- 충돌 종료 시 호출
    SKOPE.Collision.on_exit(ctx.entity_id, function(other_id)
        print("No longer colliding")
    end)
end
```

### 충돌 조회

```lua
-- 현재 프레임의 모든 충돌 이벤트
local events = SKOPE.Collision.get_events()

-- 특정 엔티티와 충돌 중인 목록
local collisions = SKOPE.Collision.get_collisions_with(my_id)

-- 두 엔티티가 충돌 중인지 확인
if SKOPE.Collision.is_colliding(player_id, enemy_id) then
    -- 데미지 처리
end
```

---

## SKOPE.Debug

디버그 출력 및 시각화

### 로그

```lua
SKOPE.Debug.log("일반 메시지")
SKOPE.Debug.warn("경고 메시지")
SKOPE.Debug.error("에러 메시지")
```

### 디버그 드로우

```lua
-- 색상 정의
local red = { r = 1, g = 0, b = 0, a = 1 }
local green = { r = 0, g = 1, b = 0, a = 1 }

-- 라인 그리기
SKOPE.Debug.draw_line(
    SKOPE.Vec3.new(0, 0, 0),  -- from
    SKOPE.Vec3.new(1, 1, 1),  -- to
    red
)

-- 구 그리기
SKOPE.Debug.draw_sphere(
    SKOPE.Vec3.new(0, 1, 0),  -- center
    0.5,                       -- radius
    green
)

-- 박스 그리기
SKOPE.Debug.draw_box(
    SKOPE.Vec3.new(-1, 0, -1),  -- min
    SKOPE.Vec3.new(1, 2, 1),    -- max
    red
)

-- 점 그리기
SKOPE.Debug.draw_point(
    SKOPE.Vec3.new(0, 0, 0),
    red,
    0.1  -- size (optional)
)

-- 축 기즈모 그리기 (RGB = XYZ)
SKOPE.Debug.draw_axis(
    SKOPE.Vec3.new(0, 0, 0),
    1.0  -- size (optional)
)
```

---

## 전체 예제: 플레이어 컨트롤러

```lua
local Player = {}

function Player:on_start(ctx)
    self.speed = 5.0
    self.jump_force = 10.0
    self.is_grounded = true
    self.velocity_y = 0

    SKOPE.Debug.log("Player initialized!")
end

function Player:on_update(ctx)
    local dt = SKOPE.Time.delta

    -- 이동 입력
    local h = SKOPE.Input.get_axis("horizontal")
    local v = SKOPE.Input.get_axis("vertical")

    -- 새 위치 계산
    local new_x = ctx.position.x + h * self.speed * dt
    local new_z = ctx.position.z + v * self.speed * dt

    -- 점프
    if SKOPE.Input.is_key_pressed("Space") and self.is_grounded then
        self.velocity_y = self.jump_force
        self.is_grounded = false
    end

    -- 중력
    self.velocity_y = self.velocity_y - 20 * dt
    local new_y = ctx.position.y + self.velocity_y * dt

    -- 지면 체크
    if new_y <= 0 then
        new_y = 0
        self.velocity_y = 0
        self.is_grounded = true
    end

    -- 이동 방향으로 회전
    local rotation = ctx.rotation
    if h ~= 0 or v ~= 0 then
        local dir = SKOPE.Vec3.normalize(SKOPE.Vec3.new(h, 0, v))
        rotation = SKOPE.Quat.look_at(dir, SKOPE.Vec3.up())
    end

    -- 디버그: 속도 벡터 표시
    SKOPE.Debug.draw_line(
        ctx.position,
        SKOPE.Vec3.new(new_x, new_y, new_z),
        { r = 0, g = 1, b = 0, a = 1 }
    )

    return {
        position = { x = new_x, y = new_y, z = new_z },
        rotation = rotation
    }
end

return Player
```

---

## 버전 정보

- SKOPE Engine v0.4.0
- Lua 5.4 (mlua)
