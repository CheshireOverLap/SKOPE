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

    -- Spell/Trigger API 테스트
    print("=== Testing Spell/Trigger API ===")

    -- 테스트 스펠 정의
    SKOPE.Spell.define("test_fireball", {
        damage_type = "fire",
        base_damage = 25,
        cooldown = 1.0,
        on_cast = function(caster, target_pos)
            print("[Spell] Fireball cast at x=" .. target_pos.x .. ", y=" .. target_pos.y .. ", z=" .. target_pos.z)
            return { hit = true }
        end
    })
    print("[Test] Spell 'test_fireball' defined")

    -- 스펠 목록 확인
    local spells = SKOPE.Spell.list()
    print("[Test] Total spells defined: " .. #spells)

    -- 테스트 트리거 정의
    SKOPE.Trigger.define("test_zone", {
        shape = "sphere",
        radius = 5.0,
        position = {0, 1, 0},
        on_enter = function(entity, trigger)
            print("[Trigger] Entity " .. tostring(entity) .. " entered " .. trigger)
        end,
        on_exit = function(entity, trigger)
            print("[Trigger] Entity " .. tostring(entity) .. " exited " .. trigger)
        end
    })
    print("[Test] Trigger 'test_zone' defined")

    -- 트리거 목록 확인
    local triggers = SKOPE.Trigger.list()
    print("[Test] Total triggers defined: " .. #triggers)

    print("=== Spell/Trigger API Test Complete ===")
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
