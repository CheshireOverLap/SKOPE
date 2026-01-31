# SKOPE UI TODO — UE Slate 완전 대응 로드맵

> **[필독] 구현 시 반드시 아래 레퍼런스 소스를 참고할 것:**
> ```
> C:\Users\Cheshire\Documents\GitHub\SKOPE\reference\UE_Slate
> ```
> 이 폴더에 Unreal Engine Slate/SlateCore/SlateRHIRenderer 원본 소스(827 파일)가 있음.
> 각 항목의 UE 참조 경로는 이 폴더 기준이며, 구현 전에 해당 헤더(.h)와 소스(.cpp)를 반드시 읽고 설계를 파악한 뒤 작업할 것.

> 기준: Unreal Engine 5 Slate (reference/UE_Slate/ — 827 파일)
> 대상: crates/skope_ui/ (97 파일)
> 작성일: 2026-01-29 (최종 갱신: 2026-01-31)
> 현재 완성도: ~68% (Phase 1~3 완료: FastUpdate + 엘리먼트캐싱 + MultiBox + 텍스트프레임워크 + 멀티윈도우 + DnD + 속성 + 렌더트랜스폼 + 클리핑 + 모달 + 폰트확장)

---

## 범례

- `[ ]` 미착수
- `[~]` 작업 중 / 부분 구현
- `[x]` 완료

우선순위:
- **P0 — Critical**: 성능/기능의 근본적 병목. 프로덕션 에디터에 필수.
- **P1 — Major**: 에디터 완성도에 큰 영향. 핵심 워크플로 차단.
- **P2 — Moderate**: 완성도/편의성. 없어도 동작하지만 품질 차이.
- **P3 — Minor**: 부가 기능. 특수 상황에서만 필요.

---

## P0 — Critical

### 1. FastUpdate / 무효화 시스템

> UE 참조: `SlateCore/Public/FastUpdate/`, `SlateCore/Private/FastUpdate/` (16 파일)
> ~~현재: 매 프레임 전체 위젯 트리 재레이아웃 + 재페인트 (O(n))~~
> **구현 완료 (Phase 1-3)**: 위젯별 dirty flag + prepass + upward propagation + DrawElementList 캐싱

- [x] **Widget ID 시스템** — `next_widget_id()` (AtomicU64), 모든 위젯에 고유 `id: u64` 부여 (`traits.rs`)
- [x] **Dirty Flag 기반** — 모든 43개 Widget 구현체에 `dirty: InvalidateWidgetReason` 필드 추가, `dirty_flags()`, `invalidate()`, `clear_dirty()` 구현
- [x] **CHILD_ORDER Flag** — 자식 구조 변경 감지용 `InvalidateWidgetReason::CHILD_ORDER` 추가 (`attribute.rs`)
- [x] **Prepass 단계** — 메인 윈도우 렌더 루프에 `prepass_widget()` 호출 추가: 속성 업데이트 + volatile 감지 + dirty 전파 (`slate_app.rs`)
- [x] **Upward Dirty Propagation** — `prepass_widget()`이 자식 dirty를 부모로 전파 (LAYOUT, PAINT)
- [x] **Volatile 위젯** — `is_volatile() == true`인 위젯은 prepass에서 매 프레임 PAINT dirty 자동 설정
- [x] **Clear Dirty** — paint 완료 후 `clear_dirty_recursive()` 호출하여 전체 트리 dirty 클리어
- [x] **DrawElementList 캐싱** — `RSlateRenderer`에 `cached_draw_elements` + `cache_valid` 추가, root clean 시 on_paint 생략
- [x] **리사이즈 무효화** — 화면 리사이즈 시 캐시 자동 무효화

**미구현 (향후 최적화):**
- [ ] `WidgetProxy` — 위젯별 경량 프록시 (가시성 캐스케이딩, 업데이트 플래그)
- [ ] `SlateInvalidationWidgetList` — 캐시 친화적 flat 위젯 리스트
- [ ] `SlateInvalidationWidgetHeap` — 우선순위 힙 기반 무효화 처리 순서
- [ ] `SlateInvalidationWidgetSortOrder` — 결정적 처리 순서
- [ ] `CachedElementData` — 위젯별 버텍스/인덱스 캐싱 (서브트리 레벨)
- [ ] Desired Size 캐싱 — `compute_desired_size()` 결과 캐싱 (LAYOUT 시만 재계산)
- [ ] `SlateInvalidationContext` — CullingRect, ViewOffset, LayoutScale 등 페인트 컨텍스트

### 2. 엘리먼트 캐싱 & 배칭

> UE 참조: `SlateCore/Public/Rendering/ElementBatcher.h`, `DrawElements.h`
> ~~현재: 텍스처 ID로만 배칭, 캐싱 없음, 단일 버퍼~~
> **구현 완료 (Phase 1-3)**: 테셀레이션 캐싱 + 버퍼 재사용 + 정렬 캐싱 + 배치 병합

- [x] **테셀레이션 캐싱** — `tessellate_elements()` + `submit_render()` 분리, `tessellation_valid` 플래그로 idle 프레임 재테셀레이션 생략 (`renderer.rs`)
- [x] **버퍼 재사용** — `cached_vertices/indices/batches` 필드 (`.clear()` + 재사용, 매 프레임 Vec 할당 제거) (`renderer.rs`)
- [x] **텍스트 데이터 스냅샷** — `cached_text_vertices/indices`, `render_from_cache()` + `upload_and_draw()` 헬퍼 (`text_renderer.rs`)
- [x] **정렬 캐싱** — DrawElementList에 `sorted_indices` + `sort_valid`, `ensure_sorted()` + `sorted_iter()` — 변경 시에만 재정렬 (`traits.rs`)
- [x] **배치 병합** — 인접 동일 텍스처+클립 DrawBatch in-place 병합 (zero allocation) (`renderer.rs`)
- [x] **render() 3단계 분리** — paint 체크 → tessellate 체크 → submit (캐시 유효 시 GPU 업로드+드로우만 실행)

**미구현 (향후 최적화):**
- [ ] `SlateElementBatcher` 완전 대응 — Layer + ShaderType + DrawEffects 다중 키 배칭
- [ ] `SlateDrawBuffer` — 더블 버퍼링 (스레드 안전 렌더 제출)
- [ ] Deferred Painting — 후순위 위젯 지연 페인팅
- [ ] Layer ID 시스템 — 드로우 순서 제어용 레이어
- [ ] 위젯별 서브트리 캐싱 — `CachedElementData` 위젯 단위 정점/인덱스 캐시

