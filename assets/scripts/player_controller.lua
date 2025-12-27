-- SKOPE Engine - Player Controller Script
-- 기본 플레이어 이동 컨트롤러

local PlayerController = {}

-- 설정값
PlayerController.move_speed = 5.0
PlayerController.look_sensitivity = 0.1
PlayerController.jump_force = 8.0

-- 상태
PlayerController.velocity = { x = 0, y = 0, z = 0 }
PlayerController.is_grounded = true

-- 초기화 (엔티티 생성 시 호출)
function PlayerController:on_start(ctx)
    print("PlayerController started on entity: " .. ctx.entity_id)
    self.position = ctx.position
    self.velocity = SKOPE.Vec3.zero()
end

-- 매 프레임 호출
function PlayerController:on_update(ctx)
    local dt = ctx.delta_time

    -- 입력 처리
    local h = SKOPE.Input.get_axis("horizontal")
    local v = SKOPE.Input.get_axis("vertical")

    -- 이동 방향 계산
    local move_dir = SKOPE.Vec3.new(h, 0, -v)

    -- 길이가 1보다 크면 정규화
    local len = SKOPE.Vec3.length(move_dir)
    if len > 1.0 then
        move_dir = SKOPE.Vec3.normalize(move_dir)
    end

    -- 속도 적용
    move_dir = SKOPE.Vec3.mul(move_dir, self.move_speed * dt)

    -- 위치 업데이트
    self.position = SKOPE.Vec3.add(ctx.position, move_dir)

    -- 점프
    if SKOPE.Input.is_key_pressed("Space") and self.is_grounded then
        self.velocity.y = self.jump_force
        self.is_grounded = false
        print("Jump!")
    end

    -- 중력 적용
    if not self.is_grounded then
        self.velocity.y = self.velocity.y - 9.8 * dt
        self.position.y = self.position.y + self.velocity.y * dt

        -- 바닥 체크 (간단한 예시)
        if self.position.y <= 0 then
            self.position.y = 0
            self.velocity.y = 0
            self.is_grounded = true
        end
    end

    -- 디버그 출력 (초당 1회)
    if math.floor(SKOPE.Time.elapsed) ~= math.floor(SKOPE.Time.elapsed - dt) then
        SKOPE.Debug.log(string.format("Pos: (%.2f, %.2f, %.2f) | FPS: %.0f",
            self.position.x, self.position.y, self.position.z,
            SKOPE.Time.fps))
    end

    -- 결과 반환 (Rust에서 Transform 업데이트에 사용)
    return {
        position = self.position,
        velocity = self.velocity
    }
end

-- 파괴 시 호출
function PlayerController:on_destroy()
    print("PlayerController destroyed")
end

return PlayerController
