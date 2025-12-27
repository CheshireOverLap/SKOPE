-- SKOPE Engine - Rotator Script
-- 오브젝트를 회전시키는 간단한 스크립트

local Rotator = {}

-- 설정값
Rotator.speed = 45.0  -- 초당 회전 각도
Rotator.axis = "y"    -- 회전 축 (x, y, z)

-- 상태
Rotator.current_angle = 0

function Rotator:on_start(ctx)
    print("Rotator initialized - speed: " .. self.speed .. " deg/s")
end

function Rotator:on_update(ctx)
    local dt = ctx.delta_time

    -- 각도 업데이트
    self.current_angle = self.current_angle + self.speed * dt

    -- 360도 넘으면 리셋
    if self.current_angle >= 360 then
        self.current_angle = self.current_angle - 360
    end

    -- 회전값 반환 (라디안)
    local rad = SKOPE.Math.deg_to_rad(self.current_angle)

    return {
        rotation = {
            axis = self.axis,
            angle = rad
        }
    }
end

return Rotator