### 3. MultiBox — 메뉴/툴바 빌더 시스템

> UE 참조: `Slate/Public/Framework/MultiBox/` (9 파일), `Slate/Private/Framework/MultiBox/` (38 파일)
> ~~현재: 동등한 시스템 없음~~
> **부분 구현 (Phase 3)**: MultiBlockEntry + MultiBoxBuilder + SMultiBoxToolbar + MenuItem 편의 생성자

- [x] `MultiBlockEntry` — 멀티블록 엔트리 (Button/Toggle/Check/Radio/Separator/SubMenu/Widget/Heading) (`framework/multi_box.rs`)
- [x] `MultiBoxBuilder` — 커맨드 기반 빌더 (add_command, add_separator, build_menu_items) (`framework/multi_box.rs`)
- [x] `SMultiBoxToolbar` — 수평 툴바 위젯 (호버/클릭 + 커맨드 콜백 + 구분선/헤더) (`widget/s_multi_box_toolbar.rs`)
- [x] `MenuItem::from_command()` — UICommandList 기반 메뉴 아이템 생성 (`widget/s_menu.rs`)
- [ ] `MultiBoxExtender` — 플러그인 확장 포인트 (named hook으로 메뉴/툴바 주입)
- [ ] `MultiBoxCustomization` — 사용자 정의 툴바 레이아웃
- [ ] `SToolBarComboButtonBlock` — 툴바 콤보 버튼
- [ ] `SToolBarStackButtonBlock` — 툴바 스택 버튼
- [ ] `SClippingHorizontalBox` — 툴바 오버플로 자동 처리
- [ ] `SPrioritizedWrapBox` — 공간 부족 시 우선순위 기반 레이아웃

### 4. 텍스트 프레임워크

> UE 참조: `Slate/Public/Framework/Text/`, `Slate/Private/Framework/Text/` (49 파일)
> ~~현재: 기본 텍스트 렌더링만 (STextBlock, SEditableTextBox). Run 아키텍처 없음.~~
> **부분 구현 (Phase 3)**: ITextRun + TextLayout 엔진 + TextRange + FSlateWidgetRun + SRichTextBlock/STextBlock 연동

- [x] `ITextRun` trait / `FSlateTextRun` — 스타일별 텍스트 Run + 기본 구현 (`render/text_run.rs`)
- [x] `FSlateWidgetRun` — 인라인 위젯 Run (U+FFFC) (`render/text_run.rs`)
- [x] `TextRunStyle` — 폰트 셀렉터 + 크기 + 색상 + 밑줄/취소선/자간 (`render/text_run.rs`)
- [x] `TextRange` — 바이트 범위 관리 (`render/text_run.rs`)
- [x] `TextLayout` 엔진 — 멀티라인 글리프 배치 (NoWrap/WordWrap/CharWrap) (`render/text_layout.rs`)
- [x] `ShapedGlyphEntry` / `ShapedTextLine` — 배치 결과 구조체 (`render/text_layout.rs`)
- [x] `TextLayoutParams` — 레이아웃 파라미터 (max_width, line_break_mode, max_lines) (`render/text_layout.rs`)
- [x] SRichTextBlock TextLayout 연동 — 멀티라인 + wrap 모드 + 캐시 (`widget/s_rich_text_block.rs`)
- [x] STextBlock TextLayout 연동 — WordWrap/CharWrap 시 멀티라인 렌더링 (`widget/s_text_block.rs`)
- [ ] `SlateHyperlinkRun` — 클릭 가능한 하이퍼링크 Run
- [ ] `SlateImageRun` — 텍스트 내 인라인 이미지
- [ ] `SlatePasswordRun` — 비밀번호 마스킹 Run
- [ ] `IRichTextMarkupParser` — XML/마크업 텍스트 파싱
- [ ] `RichTextLayoutMarshaller` — 리치 텍스트 마샬링
- [ ] `ITextDecorator` — 텍스트 데코레이터 (밑줄, 하이라이트)
- [ ] `SyntaxTokenizer` — 구문 강조용 토크나이저
- [ ] `ShapedTextCache` — HarfBuzz/ICU 복잡 스크립트 셰이핑
- [ ] BiDi / RTL 지원 — 양방향 텍스트
- [ ] `TextHitPoint` — 텍스트 내 커서 위치 hit-test

### 5. 멀티 윈도우 지원

> UE 참조: `FSlateApplication::AddWindow()`, `MakeWindow()`, `DestroyWindowImmediately()`
> ~~현재: 단일 winit::Window만 사용~~
> **구현 완료 (Phase 1-2)**: 다수 OS 윈도우 + Tear-off + 윈도우간 D&D + Per-window DPI + GetWorkArea + 멀티모니터 클램핑 + 팝업 인프라

- [x] 다수 OS 윈도우 생성/관리 (`AddWindow`, `DestroyWindow`) — `FloatingWindowInfo`, `WindowState` (`slate_app.rs`)
- [x] 도킹 탭 → 새 OS 윈도우 Tear-off — `DockingDragOperation` → `create_floating_window()` (`slate_app.rs`)
- [x] 윈도우 간 탭/콘텐츠 드래그앤드롭 — decorator window + morph state (`slate_app.rs`)
- [x] Per-window DPI 스케일링 — `WindowState::scale_factor` + `ScaleFactorChanged` 이벤트 처리 (`slate_app.rs`)
- [x] `GetWorkArea()` — `get_work_area_at()`, `get_primary_work_area()` + `MonitorWorkArea` (`slate_app.rs`)
- [x] 멀티 모니터 팝업 위치 계산 — `clamp_window_to_work_area()` + `create_floating_window` 보정 (`slate_app.rs`)
- [~] 팝업/메뉴 별도 윈도우 (부모 밖으로 확장) — `PopupWindowInfo`, `PopupWindowRequest`, `create_popup_window()` 인프라 준비 (`slate_app.rs`)

**미구현 (향후):**
- [ ] PopupLayer → PopupWindow 자동 전환 (메뉴가 부모 윈도우 밖으로 확장 시)
- [ ] 팝업 윈도우 렌더링/이벤트 라우팅 완성

