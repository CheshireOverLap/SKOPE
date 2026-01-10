-- SKOPE Spell/Trigger API 테스트 스크립트
-- 이 스크립트는 Spell과 Trigger API의 기능을 테스트합니다.

local TestSpellTrigger = {}

-- 초기화
function TestSpellTrigger:on_start(ctx)
    SKOPE.Debug.log("=== Spell/Trigger Test Started ===")

    -- 파이어볼 스펠 정의
    SKOPE.Spell.define("fireball", {
        damage_type = "fire",
        base_damage = 50,
        cooldown = 2.0,
        range = 30.0,
        aoe_radius = 3.0,

        on_cast = function(caster, target_pos)
            SKOPE.Debug.log("Fireball cast by " .. tostring(caster))
            SKOPE.Debug.log("Target: x=" .. target_pos.x .. ", y=" .. target_pos.y .. ", z=" .. target_pos.z)

            -- 시각적 효과: 타겟 위치에 빨간 구 그리기
            SKOPE.Debug.draw_sphere(target_pos, 2.0, {r=1, g=0.3, b=0, a=0.8})

            return { projectile = true, speed = 20.0 }
        end,

        on_hit = function(caster, target)
            SKOPE.Debug.log("Fireball hit target: " .. tostring(target))
            -- 실제 게임에서는 여기서 데미지 적용
            -- SKOPE.Entity.apply_damage(target, 50, "fire")
        end,

        on_end = function(caster)
            SKOPE.Debug.log("Fireball effect ended")
        end
    })

    -- 힐 스펠 정의
    SKOPE.Spell.define("heal", {
        damage_type = "holy",
        base_damage = -30,  -- 음수 = 힐
        cooldown = 5.0,
        range = 20.0,
        mana_cost = 25,

        on_cast = function(caster, target_pos)
            SKOPE.Debug.log("Heal cast!")
            -- 녹색 힐 이펙트
            SKOPE.Debug.draw_sphere(target_pos, 1.5, {r=0.2, g=1, b=0.3, a=0.7})
            return { instant = true }
        end
    })

    SKOPE.Debug.log("Defined spells: fireball, heal")

    -- 정의된 스펠 목록 출력
    local spells = SKOPE.Spell.list()
    SKOPE.Debug.log("Spell list count: " .. #spells)

    -- 트리거 존 정의
    SKOPE.Trigger.define("safe_zone", {
        shape = "sphere",
        radius = 5.0,
        position = {0, 1, 0},

        filter = function(entity)
            -- 모든 엔티티 허용 (필터링 없음)
            return true
        end,

        on_enter = function(entity, trigger_name)
            SKOPE.Debug.log("Entity " .. tostring(entity) .. " entered " .. trigger_name)
            SKOPE.Debug.draw_sphere({x=0, y=1, z=0}, 5.0, {r=0, g=1, b=0, a=0.3})
        end,

        on_stay = function(entity, trigger_name, duration)
            -- 매 프레임 호출되므로 가끔만 로그
            if duration % 1.0 < 0.02 then
                SKOPE.Debug.log("Entity staying in " .. trigger_name .. " for " .. string.format("%.1f", duration) .. "s")
            end
        end,

        on_exit = function(entity, trigger_name)
            SKOPE.Debug.log("Entity " .. tostring(entity) .. " exited " .. trigger_name)
        end
    })

    -- 데미지 존 정의
    SKOPE.Trigger.define("damage_zone", {
        shape = "box",
        radius = 3.0,  -- box의 경우 half-extent
        position = {10, 1, 0},

        on_enter = function(entity, trigger_name)
            SKOPE.Debug.log("Entity entered damage zone!")
            -- 화상 효과 적용
            SKOPE.Spell.apply_effect(entity, "burning", 3.0, {
                damage_per_sec = 10
            })
        end,

        on_exit = function(entity, trigger_name)
            SKOPE.Debug.log("Entity left damage zone")
        end
    })

    SKOPE.Debug.log("Defined triggers: safe_zone, damage_zone")

    -- 정의된 트리거 목록 출력
    local triggers = SKOPE.Trigger.list()
    SKOPE.Debug.log("Trigger list count: " .. #triggers)

    -- 저장할 상태
    self.cast_timer = 0
    self.entity_id = ctx.entity_id
end

-- 매 프레임 업데이트
function TestSpellTrigger:on_update(ctx)
    local dt = ctx.delta_time or SKOPE.Time.delta

    -- 타이머 업데이트
    self.cast_timer = self.cast_timer + dt

    -- 3초마다 파이어볼 시전 테스트
    if self.cast_timer >= 3.0 then
        self.cast_timer = 0

        -- 스펠 쿨다운 체크
        if SKOPE.Spell.is_ready("fireball", ctx.entity_id) then
            -- 앞쪽으로 5유닛 위치에 시전
            local pos = ctx.position
            local target = {pos.x + 5, pos.y, pos.z}

            local success = SKOPE.Spell.cast("fireball", ctx.entity_id, target)
            if success then
                SKOPE.Debug.log("Fireball cast successful!")
            else
                SKOPE.Debug.log("Fireball cast failed (cooldown or other reason)")
            end
        else
            local remaining = SKOPE.Spell.get_cooldown("fireball", ctx.entity_id)
            SKOPE.Debug.log("Fireball on cooldown: " .. string.format("%.1f", remaining) .. "s remaining")
        end
    end

    -- 트리거 존 시각화 (safe_zone)
    local safe_zone_pos = SKOPE.Trigger.get_position("safe_zone")
    if safe_zone_pos then
        SKOPE.Debug.draw_sphere(safe_zone_pos, 5.0, {r=0, g=0.5, b=1, a=0.2})
    end

    -- 트리거 존 시각화 (damage_zone)
    local damage_zone_pos = SKOPE.Trigger.get_position("damage_zone")
    if damage_zone_pos then
        SKOPE.Debug.draw_box(
            {x=damage_zone_pos.x - 3, y=damage_zone_pos.y - 3, z=damage_zone_pos.z - 3},
            {x=damage_zone_pos.x + 3, y=damage_zone_pos.y + 3, z=damage_zone_pos.z + 3},
            {r=1, g=0.2, b=0.2, a=0.3}
        )
    end

    -- 트리거 내 엔티티 확인
    local entities_in_safe = SKOPE.Trigger.get_entities_in("safe_zone")
    if #entities_in_safe > 0 then
        -- SKOPE.Debug.log("Entities in safe_zone: " .. #entities_in_safe)
    end

    -- 현재 엔티티가 트리거 안에 있는지 확인
    if SKOPE.Trigger.is_inside("safe_zone", ctx.entity_id) then
        -- 안전 존 내부
    end

    return nil
end

-- 종료
function TestSpellTrigger:on_destroy()
    SKOPE.Debug.log("=== Spell/Trigger Test Ended ===")

    -- 트리거 정리
    SKOPE.Trigger.remove("safe_zone")
    SKOPE.Trigger.remove("damage_zone")
end

return TestSpellTrigger
