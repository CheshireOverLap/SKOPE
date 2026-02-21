# SKOPE Gameplay Scripting System

Roblox 스타일 Lua 스크립팅 시스템. EntityHandle + Signal + 코루틴 스케줄러 + Combat API.

---

## 아키텍처 개요

```
Lua Script                    Rust (ECS)
──────────                    ──────────
entity.OnDamaged:Connect(fn)  ←── signal_bridge::fire_signal()
task.spawn(fn)                ──→ TaskScheduler (coroutine resume)
Combat.damage(entity, opts)   ──→ combat_command_system → Health 컴포넌트
Tag.add(entity, "boss")       ──→ tag_command_system → Tags 컴포넌트
Tween.create(entity, opts)    ──→ tween_update_system → Transform 보간
require("skills/fireball")    ──→ module_loader (파일 로드 + 캐싱 + 검증)
```

### ECS 시스템 체인 (매 프레임)

```
entity_sync → task_scheduler_tick → script_update → combat_command →
status_effect_tick → tag_command → tween_update → debug_draw_sync
```

### Rust-Lua 경계

| 기능 | Rust | Lua |
|------|------|-----|
| EntityHandle | UserData (fields/methods) | — |
| Signal | fire_signal() 호출 | Connect/Fire/Once/DisconnectAll |
| task.wait | — | coroutine.yield(seconds) |
| task.spawn | 코루틴 생성+resume+스케줄링 | — |
| Combat API | command queue drain + ECS 반영 | API 표면 (얇은 래퍼) |
| require | 파일 로드 + 캐싱 + 검증 | — |
| Tag/Tween | ECS 시스템 | API 표면 |

---

## 1. EntityHandle

`Entity.find(name)` → EntityHandle UserData 반환. 프로퍼티 스타일 접근.

```lua
local boss = Entity.find("DragonBoss")

-- 프로퍼티 읽기
print(boss.name)              -- "DragonBoss"
print(boss.position.x)        -- Transform.translation.x
print(boss.health)            -- Health.current
print(boss.max_health)        -- Health.maximum
print(boss.health_percent)    -- current / max (0.0~1.0)
print(boss.is_alive)          -- Health 없으면 true, 있으면 current > 0
print(boss.team)              -- "Player" | "Enemy" | "Neutral"
print(boss.id)                -- entity bits (u64)

-- 메서드
local dist = boss:distance_to(other_entity)
local has = boss:has_tag("elite")

-- 비교
if boss == other_entity then ... end

-- 시그널 (아래 참조)
boss.OnDamaged:Connect(function(amount, source, damage_type)
    print("Took " .. amount .. " damage!")
end)
```

### Entity 검색 API

```lua
Entity.find("name")          -- EntityHandle 또는 nil
Entity.find_by_id(id)        -- EntityHandle 또는 nil
Entity.find_all("pattern")   -- EntityHandle 배열 (이름에 패턴 포함)
Entity.get_all()              -- 모든 entity ID 배열
Entity.count()                -- 엔티티 수
```

### 파일 위치

- `src/scripting/entity_handle.rs` — UserData 구현
- `src/scripting/entity_api.rs` — Entity 검색 API

---

## 2. Signal System

순수 Lua 구현. Roblox의 `RBXScriptSignal` 패턴.

### 사용법

```lua
-- Connect: 시그널에 콜백 연결
local conn = entity.OnDamaged:Connect(function(amount, source, damage_type)
    print("피해 " .. amount)
end)

-- Once: 1회만 호출 후 자동 해제
entity.Died:Once(function(killer_bits)
    print("사망!")
end)

-- Disconnect
conn:Disconnect()
print(conn.Connected)  -- false

-- DisconnectAll: 모든 연결 해제
entity.OnDamaged:DisconnectAll()

-- 커스텀 시그널 생성
local mySignal = Signal.new()
mySignal:Fire("hello", 42)
```

### 내장 시그널

| 시그널 | 인자 | 발화 시점 |
|--------|------|----------|
| `OnDamaged` | `(amount, source_bits, damage_type)` | Combat.damage 처리 후 |
| `Died` | `(killer_bits)` | 체력 0 이하 |
| `OnCollisionEnter` | — | 충돌 시작 (미연결) |
| `OnCollisionExit` | — | 충돌 종료 (미연결) |
| `OnStatusApplied` | `(status_name, duration)` | 상태효과 적용 |
| `OnStatusRemoved` | `(status_name)` | 상태효과 만료/제거 |

### 동작 원리

- Signal은 lazy 생성: `entity.OnDamaged`에 처음 접근할 때 `__signals[entity_bits]["OnDamaged"]`에 Signal.new() 저장
- Rust에서 `signal_bridge::fire_signal(lua, entity_bits, "OnDamaged", args)` 호출 시 해당 Signal의 `Fire()` 실행
- Signal이 존재하지 않으면(= 리스너 없음) fire_signal은 no-op
- `cleanup_entity_signals()`로 엔티티 파괴 시 모든 시그널 정리

