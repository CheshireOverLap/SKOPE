# SKOPE Blender Addon

블렌더 씬을 SKOPE Engine으로 익스포트하는 애드온입니다.

## 설치 방법

### Option 1: ZIP 설치 (권장)

1. `blender_addon` 폴더를 ZIP으로 압축:
   ```bash
   cd /home/user/문서/SKOPE
   zip -r skope_addon.zip blender_addon/
   ```

2. Blender 5.0.1 실행

3. Edit → Preferences → Add-ons

4. Install... 버튼 클릭

5. `skope_addon.zip` 선택

6. "SKOPE Exporter" 애드온 체크박스 활성화

### Option 2: 직접 복사

```bash
cp -r blender_addon ~/.config/blender/5.0/scripts/addons/skope_exporter
```

그 후 Blender에서 Preferences → Add-ons → "SKOPE Exporter" 활성화

## 사용 방법

### 1. 컴포넌트 추가

1. 오브젝트 선택 (Outliner 또는 3D Viewport)

2. Properties 패널 → Object Properties 탭

3. **SKOPE Component** 패널 찾기

4. Component Type 선택:
   - **Player Spawn**: 플레이어 시작 위치
   - **Enemy Spawner**: 적 생성 위치
   - **Static Prop**: 배경 오브젝트
   - **Collider**: 충돌 영역
   - **Item Pickup**: 아이템 위치
   - **Trigger Zone**: 이벤트 트리거
   - **Light**: 게임 조명

5. 컴포넌트별 속성 설정

### 2. 씬 익스포트

1. SKOPE Component 패널 하단의 **Export SKOPE Scene** 버튼 클릭

2. 파일 저장 위치 선택 (예: `scenes/level1.skope`)

3. Save 클릭

4. 생성된 `.skope` 파일을 SKOPE Engine에서 로드

## 컴포넌트 타입별 속성

### Player Spawn
- 위치만 사용 (오브젝트 Transform)

### Enemy Spawner
- **Enemy Type**: 적 종류 (예: "goblin_basic")
- **Count**: 생성 개수
- **Respawn**: 리스폰 여부

### Static Prop
- **Has Collision**: 충돌 여부

### Collider
- **Shape**: Box / Sphere / Mesh
- **Is Trigger**: 트리거 모드 (물리 충돌 없음)

### Item Pickup
- **Item ID**: 아이템 고유 ID
- **Item Type**: Weapon / Grimoire / Consumable

### Trigger Zone
- **Event**: 트리거할 이벤트 이름

### Light
- 블렌더 라이트 속성 사용 (Type, Energy, Color)

## 예시 워크플로우

```
1. Blender에서 새 씬 생성
2. Cube 생성 → StaticProp 컴포넌트 추가 → Has Collision 체크
3. Empty 오브젝트 생성 → PlayerSpawn 컴포넌트 추가
4. Empty 오브젝트 생성 → EnemySpawner 컴포넌트 추가 → 설정
5. Export SKOPE Scene → level1.skope 저장
6. SKOPE Engine에서 로드하여 테스트
```

## 팁

- **Empty 오브젝트 사용**: Player Spawn, Enemy Spawner, Trigger Zone은 Empty 오브젝트에 추가
- **메쉬 오브젝트**: Static Prop은 실제 Mesh 오브젝트에 추가
- **레이어 구조**: Collection으로 엔티티 그룹화 가능
- **명명 규칙**: 오브젝트 이름이 엔티티 이름이 됨 (의미있게 작성)

## 문제 해결

**애드온이 안 보여요**
- Preferences → Add-ons → "Import-Export" 카테고리 확인
- 검색창에 "SKOPE" 입력

**패널이 안 보여요**
- 오브젝트를 선택했는지 확인
- Properties 패널 → Object Properties 탭 확인

**Export 버튼을 눌렀는데 아무 일도 안 일어나요**
- Blender Console 확인 (Window → Toggle System Console)
- Python 에러 메시지 확인

## 개발 정보

- **버전**: 0.1.0
- **Blender 호환**: 5.0.1+
- **라이선스**: MIT OR Apache-2.0
