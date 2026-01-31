-- SKOPE Engine Lua API Type Definitions
-- 이 파일은 자동완성을 위한 타입 정의입니다. 런타임에 로드되지 않습니다.

---@meta

---@class Vec3
---@field x number
---@field y number
---@field z number

---@class Vec3Module
---@field new fun(x: number, y: number, z: number): Vec3 새 벡터 생성
---@field zero fun(): Vec3 영벡터 (0, 0, 0)
---@field one fun(): Vec3 (1, 1, 1)
---@field up fun(): Vec3 위쪽 방향 (0, 1, 0)
---@field forward fun(): Vec3 앞쪽 방향 (0, 0, -1)
---@field add fun(a: Vec3, b: Vec3): Vec3 벡터 덧셈
---@field sub fun(a: Vec3, b: Vec3): Vec3 벡터 뺄셈
---@field mul fun(v: Vec3, scalar: number): Vec3 스칼라 곱
---@field length fun(v: Vec3): number 벡터 길이
---@field normalize fun(v: Vec3): Vec3 정규화
---@field dot fun(a: Vec3, b: Vec3): number 내적
---@field cross fun(a: Vec3, b: Vec3): Vec3 외적
---@field lerp fun(a: Vec3, b: Vec3, t: number): Vec3 선형 보간
---@field distance fun(a: Vec3, b: Vec3): number 두 점 사이 거리

---@class MathModule
---@field PI number 원주율 (3.14159...)
---@field TAU number 2π (6.28318...)
---@field E number 자연상수 (2.71828...)
---@field clamp fun(value: number, min: number, max: number): number 값 범위 제한
---@field lerp fun(a: number, b: number, t: number): number 선형 보간
---@field smoothstep fun(edge0: number, edge1: number, x: number): number 부드러운 보간
---@field deg_to_rad fun(degrees: number): number 도 → 라디안
---@field rad_to_deg fun(radians: number): number 라디안 → 도
---@field sign fun(value: number): number 부호 반환 (-1, 0, 1)

---@class InputState
---@field mouse_x number 마우스 X 좌표
---@field mouse_y number 마우스 Y 좌표
---@field mouse_delta_x number 마우스 X 이동량
---@field mouse_delta_y number 마우스 Y 이동량

---@class MousePosition
---@field x number
---@field y number

---@class InputModule
---@field state InputState 입력 상태
---@field keys table<string, boolean> 키 상태
---@field is_key_pressed fun(key: string): boolean 키가 눌려있는지 확인
---@field get_mouse_position fun(): MousePosition 마우스 위치
---@field get_mouse_delta fun(): MousePosition 마우스 이동량
---@field get_axis fun(axis: "horizontal"|"vertical"): number 축 입력값 (-1 ~ 1)

---@class DebugModule
---@field log fun(message: string) 로그 출력
---@field warn fun(message: string) 경고 출력
---@field error fun(message: string) 에러 출력
---@field draw_line fun(from: Vec3, to: Vec3, color: Vec3) 디버그 선 그리기
---@field draw_sphere fun(center: Vec3, radius: number, color: Vec3) 디버그 구 그리기

---@class TimeModule
---@field delta number 프레임 델타 타임 (초)
---@field elapsed number 총 경과 시간 (초)
---@field frame_count number 총 프레임 수
---@field fps number 현재 FPS

---@class SKOPE
---@field Vec3 Vec3Module 벡터 연산
---@field Math MathModule 수학 함수
---@field Input InputModule 입력 처리
---@field Debug DebugModule 디버그 도구
---@field Time TimeModule 시간 정보
SKOPE = {}

---@class ScriptContext
---@field entity_id number 현재 엔티티 ID
---@field delta_time number 델타 타임
---@field position Vec3 현재 위치

-- 스크립트 기본 구조
---@class Script
---@field on_start fun(self: Script, ctx: ScriptContext) 초기화 시 호출
---@field on_update fun(self: Script, ctx: ScriptContext) 매 프레임 호출
---@field on_destroy fun(self: Script) 파괴 시 호출