### 파일 위치

- `src/scripting/api.rs` — Signal Lua 클래스 등록 (`register_signal_class`)
- `src/scripting/signal_bridge.rs` — Rust→Lua 시그널 발화

---

## 3. Task (코루틴 스케줄러)

보스 패턴, 스킬 시퀀스를 순차 코드로 작성.

### API

```lua
-- task.spawn: 코루틴 생성, 즉시 시작
local id = task.spawn(function()
    print("1")
    task.wait(2.0)   -- 2초 대기 (yield)
    print("2")       -- 2초 후 resume
    task.wait(1.5)
    print("3")       -- 1.5초 후
end)

-- task.delay: 지정 시간 후 실행
local id = task.delay(3.0, function()
    print("3초 후 실행")
end)

-- task.defer: 다음 프레임에 실행
task.defer(function()
    print("다음 프레임")
end)

-- task.cancel: 대기 중인 코루틴 취소
task.cancel(id)
```

### 보스 패턴 예시

```lua
function BossScript:on_start(ctx)
    self.entity = Entity.find("DragonBoss")

    self.entity.OnDamaged:Connect(function(amount, source)
        if self.entity.health_percent < 0.5 then
            self:enter_phase2()
        end
    end)

    task.spawn(function()
        while self.entity.is_alive do
            self:flame_breath()
            task.wait(2.0)
            self:tail_swipe()
            task.wait(1.5)
        end
    end)
end
```

### 동작 원리

- `task.wait(seconds)` = 순수 Lua: `coroutine.yield(seconds)` (Lua 5.4 yield 규칙 준수)
- `task.spawn(fn)` = Rust: 코루틴 생성 → 즉시 resume → yield 시 스케줄러에 등록
- `TaskScheduler.tick()`: 매 프레임 대기 시간 만료된 코루틴을 `coroutine.resume()`
- thread_id는 Lua 쪽에서 미리 할당 → `task.cancel(id)`과 정확히 매칭
- `cancel_entity(bits)`: 엔티티 파괴 시 소유 코루틴 일괄 취소

### 파일 위치

- `src/scripting/task_scheduler.rs` — TaskScheduler 구현
- `src/scripting/api.rs` — `register_task_library()`
- `src/ecs_systems/scripting.rs` — `task_scheduler_tick_system`

---

## 4. Combat API

데미지/힐/상태효과를 안전하게 적용. Command-queue 패턴으로 ECS 반영.

### API

```lua
-- 데미지
Combat.damage(target, {
    amount = 50,
    source = self.entity,       -- EntityHandle 또는 raw ID
    damage_type = "fire",       -- 기본값: "default"
    ignore_invincibility = false,
})

-- 힐
Combat.heal(target, 25)
Combat.heal(target, 25, source_entity)  -- source 명시

-- 상태효과 적용
Combat.apply_status(target, "burning", {
    duration = 5.0,          -- 초
    tick_damage = 10,        -- 틱당 데미지
    tick_interval = 1.0,     -- 틱 간격 (초)
})

-- 상태효과 제거
Combat.remove_status(target, "burning")

-- 무적
Combat.set_invincible(target, 1.5)  -- 1.5초 무적
```

### ECS 컴포넌트

```rust
// crates/skope_core/src/components/gameplay.rs

struct StatusEffects {
    effects: Vec<ActiveStatusEffect>,
    invincible_until: f64,
}

struct ActiveStatusEffect {
    name: String,
    remaining: f32,
    tick_interval: f32,
    time_since_tick: f32,
    tick_damage: f32,
}
```

### 시스템 체인

1. Lua `Combat.damage()` → `_command_queue`에 push
2. `combat_command_system` → 큐 drain → Health 수정 → `fire_signal("OnDamaged")`/`fire_signal("Died")`
3. `status_effect_tick_system` → 효과 지속시간 감소, 틱 데미지 적용, 만료 제거

### 파일 위치

- `src/scripting/combat_api.rs` — Lua API + CombatCommand enum
- `src/ecs_systems/combat.rs` — ECS 시스템

---

## 5. Tag System

엔티티 그룹핑/범위 공격 대상 선정.

### API

```lua
Tag.add(entity, "enemy")
Tag.add(entity, "elite")
Tag.remove(entity, "elite")
Tag.has(entity, "enemy")           -- true/false (즉시 읽기)

local enemies = Tag.getTagged("enemy")  -- EntityHandle 배열
for _, e in ipairs(enemies) do
    Combat.damage(e, { amount = 10 })
end
```

### 동작 원리

- `Tag.add/remove` → command queue → `tag_command_system`이 ECS `Tags` 컴포넌트 수정
- `Tag.has` → entity registry에서 즉시 읽기 (커맨드 큐 안 거침)
- `Tag.getTagged` → registry 순회, 매칭 엔티티를 EntityHandle 배열로 반환

