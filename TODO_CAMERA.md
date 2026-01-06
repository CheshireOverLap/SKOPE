# 카메라 스무딩 작업 (완료)

## 목표
- 카메라 움직임을 부드럽게 (뚝뚝 끊기는 → 쭈욱)
- Unreal 스타일 에디터 카메라

## 구현된 것
- [x] 새 카메라 구조 (Input → Target → Current)
- [x] 지수 감쇠 스무딩 (`1.0 - exp(-smoothing * dt)`)
- [x] yaw/pitch 기반 방향 계산 (Z-up 좌표계)
- [x] 스무딩 파라미터 (CameraSettings)
- [x] 좌표 불일치 수정 (물리적↔논리적 좌표)
- [x] scene_viewer.update(dt) 호출 추가

## 해결된 문제
- [x] 카메라가 움직이지 않음
  - 원인 1: 좌표 불일치 (마우스 버튼: 논리적 좌표, 마우스 이동: 물리적 좌표)
  - 원인 2: scene_viewer.update(dt) 미호출 → 스무딩이 적용되지 않음

## 파일 변경 목록
- `src/editor/scene_viewer/camera.rs` - 완전히 재작성
- `src/editor/scene_viewer/mod.rs` - 새 API 적용, last_mouse_pos 초기화 추가
- `src/main.rs` - 좌표 변환 수정, scene_viewer.update(dt) 호출 추가

## 새 카메라 조작법
| 입력 | 동작 |
|------|------|
| 우클릭 + 마우스 | Look around |
| 우클릭 + WASD | Fly-through |
| 중클릭 드래그 | Pan |
| Alt + 좌클릭 | Orbit |
| 스크롤 | Dolly/Zoom |
| F | Focus on selection |

## 스무딩 파라미터
```rust
position_smoothing: 12.0   // 위치
rotation_smoothing: 20.0   // 회전
velocity_smoothing: 8.0    // 가감속
```
