-- Debug Drawing Test Script
-- 디버그 드로잉 API 테스트용 스크립트

local DebugTest = {}

function DebugTest:on_start(ctx)
    SKOPE.Debug.log("Debug Draw Test Script started!")
    self.time = 0
end

function DebugTest:on_update(ctx)
    self.time = self.time + ctx.delta_time

    -- 현재 엔티티 위치에 축 기즈모 그리기
    SKOPE.Debug.draw_axis(ctx.position, 1.0)

    -- 원점에서 현재 위치까지 라인 그리기
    SKOPE.Debug.draw_line(
        SKOPE.Vec3.zero(),
        ctx.position,
        { r = 1.0, g = 1.0, b = 0.0, a = 1.0 }  -- 노란색
    )

    -- 회전하는 구 그리기
    local orbit_radius = 2.0
    local orbit_x = math.cos(self.time) * orbit_radius
    local orbit_z = math.sin(self.time) * orbit_radius
    SKOPE.Debug.draw_sphere(
        SKOPE.Vec3.new(orbit_x, 1.0, orbit_z),
        0.5,
        { r = 0.0, g = 1.0, b = 1.0, a = 0.8 }  -- 청록색
    )

    -- 바운딩 박스 그리기
    SKOPE.Debug.draw_box(
        SKOPE.Vec3.new(-3, 0, -3),
        SKOPE.Vec3.new(3, 2, 3),
        { r = 0.5, g = 0.5, b = 0.5, a = 0.5 }  -- 회색
    )

    -- 점 그리기
    for i = 1, 8 do
        local angle = (i / 8) * math.pi * 2
        local px = math.cos(angle + self.time * 2) * 1.5
        local pz = math.sin(angle + self.time * 2) * 1.5
        SKOPE.Debug.draw_point(
            SKOPE.Vec3.new(px, 0.5, pz),
            { r = 1.0, g = 0.5, b = 0.0, a = 1.0 },  -- 주황색
            0.15
        )
    end

    return nil  -- Transform 업데이트 없음
end

return DebugTest