### 파일 위치

- `src/scripting/tag_api.rs` — Lua API
- `src/ecs_systems/scripting.rs` — `tag_command_system`
- `crates/skope_core/src/components/gameplay.rs` — `Tags` 컴포넌트

---

## 6. Tween System

Transform 보간. Rust 시스템이 수행하므로 코루틴보다 성능 우수.

### API

```lua
local tween = Tween.create(entity, {
    duration = 2.0,
    easing = "ease_in_out",   -- "linear" | "ease_in" | "ease_out" | "ease_in_out"
    position = Vec3.new(10, 5, 0),
    -- rotation = { x=0, y=0, z=0, w=1 },  -- 선택
    -- scale = Vec3.new(2, 2, 2),           -- 선택
    auto_play = false,
})

tween:Play()
tween:Pause()
tween:Cancel()

tween.Completed:Connect(function()
    print("트윈 완료!")
end)
```

### 이징 함수

| 이름 | 수식 |
|------|------|
| `linear` | `t` |
| `ease_in` | `t^2` |
| `ease_out` | `t(2-t)` |
| `ease_in_out` | `t<0.5: 2t^2`, `else: -1+(4-2t)t` |

### 동작 원리

- `Tween.create` → command queue + Lua 핸들 반환
- `tween_update_system` → 커맨드 처리, 시작값 캡처(첫 틱), `Vec3::lerp`/`Quat::slerp` 보간
- 완료 시 `fire_tween_completed()` → Completed Signal 발화 + 핸들 정리

### 파일 위치

- `src/scripting/tween_api.rs` — Lua API
- `src/ecs_systems/tween.rs` — ECS 시스템 + `ActiveTweens` 리소스

---

## 7. Module System (require)

공유 라이브러리를 모듈로 분리.

### 사용법

```lua
local Fireball = require("skills/fireball")
Fireball.cast(caster, target_pos)

local Combat = require("skope/combat")
```

### 경로 규칙

| Trust Level | 접근 범위 |
|-------------|----------|
| `AiGenerated` | require 사용 불가 |
| `UserScript` | `modules/`, `skope/` 하위만 |
| `GameScript`, `Engine` | `scripts/` 전체 |

- 디렉토리 탈출(`..`) 차단
- 결과 캐싱 (`__module_cache`)
- 핫 리로드: `ModuleCache::invalidate_by_path()`로 캐시 무효화

### 파일 위치

- `src/scripting/module_loader.rs`

---

## 파일 구조

```
src/scripting/
├── api.rs              # 전체 API 등록 (Signal, task, 각 API 호출)
├── entity_handle.rs    # EntityHandle UserData + extract_entity_bits()
├── signal_bridge.rs    # Rust → Lua 시그널 발화
├── task_scheduler.rs   # 코루틴 스케줄러 (TaskScheduler)
├── combat_api.rs       # Combat Lua API + CombatCommand
├── tag_api.rs          # Tag Lua API + TagCommand
├── tween_api.rs        # Tween Lua API + TweenCommand
├── module_loader.rs    # require() 구현 + ModuleCache
├── entity_api.rs       # Entity 검색 API (find, find_all 등)
└── ...

src/ecs_systems/
├── scripting.rs        # entity_sync, task_tick, tag_command, debug_draw
├── combat.rs           # combat_command_system, status_effect_tick_system
├── tween.rs            # tween_update_system, ActiveTweens
└── ...

crates/skope_core/src/components/
└── gameplay.rs         # Health, StatusEffects, ActiveStatusEffect, Tags
```

---

## 설계 결정 사항

### Lua 5.4 yield 제약

`coroutine.yield()`는 C 함수(Rust closure) 내부에서 호출 불가. 따라서:
- `task.wait()` = **순수 Lua 함수** (`coroutine.yield(seconds)`)
- `task.spawn()` = **Rust 함수** (코루틴 생성 + 즉시 resume + 스케줄러 등록)

### Command-Queue 패턴

Lua에서 직접 ECS 컴포넌트를 수정하지 않음. 대신:
1. Lua 함수가 커맨드를 큐에 push
2. Rust ECS 시스템이 매 프레임 큐를 drain하고 컴포넌트에 반영
3. 무한 루프 방지 + borrow checker 호환 + 실행 순서 보장

### Borrow Splitting (`std::mem::take`)

`ScriptEngine`은 `task_scheduler`와 `lua` 필드를 동시에 접근해야 함.
`&mut self.task_scheduler`와 `&self.lua`는 Rust borrow checker가 거부.
해결: `std::mem::take(&mut self.task_scheduler)` → 스케줄러를 꺼내서 사용 → 다시 넣음.

### EntityHandle vs Raw ID

모든 API가 EntityHandle UserData와 raw u64/i64를 모두 받음 (`extract_entity_bits()`).
하위 호환성 유지하면서 새 코드는 EntityHandle 사용 권장.