---

## P1 — Major

### 6. 드래그앤드롭 프레임워크 ✓ 완료

> UE 참조: `SlateCore/Public/Input/DragAndDrop.h`, `FSlateApplication` 통합
> ~~현재: 도킹 탭 드래그만 존재. 범용 시스템 없음.~~
> **구현 완료**: DragDropManager 상태 머신 + Widget trait 드래그 콜백 + SlateApp 이벤트 통합

- [x] `DragDropOperation` — 드래그 페이로드 + 데코레이터 + 타입 태그
- [x] `WidgetDragDropEvent` — 드래그 이벤트 데이터 (docking DragDropEvent와 독립)
- [x] `DragDropManager` — 3상태 머신 (Idle → Detecting → Dragging)
- [x] Widget trait 확장: `on_drag_detected`, `on_drag_enter`, `on_drag_leave`, `on_drag_over`, `on_drop`
- [x] Widget trait 확장: `on_mouse_capture_lost`
- [x] Reply 확장: `detect_drag_with(button, widget_id)`, `begin_drag_drop(op)`
- [x] SlateApp 통합 — MouseDown/Move/Up 이벤트 라우팅
- [x] 드래그 임계값 설정 (DragDropManager::drag_threshold, 기본 5px)

### 7. 반응형 속성 시스템 (TSlateAttribute 완전 대응)

> UE 참조: `SlateCore/Public/Types/SlateAttribute.h`
> ~~현재: Attribute<T>에 바인딩+dirty 있으나 무효화 시스템 미연결~~
> **구현 완료 (Phase 1)**: SlateAttribute<T> dirty_from_set + update_attributes! 매크로 + 6개 위젯 변환

- [x] 기본 `Attribute<T>` (Static/Bound + dirty flag) — 구현됨
- [x] `SlateAttribute<T>` — `dirty_from_set` 플래그 추가, `set()` 시 값 비교 + 변경 감지
- [x] `update_attributes!` 매크로 — 다수 SlateAttribute 필드 일괄 업데이트 + InvalidateWidgetReason 집계
- [x] 속성 변경 → 자동 `InvalidateWidgetReason` 연결 — `update()` → prepass → `invalidate()`
- [x] Prepass 통합 — 프레임 시작 시 `update_attributes()` 호출 → dirty 전파
- [x] 위젯 변환 6개: STextBlock(3), SBorder(1), SImage(1), SProgressBar(1), SCheckBox(1), SSlider(1)
- [x] 빌더 하위 호환 + `_attr()` 바인딩 메서드 추가
- [x] `SlateBrush`에 `PartialEq` derive 추가

**미구현 (향후 최적화):**
- [ ] 업데이트 순서 보장 (속성 A → B dependency)
- [ ] `ManagedAttribute<T>` — 이동 가능 컨테이너용 변형
- [ ] 나머지 위젯 SlateAttribute 변환 (SComboBox, SSpinBox, SEditableTextBox 등)

### 8. 렌더 트랜스폼

> UE 참조: `SlateCore/Public/Layout/Geometry.h` — FSlateRenderTransform
> ~~현재: position + uniform scale만 지원~~
> **구현 완료**: SlateRenderTransform + SlateRotatedRect + Geometry/PaintGeometry RT 지원 + SFxWidget + apply_widget_render_effects 헬퍼

- [x] `SlateRenderTransform` — 2D affine (회전, 기울기, 비균일 스케일) (`core/render_transform.rs`)
- [x] 트랜스폼 피봇 — 위젯 중심/커스텀 기준 트랜스폼 (`Geometry::with_render_transform` pivot 파라미터)
- [x] `SlateRotatedRect` — 회전된 사각형 AABB + 포인트 히트테스트 (`core/render_transform.rs`)
- [x] `RenderOpacity` — 위젯별 투명도, 계층적 누적 (`Geometry::with_render_opacity`)
- [x] Accumulated Render Transform — 계층적 트랜스폼 누적 (`Geometry::accumulated_render_transform`)
- [x] Geometry에 `has_render_transform` 플래그 (최적화)
- [x] Widget trait 확장 — `render_opacity()`, `render_transform()`, `render_transform_pivot()`
- [x] 테셀레이션 헬퍼 RT 분기 — `emit_quad`, `emit_local_rect`, `emit_quad_gradient`, `emit_border`
- [x] `RENDER_TRANSFORM` dirty flag + 캐시 무효화 연동
- [x] `SFxWidget` — 렌더 트랜스폼 + 불투명도 효과 래퍼 위젯 (`widget/s_fx_widget.rs`)
- [x] `apply_widget_render_effects()` — 프레임워크 레벨 헬퍼 함수 (`widget/traits.rs`)

