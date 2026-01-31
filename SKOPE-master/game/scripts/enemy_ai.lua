-- SKOPE Engine - Enemy AI Script
-- 간단한 적 AI (플레이어 추적)

local EnemyAI = {}

-- 설정
EnemyAI.move_speed = 3.0
EnemyAI.detection_range = 10.0
EnemyAI.attack_range = 2.0
EnemyAI.attack_cooldown = 1.5

-- 상태
EnemyAI.state = "idle"  -- idle, chase, attack
EnemyAI.target_position = nil
EnemyAI.attack_timer = 0
EnemyAI.patrol_points = {}
EnemyAI.current_patrol_index = 1

function EnemyAI:on_start(ctx)
    print("EnemyAI started - State: " .. self.state)

    -- 순찰 지점 설정 (예시)
    self.patrol_points = {
        SKOPE.Vec3.new(5, 0, 5),
        SKOPE.Vec3.new(-5, 0, 5),
        SKOPE.Vec3.new(-5, 0, -5),
        SKOPE.Vec3.new(5, 0, -5),
    }

    self.position = ctx.position
end

function EnemyAI:on_update(ctx)
    local dt = ctx.delta_time
    self.position = ctx.position

    -- 공격 쿨다운 업데이트
    if self.attack_timer > 0 then
        self.attack_timer = self.attack_timer - dt
    end

    -- 상태 머신
    if self.state == "idle" then
        self:do_patrol(dt)
    elseif self.state == "chase" then
        self:do_chase(dt)
    elseif self.state == "attack" then
        self:do_attack(dt)
    end

    -- 결과 반환
    return {
        position = self.position,
        state = self.state
    }
end

-- 순찰
function EnemyAI:do_patrol(dt)
    if #self.patrol_points == 0 then
        return
    end

    local target = self.patrol_points[self.current_patrol_index]
    local dist = SKOPE.Vec3.distance(self.position, target)

    if dist < 0.5 then
        -- 다음 순찰 지점으로
        self.current_patrol_index = self.current_patrol_index + 1
        if self.current_patrol_index > #self.patrol_points then
            self.current_patrol_index = 1
        end
    else
        -- 순찰 지점으로 이동
        self:move_toward(target, self.move_speed * 0.5 * dt)
    end
end

-- 추적
function EnemyAI:do_chase(dt)
    if not self.target_position then
        self.state = "idle"
        return
    end

    local dist = SKOPE.Vec3.distance(self.position, self.target_position)

    if dist <= self.attack_range then
        self.state = "attack"
    elseif dist > self.detection_range then
        -- 타겟을 놓침
        self.state = "idle"
        self.target_position = nil
        print("Target lost!")
    else
        -- 추적
        self:move_toward(self.target_position, self.move_speed * dt)
    end
end

-- 공격
function EnemyAI:do_attack(dt)
    if self.attack_timer <= 0 then
        print("Enemy attacks!")
        self.attack_timer = self.attack_cooldown

        -- TODO: 실제 데미지 처리
    end

    -- 공격 후 다시 추적 상태로
    if self.target_position then
        local dist = SKOPE.Vec3.distance(self.position, self.target_position)
        if dist > self.attack_range then
            self.state = "chase"
        end
    else
        self.state = "idle"
    end
end

-- 목표를 향해 이동
function EnemyAI:move_toward(target, distance)
    local direction = SKOPE.Vec3.sub(target, self.position)
    direction = SKOPE.Vec3.normalize(direction)
    local move = SKOPE.Vec3.mul(direction, distance)
    self.position = SKOPE.Vec3.add(self.position, move)
end

-- 타겟 설정 (외부에서 호출)
function EnemyAI:set_target(target_pos)
    self.target_position = target_pos
    if self.state == "idle" then
        self.state = "chase"
        print("Target acquired!")
    end
end

function EnemyAI:on_destroy()
    print("EnemyAI destroyed")
end

return EnemyAI
