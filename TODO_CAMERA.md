# 카메라 스무딩 작업 (진행 중)

## 목표
- 카메라 움직임을 부드럽게 (뚝뚝 끊기는 → 쭈욱)
- Unreal 스타일 에디터 카메라

## 구현된 것
- [x] 새 카메라 구조 (Input → Target → Current)
- [x] 지수 감쇠 스무딩 (`1.0 - exp(-smoothing * dt)`)
- [x] yaw/pitch 기반 방향 계산 (Z-up 좌표계)
- [x] 스무딩 파라미터 (CameraSettings)

## 남은 문제
- [ ] **카메라가 움직이지 않음** - 입력 처리 확인 필요
  - `on_mouse_move`에서 delta가 제대로 전달되는지
  - `handle_input`이 호출되는지
  - main.rs에서 마우스 이벤트가 scene_viewer로 전달되는지

## 디버깅 체크리스트
1. main.rs에서 마우스 이벤트 로그 추가
2. camera.rs에서 handle_input 호출 시 로그
3. mode 변경 확인 (Idle → Looking 등)
4. delta 값 확인

## 파일 변경 목록
- `src/editor/scene_viewer/camera.rs` - 완전히 재작성
- `src/editor/scene_viewer/mod.rs` - 새 API 적용
- `src/main.rs` - on_key에서 dt 제거
- `src/app/state.rs` - position(), fov, yaw(), pitch() API 변경
- `src/editor/gizmo/*.rs` - camera.position, camera.settings.fov로 변경
- `src/editor/scene_viewer/grid.rs` - camera.position으로 변경

## 새 카메라 조작법 (설계)
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
