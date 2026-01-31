-- SKOPE Engine - UI API Test Script
-- SKOPE.UI API 기능 테스트

local UiTest = {}

-- 초기화
function UiTest:on_start(ctx)
    print("=== UI API Test Started ===")

    -- UI 테이블 확인
    if SKOPE.UI then
        print("SKOPE.UI is available!")

        -- 등록된 위젯 목록 확인
        if SKOPE.UI._widgets then
            print("Widget registry initialized")
            local count = 0
            for id, _ in pairs(SKOPE.UI._widgets) do
                count = count + 1
                if count <= 5 then
                    print("  - Widget: " .. id)
                end
            end
            print("Total widgets: " .. count)
        end

        -- UI 상태 확인
        if SKOPE.UI._state then
            local state = SKOPE.UI._state
            print("UI State:")
            print("  - mouse_over_ui: " .. tostring(state.mouse_over_ui))
            print("  - hovered_widget: " .. tostring(state.hovered_widget or "none"))
            print("  - focused_widget: " .. tostring(state.focused_widget or "none"))
        end
    else
        print("ERROR: SKOPE.UI is not available!")
    end
end

-- 매 프레임 호출
function UiTest:on_update(ctx)
    local dt = ctx.delta_time

    -- 테스트: 1초마다 UI 상태 출력
    self.timer = (self.timer or 0) + dt

    if self.timer >= 2.0 then
        self.timer = 0

        -- 마우스가 UI 위에 있는지 확인
        local over_ui = SKOPE.UI.is_mouse_over_ui()
        if over_ui then
            local hovered = SKOPE.UI.get_hovered_widget()
            print("Mouse over UI: " .. (hovered or "unknown"))
        end

        -- health_bar 위젯 테스트 (HUD에 있다면)
        if SKOPE.UI.exists("health_bar") then
            local val, max = SKOPE.UI.get_progress("health_bar")
            print("Health bar: " .. tostring(val) .. "/" .. tostring(max))
        end

        -- health_text 위젯 테스트
        if SKOPE.UI.exists("health_text") then
            local text = SKOPE.UI.get_text("health_text")
            print("Health text: " .. tostring(text))
        end
    end

    -- T키로 텍스트 변경 테스트
    if SKOPE.Input.is_key_just_pressed("T") then
        print("T pressed - Testing set_text")
        if SKOPE.UI.exists("health_text") then
            local current = SKOPE.UI.get_text("health_text")
            SKOPE.UI.set_text("health_text", "HP: " .. math.random(0, 100))
            print("Changed health_text from: " .. tostring(current))
        end
    end

    -- P키로 프로그레스 바 변경 테스트
    if SKOPE.Input.is_key_just_pressed("P") then
        print("P pressed - Testing set_progress")
        if SKOPE.UI.exists("health_bar") then
            local new_val = math.random(0, 100)
            SKOPE.UI.set_progress("health_bar", new_val, 100)
            print("Set health_bar to: " .. new_val)
        end
    end

    -- H키로 위젯 숨기기/보이기 토글
    if SKOPE.Input.is_key_just_pressed("H") then
        print("H pressed - Testing set_visible")
        if SKOPE.UI.exists("hud_root") then
            local visible = SKOPE.UI.get_visible("hud_root")
            SKOPE.UI.set_visible("hud_root", not visible)
            print("Toggled hud_root visibility: " .. tostring(not visible))
        end
    end

    return {}
end

-- 파괴 시 호출
function UiTest:on_destroy()
    print("=== UI API Test Ended ===")
end

return UiTest