**미구현 (향후 최적화):**
- [ ] 스텐실 버퍼 기반 비축 정렬 클리핑 (P1#9 계층적 클리핑에서 처리)
- [ ] 히트테스트 역변환 캐싱 — `accumulated_render_transform.inverse()` 캐시
- [ ] 애니메이션 자동 바인딩 — `CurveSequence` 연동 트랜스폼 애니메이션

### 9. 계층적 클리핑 ✓ 완료

> UE 참조: `SlateCore/Public/Layout/Clipping.h`, `FSlateClippingManager`
> ~~현재: 단순 시저 렉트 (AABB) 스택~~
> **구현 완료**: SlateClippingManager + EWidgetClipping + 계층적 클립 합성 + Scissor 렌더링 + Widget trait 확장

- [x] `SlateClippingManager` — 클리핑 존 관리자, push/pop + 상태 합성 (`core/clipping.rs`)
- [x] `EWidgetClipping` — Inherit / ClipToBounds / ClipToBoundsWithoutIntersecting / ClipToBoundsAlways / OnDemand (`core/clipping.rs`)
- [x] `SlateClippingZone` — 4코너 + 축 정렬 감지 + AABB/Scissor 변환 (`core/clipping.rs`)
- [x] `SlateClippingState` — Scissor rect / Stencil quads 이중 모드 (`core/clipping.rs`)
- [x] `EClippingMethod` — Scissor / Stencil 구분 (`core/clipping.rs`)
- [x] DrawElementList 통합 — clip_state_indices + ClippingManager 위임 (`widget/traits.rs`)
- [x] Widget trait `widget_clipping()` — 위젯별 클리핑 모드 설정 (`widget/traits.rs`)
- [x] `paint_child_with_clipping()` 헬퍼 — 자동 클리핑 적용 (`widget/traits.rs`)
- [x] 렌더러 배치 키 변경 — clip_state_index + 캐시된 클리핑 상태 (`render/renderer.rs`)
- [x] `push_clip_rect()` 하위 호환 — SScrollBox 등 기존 호출자 지원 (`widget/traits.rs`)
- [~] 스텐실 버퍼 클리핑 — 비축 정렬은 보수적 AABB fallback (인프라 준비 완료)

**미구현 (향후):**
- [ ] 실제 wgpu stencil write/test 파이프라인 — Depth24PlusStencil8 텍스처 + 스텐실 파이프라인
- [ ] 히트테스트 시 클리핑 존 반영

### 10. 모달 윈도우 관리

> UE 참조: `FSlateApplication::GetActiveModalWindow()`
> ~~현재: PopupLayer에 기본 모달 플래그만~~
> **부분 구현**: FocusManager 모달 스코프 + ModalInputFilter + PopupLayer 모달 포커스 연동

- [x] 포커스 트래핑 — `FocusManager::push_modal_scope()` / `pop_modal_scope()` + navigate()에서 scope 내 위젯만 후보로 필터 (`framework/focus.rs`)
- [x] 모달 뒤 위젯 이벤트 차단 — `ModalInputFilter` InputPreProcessor, Escape 외 키 이벤트 차단 (`framework/modal_input_filter.rs`)
- [x] PopupLayer 모달 포커스 연동 — `modal_scope_id`, `active_modal_scope()`, `ModalDismissEvent`, `take_modal_events()` (`framework/popup.rs`)
- [x] SlateApp 모달 통합 — `SlateAppHandler::focus_manager()` + 모달 push/dismiss 시 FocusManager 연동 (`application/slate_app.rs`)
- [ ] 모달 윈도우 스택 (다중 모달)
- [ ] `FModalWindowStackStarted/Ended` 델리게이트
- [ ] 비-Slate 모달 연동 (`ExternalModalStart/Stop`)

### 11. 폰트 시스템 확장

> UE 참조: `SlateCore/Public/Fonts/` (13 파일)
> ~~현재: ab_glyph 래스터 + 기본 SDF 작업 중~~
> **부분 구현**: FontWeight/FontStyle/FontSelector + FontMetrics 캐시 + 폰트 변형 지원 + DrawElement::StyledText + SDF 인프라 토글

- [x] `FontWeight` / `FontStyle` / `FontSelector` — 폰트 가중치·스타일·셀렉터 타입 (`core/font_family.rs`)
- [x] `FontMetrics` + `FontMetricsCache` — 폰트 메트릭스 캐시 싱글톤, ascent/descent/line_gap/x_height/cap_height (`render/font_metrics.rs`)
- [x] 폰트 변형 지원 — `font_variant_chains`, `resolve_font_chain()`, `add_text_with_selector()`, `measure_*_with_selector()` (`render/text_renderer.rs`)
- [x] `DrawElement::StyledText` — FontSelector 기반 텍스트 렌더링 경로 + `add_styled_text()` (`widget/traits.rs`, `render/renderer.rs`)
- [~] SDF 렌더링 — `sdf_renderer.rs` 기존 + `sdf_enabled` 토글 인프라 준비 (파이프라인 미완)
- [ ] `FontOutlineSettings` — 아웃라인 크기, 마이터 코너, 별도 필 알파
- [ ] 드롭 섀도 — 폰트 드롭 섀도
- [ ] MSDF 렌더링 — 멀티 채널 SDF
- [ ] `CompositeFont` — 복합 폰트 (타입페이스 패밀리)
- [ ] Letter Spacing / Tracking — 자간 조절
- [ ] Skew Amount — 이탤릭 시뮬레이션
- [ ] `bForceMonospaced` / `MonospacedWidth` — 강제 모노스페이스
- [ ] Font Hinting 제어
- [ ] `FontMeasure` 전용 인터페이스 (현재 TextMeasurer 존재)
- [ ] Unicode Block Range 폴백 — 블록별 폰트 폴백

### 12. Active Timer 시스템

> UE 참조: `SWidget::RegisterActiveTimer()`, `EActiveTimerReturnType`
> ~~현재: Widget trait에 `can_tick()` 플래그만~~
> **구현 완료**: ActiveTimerHandle/ActiveTimers 코어 타입 + Widget trait 확장 + Prepass 통합 + PaintArgs 시간 수정 + 2개 위젯 변환

- [x] `ActiveTimerHandle` — 위젯별 주기적 업데이트 등록 (`core/active_timer.rs`)
- [x] `ActiveTimers` 컬렉션 — `register(period)`, `unregister(id)`, `execute_pending()` (zero-alloc retain_mut)
- [x] `ActiveTimerReturnType` — Stop / Continue
- [x] Widget trait 확장 — `has_active_timers()`, `tick_active_timers(current_time, delta_time)`
- [x] Prepass 통합 — `prepass_widget()`에서 타이머 실행 + dirty 자동 전파
- [x] 프레임 시간 인프라 — `SlateApp`에 `app_start_time`, `current_time`, `frame_delta_time` 추가
- [x] `PaintArgs` 시간 수정 — `current_time: 0.0` 버그 수정, 3개 호출처 실제 시간 전달
- [x] `RSlateRenderer::render()` 시그니처 확장 — `current_time: f64, delta_time: f32` 파라미터 추가
- [x] SProgressBar 변환 — 불확정(마키) 모드 자동 타이머 등록/해제, `AtomicU32` 캐시 너비
- [x] SExpandableArea 변환 — `set_expanded_animated()` 시 타이머 등록, 완료 시 자동 해제

**미구현 (향후 최적화):**
- [ ] 타이머 없으면 UI idle (CPU 절약) — `has_active_timers` 플래그 저장됨, sleep 로직 미연결
- [ ] 애니메이션 자동 등록 — `CurveSequence::Play()`가 타이머 자동 등록 (현재 수동)

---

## P2 — Moderate

### 13. 스타일링 시스템 강화

> UE 참조: `SlateCore/Public/Styling/` (22 파일)
> 현재: EditorTheme 단일 구조체 + StyleSet key-value

- [ ] 위젯별 타입 스타일 구조체 (`ButtonStyle`, `TextBlockStyle`, `ScrollBarStyle` 등)
- [x] `SlateColor` — 테마 색상 간접 참조 (이름 → 실제 색상 resolve)
- [ ] Content Root 디렉토리 — 스타일 셋 기준 에셋 경로 해석
- [ ] `SlateIconFinder` — 이름 기반 아이콘 검색
- [x] 스타일 편의 매크로 (Rust macro_rules)
- [ ] 다중 스타일 셋 공존 (에디터/게임/커스텀)
- [ ] Sound 통합 — 스타일 셋 내 `SlateSound`

### 14. 부모 추적 & 위젯 경로

> UE 참조: `SWidget::ParentWidgetPtr`, `FWidgetPath`
> 현재: 없음

- [x] `ParentWidgetPtr` — 부모 위젯 weak 참조 (또는 ID)
- [x] `WidgetPath` — 루트 → 위젯 전체 경로
- [x] `FindPathToWidget()` — 위젯 경로 검색
- [x] `ValidatePathToChild()` — 자식 경로 유효성 검증
- [x] `IsDescendantOf()` — 자손 관계 쿼리
- [ ] 포커스/입력 라우팅에 WidgetPath 활용

### 15. 도킹 시스템 보완

> UE 참조: `Slate/Public/Framework/Docking/` (6 파일 Public, 28 파일 Private)
> 현재: 기본 트리/탭/사이드바/컴패스 구현됨

- [x] `LayoutExtender` — 기존 레이아웃 수정 없이 확장 (플러그인 주입)
- [ ] `WorkspaceItem` — 탭 타입 계층적 분류/브라우징
- [ ] `STabDrawer` 풀 구현 — 자동 숨김 사이드바 (hover 시 슬라이드 아웃)
- [ ] `TabCommands` — 도킹 전용 키보드 단축키
- [x] `FOnActiveTabChanged` 델리게이트
- [ ] 탭 인스턴스 ID — `TabId` (TabType + InstanceId)
- [x] 탭 persistability 플래그 (`ShouldSaveLayout`)

### 16. 커맨드 시스템 보완

> UE 참조: `Slate/Public/Framework/Commands/` (6 파일)
> 현재: 기본 계층 컨텍스트/InputChord/UIAction 구현됨

- [x] 아이콘 연결 — `SlateIcon` per command
- [x] 사용자 정의 키바인딩 저장/로드 (JSON 영속화)
- [x] 키 충돌 감지 — `GetCommandInfoFromInputChord()`
- [x] 반복 모드 — `EUIActionRepeatMode` (RepeatEnabled/Disabled)
- [ ] 커맨드 리스트 스택 — 런타임 push/pop
- [x] Generic Commands 프리셋 — Cut / Copy / Paste / Undo / Redo / SelectAll / Delete
- [ ] `CollapsedButton` 액션 타입
- [ ] `FOnBindingContextChanged` 델리게이트

### 17. 알림 시스템 보완

> UE 참조: `Slate/Private/Framework/Notifications/`
> 현재: 기본 토스트 알림 구현됨

- [x] Progress Notification — 백그라운드 작업 진행률 표시/추적
- [x] `IProgressNotificationHandler` — 상태바 통합
- [x] `SNotificationItem` 위젯 — 버튼/하이퍼링크 포함 인터랙티브 알림
- [ ] 스레드 안전 큐 — `QueueNotification` (lock-free)
- [ ] Staged async notification — 비동기 작업 단계별 알림
- [x] 알림 만료 콜백

### 18. 디버깅 인프라

> UE 참조: `SlateCore/Public/Debugging/SlateDebugging.h`
> 현재: F9 위젯 리플렉터만 (타입명/바운드/자식수)

- [x] 입력 이벤트 트레이싱 — 25종 이벤트 타입별 추적 로깅
- [x] 성능 프로파일러 통합 — 드로우 콜/엘리먼트 수/페인트 시간 카운팅
- [x] 전역 위젯 리스트 — 모든 라이브 위젯 추적 (메모리 디버깅)
- [ ] 조건부 컴파일 — `#[cfg(feature = "slate_debugging")]`
- [x] 위젯 리플렉터 확장 — 풀 위젯 트리 뷰, 스냅샷, 속성 검사
- [ ] 아틀라스 디버그 시각화 — 텍스처 아틀라스 페이지 뷰어

### 19. 텍스처 관리 개선

> UE 참조: `SlateCore/Public/Textures/TextureAtlas.h`, `SlateIcon.h`
> 현재: Shelf-packing 아틀라스, RGBA only, 즉시 업로드

- [ ] 트리 기반 아틀라스 패킹 — 공간 효율 개선
- [ ] Multi-format 아틀라스 — Alpha / Color / MSDF 구분
- [ ] 스레드 안전 소유권 — Game/Render 스레드 분리
- [ ] Lazy/Deferred GPU 업로드 — dirty 플래그 기반
- [ ] 플러시/퇴거 시스템 — 아틀라스 메모리 관리
- [ ] `SlateIcon` 추상화 — (스타일셋 이름, 브러시 이름, small 아이콘 옵션)
- [ ] `SlateUpdatableTexture` — 런타임 업데이트 가능 텍스처
- [ ] Non-atlased 폴백 — 대형 텍스처는 별도 관리

### 20. 브러시 시스템 보완

> UE 참조: `SlateCore/Public/Styling/SlateBrush.h`, `SlateCore/Public/Brushes/`
> 현재: 5가지 DrawType + 코너/아웃라인 지원

- [ ] 타일링 — `BrushTiling::Horizontal / Vertical / Both`
- [ ] 미러링 — `BrushMirroring::Horizontal / Vertical / Both`
- [ ] UV 영역 선택 — 텍스처 서브리전
- [ ] `DynamicImageBrush` — 런타임 생성 텍스처 브러시
- [ ] Image Type 분류 — NoImage / FullColor / Linear / Vector
- [ ] `RoundingType` — FixedRadius / HalfHeightRadius
- [ ] `SlateResourceHandle` — GPU 리소스 바인딩 추상화

### 21. 렌더링 세부 보완

> UE 참조: `SlateCore/Public/Rendering/RenderingCommon.h`
> 현재: Box/Border/Text/RoundedBox/Gradient/Image/NineSlice/Brush

- [ ] `ET_Spline` — 베지어 스플라인 드로우
- [ ] `ET_ShapedText` — 복잡 스크립트 텍스트 별도 셰이더
- [ ] `ET_Viewport` — 3D 뷰포트 UI 임베딩
- [ ] `ET_Custom` — 커스텀 드로우 콜백
- [ ] `ET_CustomVerts` — 커스텀 버텍스 데이터 직접 제출
- [~] `ET_PostProcessPass` — post_process.rs / blur_shader.wgsl 작업 중
- [ ] Draw Effects 비트마스크 — NoBlending / PreMultipliedAlpha / NoGamma / DisabledEffect 등
- [ ] 픽셀 스냅 — `WidgetPixelSnapping` (위젯별 스냅 제어)
- [ ] Instanced Rendering — 인스턴스 버텍스 선언

---

## P3 — Minor / 특수 상황

### 22. 이벤트/입력 확장

> UE 참조: SWidget 이벤트 핸들러, FSlateApplication 입력 파이프라인
> 현재: 마우스/키보드/IME 기본 지원

- [ ] 터치 입력 — `on_touch_started`, `on_touch_moved`, `on_touch_ended`
- [ ] 터치 제스처 — `on_touch_gesture` (핀치/스와이프)
- [ ] 3D Touch — `on_touch_force_changed`
- [ ] 게임패드 아날로그 — `on_analog_value_changed`
- [ ] 모션 감지 — `on_motion_detected` (가속도계/자이로)
- [ ] 멀티 유저 입력 — 유저별 독립 포커스/캡처
- [ ] 입력 프로세서 스택 — 7단계 우선순위 체인 (현재 단일)
- [ ] 네비게이션 이벤트 — `on_navigation` + `FNavigationReply`
- [ ] 아날로그 커서 — 게임패드 기반 커서 제어
- [ ] `GestureDetector` — 제스처 인식기
- [ ] 버튼 활성화 방식 — `EButtonClickMethod` (DownAndUp/MouseDown/PreciseClick)
- [ ] 텍스트 커밋 타입 — `ETextCommit` (OnEnter/OnCleared 등)
- [ ] 선택 정보 타입 — `ESelectInfo` (OnKeyPress/OnMouseClick 등)

### 23. Widget Trait 누락 메서드

> UE 참조: SWidget.h (~1200 lines)

#### 이벤트 핸들러 추가
- [ ] `on_drag_detected` — 드래그 시작 (P1 #6에 포함)
- [ ] `on_drag_enter` / `on_drag_leave` / `on_drag_over` / `on_drop`
- [ ] `on_mouse_capture_lost` — 마우스 캡처 해제 알림
- [ ] `on_cursor_query` → `CursorReply` — 위젯별 커서
- [ ] `on_visualize_tooltip` — 커스텀 툴팁 렌더링
- [ ] `on_query_show_focus` — 포커스 시각화 쿼리
- [ ] `on_focus_changing` — 포커스 경로 변경 알림
- [ ] `on_finished_pointer_input` / `on_finished_key_input` — 프레임 종료 배치 처리

#### 상태/속성 메서드 추가
- [ ] `slate_prepass()` — 2패스 레이아웃의 프리패스
- [ ] `cache_desired_size()` / `get_desired_size()` — 크기 캐싱
- [ ] `get_parent_widget()` / `assign_parent_widget()` — 부모 추적
- [ ] `supports_keyboard_focus()` — trait 레벨 포커스 가능 여부
- [ ] `has_keyboard_focus()` / `has_mouse_capture()` — 현재 상태 쿼리
- [ ] `is_hovered()` / `is_directly_hovered()` — trait 레벨 호버 쿼리
- [ ] `flow_direction` — RTL/LTR 레이아웃 방향
- [ ] `tag` / `metadata` — 위젯 태깅 및 확장 메타데이터
- [ ] `get_relative_layout_scale()` — 자식별 스케일 팩터

### 24. 누락 위젯 — Layout (14개)

| # | 위젯 | 설명 | 난이도 |
|---|------|------|--------|
| 1 | `SBackgroundBlur` | 배경 블러 효과 | 중 |
| 2 | `SConstraintCanvas` | 앵커/오프셋 기반 캔버스 | 중 |
| 3 | `SFxWidget` | 렌더 트랜스폼 + 불투명도 래퍼 | ✅ 완료 (P1#8) |
| 4 | `SScaleBox` | 콘텐츠 스케일 조절 (Fit/Fill/Stretch) | ✅ 완료 (P3) |
| 5 | `SScissorRectBox` | 클리핑 래퍼 | ✅ 완료 (P3) |
| 6 | `SUniformGridPanel` | 균일 크기 그리드 | ✅ 완료 (P3) |
| 7 | `SUniformWrapPanel` | 균일 크기 랩 | 하 |
| 8 | `SResponsiveGridPanel` | 반응형 그리드 | 중 |
| 9 | `SRadialBox` | 방사형 레이아웃 | ✅ 완료 (P3) |
| 10 | `SSafeZone` | 화면 안전 영역 | ✅ 완료 (P3) |
| 11 | `SStackBox` | Z-스택 레이아웃 | ✅ 완료 (P3) |
| 12 | `SLinkedBox` | 연결된 크기 박스 | 하 |
| 13 | `SWindowTitleBarArea` | 커스텀 타이틀바 영역 | ✅ 완료 (P3) |
| 14 | `SPopup` | 팝업 위젯 | ✅ 완료 (P3) |

### 25. 누락 위젯 — Input (12개)

| # | 위젯 | 설명 | 난이도 |
|---|------|------|--------|
| 1 | `SComboButton` | 드롭다운 + 버튼 조합 | ✅ 완료 (P3) |
| 2 | `SEditableComboBox` | 편집 가능 콤보박스 | 중 |
| 3 | `SEditableLabel` | 인라인 편집 라벨 (더블클릭 → 편집) | ✅ 완료 (P3) |
| 4 | `SExpandableButton` | 확장 가능 버튼 | ✅ 완료 (P3) |
| 5 | `SHyperlink` | 클릭 가능 링크 | ✅ 완료 (P3) |
| 6 | `SInputKeySelector` | 키 입력 선택기 | 중 |
| 7 | `SNumericDropDown` | 숫자 드롭다운 | 하 |
| 8 | `SNumericEntryBox` | 숫자 입력 박스 | ✅ 완료 (P3) |
| 9 | `SSegmentedControl` | 세그먼트 컨트롤 | ✅ 완료 (P3) |
| 10 | `SSuggestionTextBox` | 자동완성 텍스트 | 중 |
| 11 | `SVirtualJoystick` | 가상 조이스틱 | 중 |
| 12 | `SVolumeControl` | 볼륨 컨트롤 | 하 |

### 26. 누락 위젯 — Text / View / Color / 기타 (16개)

| # | 위젯 | 설명 | 난이도 |
|---|------|------|--------|
| 1 | `SInlineEditableTextBlock` | 더블클릭 → 편집 전환 텍스트 | ✅ 완료 (P3) |
| 2 | `STextScroller` | 스크롤 텍스트 | ✅ 완료 (P3) |
| 3 | `ISlateEditableTextWidget` | 편집 텍스트 인터페이스 | 하 |
| 4 | `STileView` | 타일 그리드 뷰 | 중 |
| 5 | `STableRow` | 테이블 행 (컬럼/스타일/드래그 재정렬) | 중 |
| 6 | `STableViewBase` | 뷰 베이스 클래스 | 중 |
| 7 | `SColorBlock` | 단색 블록 | ✅ 완료 (P3) |
| 8 | `SColorGradingWheel` | 컬러 그레이딩 휠 | 상 |
| 9 | `SColorSpectrum` | 색상 스펙트럼 | 중 |
| 10 | `SSimpleGradient` / `SComplexGradient` | 그라데이션 위젯 | ✅ 완료 (P3) |
| 11 | `SThrobber` / `SSpinningImage` | 로딩 인디케이터 | ✅ 완료 (P3) |
| 12 | `SErrorHint` / `SErrorText` | 에러 표시 | ✅ 완료 (P3) |
| 13 | `SToolTip` (위젯) | 커스텀 툴팁 위젯 (현재 문자열만) | 중 |
| 14 | `SViewport` | 3D 뷰포트 임베딩 | 상 |
| 15 | `SInvalidationPanel` | 무효화 영역 래퍼 (P0#1 선행) | 중 |
| 16 | `SLayerManager` / `STooltipPresenter` | 레이어/툴팁 관리 위젯 | 중 |

### 27. Layout 시스템 세부 보완

> UE 참조: `SlateCore/Public/Layout/` (18 파일)

- [ ] Slot 타입 시스템 — 위젯 타입별 전용 Slot (현재 BoxSlot 공유)
- [ ] `StretchContent` 크기 규칙 — grow/shrink 별도 계수
- [ ] `FCombinedChildren` — 다중 자식 컬렉션 합성
- [ ] `LayoutGeometry` — 레이아웃 단계 전용 지오메트리
- [ ] `FlowDirection` — LTR/RTL 레이아웃 방향 지원
- [ ] 가시성 기반 자동 필터링 — ArrangedChildren에서 EVisibility 필터

### 28. Enum / 타입 보완

> UE 참조: `SlateCore/Public/Types/SlateEnums.h`

- [ ] `EButtonClickMethod` — DownAndUp / MouseDown / MouseUp / PreciseClick
- [ ] `EButtonTouchMethod` — DownAndUp / Down / PreciseTap
- [ ] `EButtonPressMethod` — DownAndUp / ButtonPress / ButtonRelease
- [ ] `ENavigationSource` — FocusedWidget / WidgetUnderCursor
- [ ] `ENavigationGenesis` — Keyboard / Controller / User
- [ ] `ETextCommit` — Default / OnEnter / OnUserMovedFocus / OnCleared
- [ ] `ESelectInfo` — OnKeyPress / OnNavigation / OnMouseClick / Direct
- [ ] `EScrollDirection` — Down / Up
- [ ] 누락 `MenuPlacement` 변형 — CenteredBelowAnchor / BelowRightAnchor / ComboBoxRight / RightLeftCenter / MatchBottomLeft

---

## 이미 잘 구현된 영역 (참고)

유지보수 및 점진 개선만 필요한 영역:

| 영역 | 완성도 | 비고 |
|------|--------|------|
| 애니메이션 시스템 | ~90% | UE 초과 수준 (Verlet/Arrive/13종 이징) |
| 커맨드 시스템 | ~75% | 계층 컨텍스트, InputChord, UIAction 잘 구현 |
| 도킹 시스템 | ~70% | 트리/탭/사이드바/컴패스/JSON 직렬화 |
| 포커스/네비게이션 | ~70% | FocusManager, TabIndex, 방향 네비게이션 |
| 팝업/툴팁 | ~70% | MenuPlacement, TransitionEffect, 모달 |
| 알림 시스템 | ~60% | 토스트 알림, 레벨별 색상, 자동 만료 |
| 접근성 | ~50%+ | WAI-ARIA 역할 매핑 (UE에 없는 고유 기능) |
| 사운드 시스템 | ~60% | 이벤트-사운드 매핑, 스팸 방지, 볼륨/피치 |
| 입력 전처리기 | ~60% | 우선순위 기반 파이프라인 |
| 테마/스타일 | ~55% | JSON 직렬화, 60+ 색상, StyleSet key-value |

---

## 구현 권장 순서

```
Phase 1 (기반) ✓ 완료
  P0#1 FastUpdate/무효화 ──→ P0#2 엘리먼트 캐싱  ✓ 완료
  P1#7 반응형 속성 ──────→ (P0#1과 연결)          ✓ 완료
  P1#12 Active Timer                              ✓ 완료
  P1#8 렌더 트랜스폼                              ✓ 완료

Phase 2 (핵심 기능) ✓ 완료
  P1#6  드래그앤드롭                                ✓ 완료
  P0#5  멀티 윈도우 (보완)                          ✓ 완료 (팝업 인프라만)
  P1#9  계층적 클리핑                               ✓ 완료 (스텐실 인프라만)

Phase 3 (에디터 필수) ✓ 완료
  P0#3  MultiBox 메뉴/툴바 빌더                       ✓ 완료
  P0#4  텍스트 프레임워크                              ✓ 완료
  P1#10 모달 윈도우                                    ✓ 완료 (포커스트래핑+입력차단)
  P1#11 폰트 확장                                      ✓ 완료 (SDF 파이프라인만 미완)

Phase 4 (완성도)
  P2#13 스타일링 강화
  P2#14 부모 추적/위젯 경로
  P2#15 도킹 보완
  P2#16 커맨드 보완
  P2#17 알림 보완
  P2#18 디버깅 인프라

Phase 5 (위젯 확장)
  P3#24-26 누락 위젯 추가 (우선순위별)

Phase 6 (부가 기능)
  P2#19-21 텍스처/브러시/렌더링 세부
  P3#22    이벤트/입력 확장
  P3#27-28 레이아웃/Enum 보완
```

---

## 수치 요약

| 카테고리 | 항목 수 | 비고 |
|----------|---------|------|
| P0 Critical | 5개 시스템, ~50 항목 | 성능/구조적 핵심 |
| P1 Major | 7개 시스템, ~40 항목 | 에디터 완성도 |
| P2 Moderate | 9개 시스템, ~55 항목 | 품질/편의성 |
| P3 Minor | 7개 시스템, ~88 항목 | 위젯 42개 + 기타 |
| **총계** | **~233 항목** | |

---

## Phase 3 복기 (2026-01-31)

### 구현 내용 요약

Phase 3에서 4개 시스템을 동시에 구현했다. 총 ~2,240줄, 신규 파일 6개, 수정 파일 12개.

### P1#11 폰트 확장 (Step 1–4)

- `FontWeight`(Thin~Black 9단계) + `FontStyle`(Normal/Italic/Oblique) + `FontSelector`(family+weight+style) 타입 추가 → `core/font_family.rs`
- `FontMetrics`(ascent/descent/line_gap/x_height/cap_height) + `FontMetricsCache` 싱글톤(OnceLock+RwLock) → `render/font_metrics.rs` 신규
- TextMeasurer에 `font_variant_chains` + `resolve_font_chain()` + `add_text_with_selector()` 추가 → `render/text_renderer.rs`
- `DrawElement::StyledText` 변형 + `add_styled_text()` + 렌더러 경로 추가 → `widget/traits.rs`, `render/renderer.rs`
- SDF 토글 인프라 (`sdf_enabled` HashMap) — 실제 파이프라인은 다음 패스

**배운 점:** ab_glyph API가 버전마다 다르다. `glyph_bounds()` 대신 `outline_glyph().px_bounds()` 패턴을 써야 했음. 기존 코드(text_renderer.rs)를 참고해서 맞췄다.

### P1#10 모달 윈도우 (Step 5–7)

- `FocusManager`에 `push_modal_scope()` / `pop_modal_scope()` 추가. `navigate()`에서 scope 내 위젯만 후보로 필터링 → `framework/focus.rs`
- `ModalInputFilter` — InputPreProcessor 구현, 모달 활성 시 Escape 외 키 이벤트 차단 → `framework/modal_input_filter.rs` 신규
- PopupLayer에 `modal_scope_id`, `ModalDismissEvent`, `take_modal_events()` 추가 → `framework/popup.rs`
- SlateApp에 `focus_manager()` 트레이트 메서드 + 모달 push/dismiss 시 FocusManager 연동 → `application/slate_app.rs`

**배운 점:** `FocusableWidget`에 `scope_id: Option<WidgetId>` 필드를 넣어서, 모달 안의 위젯만 Tab 탐색 대상이 되도록 했다. 단순하지만 효과적인 설계.

### P0#4 텍스트 프레임워크 (Step 8–11)

- `ITextRun` 트레이트 + `FSlateTextRun` / `FSlateWidgetRun` 구체 타입 + `TextRunStyle` / `TextRange` → `render/text_run.rs` 신규
- `TextLayout` 엔진 — `ShapedGlyphEntry`, `ShapedTextLine`, `LineBreakMode`(NoWrap/WordWrap/CharWrap), `TextLayoutParams`, `TextLayoutResult` → `render/text_layout.rs` 신규
  - 줄바꿈: WordWrap은 마지막 공백/CJK 경계로 역추적, CharWrap은 즉시 분리
  - CJK/한글/가타카나/히라가나 판별로 단어 경계 판단
  - `FontMetricsCache` 연동으로 라인별 baseline 계산
- `SRichTextBlock` — `TextRun.to_run_style()` 변환, `cached_layout`, `get_or_compute_layout()`, TextLayout 기반 on_paint → `widget/s_rich_text_block.rs`
- `STextBlock` — wrapping 모드일 때 `TextLayout::layout_simple()` 사용, 멀티라인 렌더링 → `widget/s_text_block.rs`

**배운 점:** TextLayout 엔진은 TextMeasurer 싱글톤에 의존하는데, 테스트에서 폰트 데이터가 없을 때 fallback 메트릭스(font_size * 0.6)를 쓰도록 해야 테스트가 안정적으로 돈다. `measure_char_advance` 분리가 핵심.

### P0#3 MultiBox 메뉴/툴바 빌더 (Step 12–14)

- `MultiBlockType`(Button/Toggle/Check/Radio/Separator/SubMenu/Widget/Heading) + `MultiBlockEntry` 빌더 패턴 + `MultiBoxBuilder` → `framework/multi_box.rs` 신규
  - `build_menu_items()`: entry → MenuItem 재귀 변환, CommandId로 label/shortcut 자동 조회
- `SMultiBoxToolbar` 위젯 — 수평 레이아웃, 호버/프레스 상태, `on_command` 콜백 → `widget/s_multi_box_toolbar.rs` 신규
- `MenuItem::from_command()`, `MenuItem::submenu_static()` 편의 생성자 → `widget/s_menu.rs`

**배운 점:** UICommandList에 `execute()` 메서드가 없었다. 직접 실행 대신 `on_command: Option<Box<dyn Fn(CommandId)>>` 콜백 패턴으로 해결. 위젯은 실행 책임을 갖지 않고, 상위에서 주입하는 게 맞다.

### 빌드/테스트 결과

- `cargo build` 성공 (경고만 존재, 에러 0)
- `cargo test` — 84 통과, 2 실패 (기존 실패: `test_arrive_interpolator`, `test_hit_test`)
- 신규 테스트 15개 전부 통과

### 다음 단계: Phase 4

Phase 4는 완성도 개선 — P2#13 스타일링 강화, P2#14 부모 추적/위젯 경로, P2#15 도킹 보완, P2#16 커맨드 보완, P2#17 알림 보완, P2#18 디버깅 인프라.
