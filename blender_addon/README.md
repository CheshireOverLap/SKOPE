# SKOPE UI Editor - Blender Addon

SKOPE 엔진의 UI 레이아웃을 Blender에서 시각적으로 편집할 수 있는 애드온입니다.

## 기능

- **위젯 생성**: Container, Text, Image, Button, 9-Slice 등 다양한 UI 위젯 생성
- **시각적 편집**: Blender의 3D 뷰에서 드래그하여 위치/크기 조정
- **속성 편집**: 사이드바 패널에서 위젯 속성 (색상, 텍스트, 이벤트 등) 편집
- **RON 내보내기**: SKOPE 엔진에서 바로 사용 가능한 RON 형식으로 내보내기
- **RON 가져오기**: 기존 RON 파일 불러오기 (기본 지원)

## 설치

### 자동 설치 (Linux)

```bash
cd blender_addon
./install.sh
```

### 수동 설치

1. `skope_ui` 폴더를 Blender addon 경로로 복사:
   - Linux: `~/.config/blender/4.x/scripts/addons/`
   - Windows: `%APPDATA%\Blender Foundation\Blender\4.x\scripts\addons\`
   - macOS: `~/Library/Application Support/Blender/4.x/scripts/addons/`

2. Blender에서 활성화:
   - Edit > Preferences > Add-ons
   - "SKOPE"로 검색
   - "SKOPE UI Editor" 체크박스 활성화

## 사용법

### 기본 워크플로우

1. **카메라 설정**: "Setup 2D Camera" 클릭하여 2D UI 편집용 직교 카메라 생성
2. **위젯 생성**: "Create Widget" 섹션에서 원하는 위젯 타입 클릭
3. **편집**:
   - G키로 이동
   - S키로 크기 조정
   - 사이드바에서 속성 편집
4. **내보내기**: "Export RON" 클릭하여 파일 저장

### 패널 위치

3D 뷰포트에서 N 키를 눌러 사이드바를 열고 "SKOPE UI" 탭 선택

### 위젯 계층 구조

- Blender의 부모-자식 관계(Ctrl+P)를 사용하여 위젯 계층 구조 생성
- 자식 위젯은 RON 내보내기 시 부모의 children 배열에 포함됨

### 좌표 시스템

- Blender X축 = UI X축 (오른쪽 양수)
- Blender Y축 = UI Y축 반전 (위쪽 양수가 Blender에서 음수)
- Blender Z축 = 렌더링 순서 (높은 Z가 위에 렌더링)
- 크기는 Blender Scale 사용 (1 unit = 1 pixel)

## 예제

### 간단한 버튼 생성

1. "Button" 클릭하여 버튼 생성
2. 위치 조정: G키 또는 Location 속성
3. 크기 조정: S키 또는 Scale 속성
4. 속성 패널에서:
   - Widget ID: "my_button"
   - Text Content: "Click Me!"
   - On Click: "handle_click"
5. "Export RON"으로 내보내기

## 제한사항

- RON 가져오기는 기본적인 구조만 지원
- 복잡한 애니메이션은 수동으로 RON 파일에 추가 필요
- 상태별 스타일(hover, pressed)은 기본값 사용
