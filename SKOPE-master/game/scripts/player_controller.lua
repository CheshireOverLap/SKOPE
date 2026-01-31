-- SKOPE Engine - Player Controller Script
-- WASD 이동 + 애니메이션 연동

local PlayerController = {}

-- 설정값
PlayerController.move_speed = 5.0
PlayerController.run_speed = 10.0
PlayerController.look_sensitivity = 2.0
PlayerController.jump_force = 8.0

-- 상태
PlayerController.velocity = { x = 0, y = 0, z = 0 }
PlayerController.is_grounded = true
PlayerController.is_running = false
PlayerController.facing_angle = 0  -- Y축 회전 (라디안)

-- 애니메이션 상태
PlayerController.current_anim = "Idle"
PlayerController.anim_blend = 0

-- 초기화 (엔티티 생성 시 호출)
function PlayerController:on_start(ctx)
    SKOPE.Debug.log("[Player] Controller started on entity: " .. ctx.entity_id)
    self.position = ctx.position or { x = 0, y = 0, z = 0 }
    self.velocity = { x = 0, y = 0, z = 0 }
    self.entity_id = ctx.entity_id

    -- 초기 애니메이션
    SKOPE.Animation.play(ctx.entity_id, "Survey")  -- Fox Idle
    self.current_anim = "Survey"
end

-- 매 프레임 호출
function PlayerController:on_update(ctx)
    local dt = ctx.delta_time

    -- 현재 위치 가져오기
    local pos = ctx.position or self.position

    -- 입력 처리
    local h = SKOPE.Input.get_axis("horizontal")  -- A/D
    local v = SKOPE.Input.get_axis("vertical")    -- W/S
    local shift = SKOPE.Input.is_key_down("ShiftLeft")

    -- 달리기 상태
    self.is_running = shift and (math.abs(h) > 0.1 or math.abs(v) > 0.1)

    -- 현재 속도 결정
    local speed = self.is_running and self.run_speed or self.move_speed

    -- 이동 방향 계산 (카메라 기준)
    local move_x = h
    local move_z = -v  -- 앞으로 갈 때 -Z

    -- 이동 벡터 정규화
    local len = math.sqrt(move_x * move_x + move_z * move_z)
    if len > 0.01 then
        move_x = move_x / len
        move_z = move_z / len

        -- 캐릭터 회전 (이동 방향을 바라보게)
        local target_angle = math.atan2(move_x, move_z)
        self.facing_angle = self:lerp_angle(self.facing_angle, target_angle, dt * 10)
    end

    -- 위치 업데이트
    if len > 0.01 then
        pos.x = pos.x + move_x * speed * dt
        pos.z = pos.z + move_z * speed * dt
    end

    -- 점프 처리
    if SKOPE.Input.is_key_pressed("Space") and self.is_grounded then
        self.velocity.y = self.jump_force
        self.is_grounded = false
        SKOPE.Debug.log("[Player] Jump!")
    end

    -- 중력 적용
    if not self.is_grounded then
        self.velocity.y = self.velocity.y - 20.0 * dt
        pos.y = pos.y + self.velocity.y * dt

        -- 바닥 체크
        if pos.y <= 0 then
            pos.y = 0
            self.velocity.y = 0
            self.is_grounded = true
        end
    end

    -- 애니메이션 전환
    self:update_animation(ctx, len)

    -- 위치 저장
    self.position = pos

    -- 쿼터니언 회전 계산 (Y축 기준)
    local half_angle = self.facing_angle / 2
    local rotation = {
        x = 0,
        y = math.sin(half_angle),
        z = 0,
        w = math.cos(half_angle)
    }

    -- 결과 반환
    return {
        position = pos,
        rotation = rotation,
        velocity = self.velocity
    }
end

-- 애니메이션 업데이트
function PlayerController:update_animation(ctx, move_magnitude)
    local target_anim = "Survey"  -- Idle

    if move_magnitude > 0.1 then
        if self.is_running then
            target_anim = "Run"
        else
            target_anim = "Walk"
        end
    end

    -- 애니메이션 변경
    if target_anim ~= self.current_anim then
        SKOPE.Animation.crossfade(ctx.entity_id, target_anim, 0.2)
        self.current_anim = target_anim
    end
end

-- 각도 보간 (래핑 처리)
function PlayerController:lerp_angle(from, to, t)
    local diff = to - from

    -- -PI ~ PI 범위로 래핑
    while diff > math.pi do diff = diff - 2 * math.pi end
    while diff < -math.pi do diff = diff + 2 * math.pi end

    return from + diff * t
end

-- 파괴 시 호출
function PlayerController:on_destroy()
    SKOPE.Debug.log("[Player] Controller destroyed")
end

-- 충돌 시 호출
function PlayerController:on_collision(ctx, other)
    -- 아이템 픽업 처리
    if other.tag == "pickup" then
        SKOPE.Debug.log("[Player] Picked up: " .. (other.name or "item"))
        -- 아이템 효과 적용
        if other.item_type == "health" then
            -- HP 회복
        elseif other.item_type == "mana" then
            -- MP 회복
        end
    end
end

-- 트리거 진입 시 호출
function PlayerController:on_trigger_enter(ctx, trigger)
    SKOPE.Debug.log("[Player] Entered trigger: " .. (trigger.name or "zone"))

    -- 이벤트 발생
    if trigger.event then
        SKOPE.Event.fire(trigger.event, {
            player = ctx.entity_id,
            trigger = trigger.name
        })
    end
end

return PlayerController
