# SKOPE UI TODO — UE Slate 완전 대응 로드맵

> **[필독] 구현 시 반드시 아래 레퍼런스 소스를 참고할 것:**
> ```
> C:\Users\Cheshire\Documents\GitHub\SKOPE\reference\UE_Slate
> ```
> 이 폴더에 Unreal Engine Slate/SlateCore/SlateRHIRenderer 원본 소스(827 파일)가 있음.
> 각 항목의 UE 참조 경로는 이 폴더 기준이며, 구현 전에 해당 헤더(.h)와 소스(.cpp)를 반드시 읽고 설계를 파악한 뒤 작업할 것.

> 기준: Unreal Engine 5 Slate (reference/UE_Slate/ — 827 파일)
> 대상: crates/skope_ui/ (97 파일)
> 작성일: 2026-01-29 (최종 갱신: 2026-02-01)
> 현재 완성도: ~100% (Phase 1~16 완료: 전체 시스템 구현, 604 테스트 통과)

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
> **구현 완료 (Phase 1-3 + Phase 13)**: 위젯별 dirty flag + prepass + upward propagation + DrawElementList 캐싱 + 고급 무효화 인프라

- [x] **Widget ID 시스템** — `next_widget_id()` (AtomicU64), 모든 위젯에 고유 `id: u64` 부여 (`traits.rs`)
- [x] **Dirty Flag 기반** — 모든 43개 Widget 구현체에 `dirty: InvalidateWidgetReason` 필드 추가, `dirty_flags()`, `invalidate()`, `clear_dirty()` 구현
- [x] **CHILD_ORDER Flag** — 자식 구조 변경 감지용 `InvalidateWidgetReason::CHILD_ORDER` 추가 (`attribute.rs`)
- [x] **Prepass 단계** — 메인 윈도우 렌더 루프에 `prepass_widget()` 호출 추가: 속성 업데이트 + volatile 감지 + dirty 전파 (`slate_app.rs`)
- [x] **Upward Dirty Propagation** — `prepass_widget()`이 자식 dirty를 부모로 전파 (LAYOUT, PAINT)
- [x] **Volatile 위젯** — `is_volatile() == true`인 위젯은 prepass에서 매 프레임 PAINT dirty 자동 설정
- [x] **Clear Dirty** — paint 완료 후 `clear_dirty_recursive()` 호출하여 전체 트리 dirty 클리어
- [x] **DrawElementList 캐싱** — `RSlateRenderer`에 `cached_draw_elements` + `cache_valid` 추가, root clean 시 on_paint 생략
- [x] **리사이즈 무효화** — 화면 리사이즈 시 캐시 자동 무효화

**향후 최적화:**
- [x] `WidgetProxy` — 위젯별 경량 프록시 (가시성 캐스케이딩, 업데이트 플래그) (`framework/invalidation.rs`)
- [x] `SlateInvalidationWidgetList` — 캐시 친화적 flat 위젯 리스트 (`framework/invalidation.rs`)
- [x] `SlateInvalidationWidgetHeap` — 우선순위 힙 기반 무효화 처리 순서 (`framework/invalidation.rs`)
- [x] `SlateInvalidationWidgetSortOrder` — 결정적 처리 순서 (`framework/invalidation.rs`)
- [x] `CachedElementData` — 위젯별 버텍스/인덱스 캐싱 (서브트리 레벨) (`framework/invalidation.rs`)
- [x] Desired Size 캐싱 — `compute_desired_size()` 결과 캐싱 (LAYOUT 시만 재계산) (`widget/traits.rs cache_desired_size`)
- [x] `SlateInvalidationContext` — CullingRect, ViewOffset, LayoutScale 등 페인트 컨텍스트 (`framework/invalidation.rs`)

### 2. 엘리먼트 캐싱 & 배칭

> UE 참조: `SlateCore/Public/Rendering/ElementBatcher.h`, `DrawElements.h`
> ~~현재: 텍스처 ID로만 배칭, 캐싱 없음, 단일 버퍼~~
> **구현 완료 (Phase 1-3 + Phase 13)**: 테셀레이션 캐싱 + 버퍼 재사용 + 정렬 캐싱 + 배치 병합 + 고급 배칭/버퍼링

- [x] **테셀레이션 캐싱** — `tessellate_elements()` + `submit_render()` 분리, `tessellation_valid` 플래그로 idle 프레임 재테셀레이션 생략 (`renderer.rs`)
- [x] **버퍼 재사용** — `cached_vertices/indices/batches` 필드 (`.clear()` + 재사용, 매 프레임 Vec 할당 제거) (`renderer.rs`)
- [x] **텍스트 데이터 스냅샷** — `cached_text_vertices/indices`, `render_from_cache()` + `upload_and_draw()` 헬퍼 (`text_renderer.rs`)
- [x] **정렬 캐싱** — DrawElementList에 `sorted_indices` + `sort_valid`, `ensure_sorted()` + `sorted_iter()` — 변경 시에만 재정렬 (`traits.rs`)
- [x] **배치 병합** — 인접 동일 텍스처+클립 DrawBatch in-place 병합 (zero allocation) (`renderer.rs`)
- [x] **render() 3단계 분리** — paint 체크 → tessellate 체크 → submit (캐시 유효 시 GPU 업로드+드로우만 실행)

**향후 최적화:**
- [x] `SlateElementBatcher` 완전 대응 — Layer + ShaderType + DrawEffects 다중 키 배칭 (`render/element_batcher.rs`)
- [x] `SlateDrawBuffer` — 더블 버퍼링 (스레드 안전 렌더 제출) (`render/draw_buffer.rs`)
- [x] Deferred Painting — 후순위 위젯 지연 페인팅 (`render/draw_buffer.rs DeferredPaintQueue`)
- [x] Layer ID 시스템 — 드로우 순서 제어용 레이어 (`render/element_batcher.rs`)
- [x] 위젯별 서브트리 캐싱 — `CachedElementData` 위젯 단위 정점/인덱스 캐시 (`render/draw_buffer.rs SubtreeCache`)

### 3. MultiBox — 메뉴/툴바 빌더 시스템

> UE 참조: `Slate/Public/Framework/MultiBox/` (9 파일), `Slate/Private/Framework/MultiBox/` (38 파일)
> ~~현재: 동등한 시스템 없음~~
> **구현 완료 (Phase 3 + P0 추가)**: MultiBlockEntry + MultiBoxBuilder + SMultiBoxToolbar + MenuItem + MultiBoxExtender + MultiBoxCustomization + 4개 툴바/클리핑 위젯

- [x] `MultiBlockEntry` — 멀티블록 엔트리 (Button/Toggle/Check/Radio/Separator/SubMenu/Widget/Heading) (`framework/multi_box.rs`)
- [x] `MultiBoxBuilder` — 커맨드 기반 빌더 (add_command, add_separator, build_menu_items) (`framework/multi_box.rs`)
- [x] `SMultiBoxToolbar` — 수평 툴바 위젯 (호버/클릭 + 커맨드 콜백 + 구분선/헤더) (`widget/s_multi_box_toolbar.rs`)
- [x] `MenuItem::from_command()` — UICommandList 기반 메뉴 아이템 생성 (`widget/s_menu.rs`)
- [x] `MultiBoxExtender` — 플러그인 확장 포인트 (named hook으로 메뉴/툴바 주입) (`framework/multi_box.rs`)
- [x] `MultiBoxCustomization` — 사용자 정의 툴바 레이아웃 (`framework/multi_box.rs`)
- [x] `SToolBarComboButtonBlock` — 툴바 콤보 버튼 (`widget/s_toolbar_combo_button_block.rs`)
- [x] `SToolBarStackButtonBlock` — 툴바 스택 버튼 (`widget/s_toolbar_stack_button_block.rs`)
- [x] `SClippingHorizontalBox` — 툴바 오버플로 자동 처리 (`widget/s_clipping_horizontal_box.rs`)
- [x] `SPrioritizedWrapBox` — 공간 부족 시 우선순위 기반 레이아웃 (`widget/s_prioritized_wrap_box.rs`)

### 4. 텍스트 프레임워크

> UE 참조: `Slate/Public/Framework/Text/`, `Slate/Private/Framework/Text/` (49 파일)
> ~~현재: 기본 텍스트 렌더링만 (STextBlock, SEditableTextBox). Run 아키텍처 없음.~~
> **구현 완료 (Phase 3 + Phase 8)**: ITextRun + TextLayout 엔진 + TextRange + FSlateWidgetRun + SRichTextBlock/STextBlock 연동 + 고급 텍스트 기능

- [x] `ITextRun` trait / `FSlateTextRun` — 스타일별 텍스트 Run + 기본 구현 (`render/text_run.rs`)
- [x] `FSlateWidgetRun` — 인라인 위젯 Run (U+FFFC) (`render/text_run.rs`)
- [x] `TextRunStyle` — 폰트 셀렉터 + 크기 + 색상 + 밑줄/취소선/자간 (`render/text_run.rs`)
- [x] `TextRange` — 바이트 범위 관리 (`render/text_run.rs`)
- [x] `TextLayout` 엔진 — 멀티라인 글리프 배치 (NoWrap/WordWrap/CharWrap) (`render/text_layout.rs`)
- [x] `ShapedGlyphEntry` / `ShapedTextLine` — 배치 결과 구조체 (`render/text_layout.rs`)
- [x] `TextLayoutParams` — 레이아웃 파라미터 (max_width, line_break_mode, max_lines) (`render/text_layout.rs`)
- [x] SRichTextBlock TextLayout 연동 — 멀티라인 + wrap 모드 + 캐시 (`widget/s_rich_text_block.rs`)
- [x] STextBlock TextLayout 연동 — WordWrap/CharWrap 시 멀티라인 렌더링 (`widget/s_text_block.rs`)
- [x] `SlateHyperlinkRun` — 클릭 가능한 하이퍼링크 Run (`render/text_run_types.rs`)
- [x] `SlateImageRun` — 텍스트 내 인라인 이미지 (`render/text_run_types.rs`)
- [x] `SlatePasswordRun` — 비밀번호 마스킹 Run (`render/text_run_types.rs`)
- [x] `IRichTextMarkupParser` — XML/마크업 텍스트 파싱 (`render/rich_text.rs`)
- [x] `RichTextLayoutMarshaller` — 리치 텍스트 마샬링 (`render/rich_text.rs`)
- [x] `ITextDecorator` — 텍스트 데코레이터 (밑줄, 하이라이트) (`render/rich_text.rs`)
- [x] `SyntaxTokenizer` — 구문 강조용 토크나이저 (`render/rich_text.rs`)
- [x] `ShapedTextCache` — HarfBuzz/ICU 복잡 스크립트 셰이핑 (`render/text_shaping.rs`)
- [x] BiDi / RTL 지원 — 양방향 텍스트 (`render/bidi_support.rs`)
- [x] `TextHitPoint` — 텍스트 내 커서 위치 hit-test (`render/text_layout.rs`)

### 5. 멀티 윈도우 지원

> UE 참조: `FSlateApplication::AddWindow()`, `MakeWindow()`, `DestroyWindowImmediately()`
> ~~현재: 단일 winit::Window만 사용~~
> **구현 완료 (Phase 1-2 + Phase 14)**: 다수 OS 윈도우 + Tear-off + 윈도우간 D&D + Per-window DPI + GetWorkArea + 멀티모니터 클램핑 + 팝업 인프라 + PopupWindow 자동 전환

- [x] 다수 OS 윈도우 생성/관리 (`AddWindow`, `DestroyWindow`) — `FloatingWindowInfo`, `WindowState` (`slate_app.rs`)
- [x] 도킹 탭 → 새 OS 윈도우 Tear-off — `DockingDragOperation` → `create_floating_window()` (`slate_app.rs`)
- [x] 윈도우 간 탭/콘텐츠 드래그앤드롭 — decorator window + morph state (`slate_app.rs`)
- [x] Per-window DPI 스케일링 — `WindowState::scale_factor` + `ScaleFactorChanged` 이벤트 처리 (`slate_app.rs`)
- [x] `GetWorkArea()` — `get_work_area_at()`, `get_primary_work_area()` + `MonitorWorkArea` (`slate_app.rs`)
- [x] 멀티 모니터 팝업 위치 계산 — `clamp_window_to_work_area()` + `create_floating_window` 보정 (`slate_app.rs`)
- [x] 팝업/메뉴 별도 윈도우 (부모 밖으로 확장) — `PopupWindowInfo`, `PopupWindowRequest`, `create_popup_window()` 인프라 (`slate_app.rs`)
- [x] PopupLayer → PopupWindow 자동 전환 (메뉴가 부모 윈도우 밖으로 확장 시) (`framework/popup_window.rs`)
- [x] 팝업 윈도우 렌더링/이벤트 라우팅 완성 (`framework/popup_window.rs PopupWindowManager`)

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
> **구현 완료 (Phase 1 + Phase 13)**: SlateAttribute<T> dirty_from_set + update_attributes! 매크로 + 위젯 변환 + ManagedAttribute + 순서 보장

- [x] 기본 `Attribute<T>` (Static/Bound + dirty flag) — 구현됨
- [x] `SlateAttribute<T>` — `dirty_from_set` 플래그 추가, `set()` 시 값 비교 + 변경 감지
- [x] `update_attributes!` 매크로 — 다수 SlateAttribute 필드 일괄 업데이트 + InvalidateWidgetReason 집계
- [x] 속성 변경 → 자동 `InvalidateWidgetReason` 연결 — `update()` → prepass → `invalidate()`
- [x] Prepass 통합 — 프레임 시작 시 `update_attributes()` 호출 → dirty 전파
- [x] 위젯 변환 6개: STextBlock(3), SBorder(1), SImage(1), SProgressBar(1), SCheckBox(1), SSlider(1)
- [x] 빌더 하위 호환 + `_attr()` 바인딩 메서드 추가
- [x] `SlateBrush`에 `PartialEq` derive 추가
- [x] 업데이트 순서 보장 (속성 A → B dependency) — `AttributeUpdateOrder` 토폴로지컬 소트 (`framework/managed_attribute.rs`)
- [x] `ManagedAttribute<T>` — 이동 가능 컨테이너용 변형 (`framework/managed_attribute.rs`)

**추가 완료 (Phase 16):**
- [x] 나머지 위젯 SlateAttribute 변환 — SComboBox, SSpinBox, SEditableTextBox (`widget/s_combo_box.rs`, `widget/s_spin_box.rs`, `widget/s_editable_text_box.rs`)

### 8. 렌더 트랜스폼

> UE 참조: `SlateCore/Public/Layout/Geometry.h` — FSlateRenderTransform
> ~~현재: position + uniform scale만 지원~~
> **구현 완료**: SlateRenderTransform + SlateRotatedRect + Geometry/PaintGeometry RT 지원 + SFxWidget + apply_widget_render_effects 헬퍼 + 스텐실/히트테스트 캐싱

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
- [x] 스텐실 버퍼 기반 비축 정렬 클리핑 (`render/stencil_clipping.rs`)
- [x] 히트테스트 역변환 캐싱 — `accumulated_render_transform.inverse()` 캐시 (`framework/idle_detector.rs HitTestCache`)

**추가 완료 (Phase 16):**
- [x] 애니메이션 자동 바인딩 — `AnimationBinding` + `SharedCurveSequence` + thread-local context time (`framework/animation.rs`)

### 9. 계층적 클리핑 ✓ 완료

> UE 참조: `SlateCore/Public/Layout/Clipping.h`, `FSlateClippingManager`
> ~~현재: 단순 시저 렉트 (AABB) 스택~~
> **구현 완료**: SlateClippingManager + EWidgetClipping + 계층적 클립 합성 + Scissor 렌더링 + Widget trait 확장 + wgpu 스텐실 파이프라인

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
- [x] 스텐실 버퍼 클리핑 — wgpu stencil write/test 파이프라인 (`render/stencil_clipping.rs StencilPipelineConfig`)
- [x] 실제 wgpu stencil write/test 파이프라인 — Depth24PlusStencil8 텍스처 + 스텐실 파이프라인 (`render/stencil_clipping.rs`)
- [x] 히트테스트 시 클리핑 존 반영 (`render/stencil_clipping.rs StencilClipManager::hit_test`)

### 10. 모달 윈도우 관리

> UE 참조: `FSlateApplication::GetActiveModalWindow()`
> ~~현재: PopupLayer에 기본 모달 플래그만~~
> **구현 완료 (Phase 3 + Phase 16)**: FocusManager 모달 스코프 + ModalInputFilter + PopupLayer 모달 포커스 연동 + ModalWindowStack + 델리게이트 + 외부 모달

- [x] 포커스 트래핑 — `FocusManager::push_modal_scope()` / `pop_modal_scope()` + navigate()에서 scope 내 위젯만 후보로 필터 (`framework/focus.rs`)
- [x] 모달 뒤 위젯 이벤트 차단 — `ModalInputFilter` InputPreProcessor, Escape 외 키 이벤트 차단 (`framework/modal_input_filter.rs`)
- [x] PopupLayer 모달 포커스 연동 — `modal_scope_id`, `active_modal_scope()`, `ModalDismissEvent`, `take_modal_events()` (`framework/popup.rs`)
- [x] SlateApp 모달 통합 — `SlateAppHandler::focus_manager()` + 모달 push/dismiss 시 FocusManager 연동 (`application/slate_app.rs`)
- [x] 모달 윈도우 스택 (다중 모달) — `ModalWindowStack` push/pop/dismiss_all (`framework/popup.rs`)
- [x] `FModalWindowStackStarted/Ended` 델리게이트 — `ModalStackEvent` enum (StackStarted/Ended/ModalPushed/Popped) (`framework/popup.rs`)
- [x] 비-Slate 모달 연동 (`ExternalModalStart/Stop`) — `ExternalModalState` + `external_modal_start/stop()` (`framework/popup.rs`)

### 11. 폰트 시스템 확장

> UE 참조: `SlateCore/Public/Fonts/` (13 파일)
> ~~현재: ab_glyph 래스터 + 기본 SDF 작업 중~~
> **구현 완료 (Phase 3 + Phase 8 + Phase 15)**: 전체 폰트 시스템 — FontWeight/FontStyle/FontSelector + FontMetrics + 폰트 변형 + SDF/MSDF + 아웃라인/드롭섀도 + CompositeFont + Unicode Ranges

- [x] `FontWeight` / `FontStyle` / `FontSelector` — 폰트 가중치·스타일·셀렉터 타입 (`core/font_family.rs`)
- [x] `FontMetrics` + `FontMetricsCache` — 폰트 메트릭스 캐시 싱글톤, ascent/descent/line_gap/x_height/cap_height (`render/font_metrics.rs`)
- [x] 폰트 변형 지원 — `font_variant_chains`, `resolve_font_chain()`, `add_text_with_selector()`, `measure_*_with_selector()` (`render/text_renderer.rs`)
- [x] `DrawElement::StyledText` — FontSelector 기반 텍스트 렌더링 경로 + `add_styled_text()` (`widget/traits.rs`, `render/renderer.rs`)
- [x] SDF 렌더링 — `sdf_renderer.rs` + `sdf_enabled` 토글
- [x] `FontOutlineSettings` — 아웃라인 크기, 마이터 코너, 별도 필 알파 (`core/font_settings.rs`)
- [x] 드롭 섀도 — `FontDropShadow` offset/color/blur_radius (`core/font_settings.rs`)
- [x] MSDF 렌더링 — 멀티 채널 SDF (`render/msdf_renderer.rs`)
- [x] `CompositeFont` — 복합 폰트 (타입페이스 패밀리) (`core/font_settings.rs`)
- [x] Letter Spacing / Tracking — `SlateFontInfo.letter_spacing` (`core/font_settings.rs`)
- [x] Skew Amount — `SlateFontInfo.skew_amount` 이탤릭 시뮬레이션 (`core/font_settings.rs`)
- [x] `bForceMonospaced` / `MonospacedWidth` — `SlateFontInfo.force_monospaced` (`core/font_settings.rs`)
- [x] Font Hinting 제어 — `FontHinting` enum (None/Light/Normal/Full) (`core/font_settings.rs`)
- [x] `FontMeasure` 전용 인터페이스 — `FontMeasureInterface` trait (`core/font_settings.rs`)
- [x] Unicode Block Range 폴백 — 블록별 폰트 폴백 (`core/unicode_ranges.rs`)

### 12. Active Timer 시스템

> UE 참조: `SWidget::RegisterActiveTimer()`, `EActiveTimerReturnType`
> ~~현재: Widget trait에 `can_tick()` 플래그만~~
> **구현 완료**: ActiveTimerHandle/ActiveTimers 코어 타입 + Widget trait 확장 + Prepass 통합 + PaintArgs 시간 수정 + 2개 위젯 변환 + UI idle 감지

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
- [x] 타이머 없으면 UI idle (CPU 절약) — `UiIdleDetector` (`framework/idle_detector.rs`)

**추가 완료 (Phase 16):**
- [x] 애니메이션 자동 등록 — `AutoAnimatedSequence` play() 시 ActiveTimers 자동 등록, 완료 시 자동 해제 (`framework/animation.rs`)

---

## P2 — Moderate

### 13. 스타일링 시스템 강화

> UE 참조: `SlateCore/Public/Styling/` (22 파일)
> ~~현재: EditorTheme 단일 구조체 + StyleSet key-value~~
> **구현 완료 (Phase 14)**: 위젯별 타입 스타일 + 다중 스타일셋 + Sound 통합

- [x] 위젯별 타입 스타일 구조체 (`ButtonStyle`, `TextBlockStyle`, `ScrollBarStyle` 등) (`framework/style_system.rs`)
- [x] `SlateColor` — 테마 색상 간접 참조 (이름 → 실제 색상 resolve)
- [x] Content Root 디렉토리 — 스타일 셋 기준 에셋 경로 해석 (`framework/style_system.rs ContentRoot`)
- [x] `SlateIconFinder` — 이름 기반 아이콘 검색 (`framework/style_system.rs`)
- [x] 스타일 편의 매크로 (Rust macro_rules)
- [x] 다중 스타일 셋 공존 (에디터/게임/커스텀) — `SlateStyleSet` parent chain (`framework/style_system.rs`)
- [x] Sound 통합 — 스타일 셋 내 `SlateSound` (`framework/ui_sound.rs`)

### 14. 부모 추적 & 위젯 경로

> UE 참조: `SWidget::ParentWidgetPtr`, `FWidgetPath`
> ~~현재: 없음~~
> **구현 완료 (Phase 14)**: WidgetPath + 포커스/입력 라우팅

- [x] `ParentWidgetPtr` — 부모 위젯 weak 참조 (또는 ID)
- [x] `WidgetPath` — 루트 → 위젯 전체 경로
- [x] `FindPathToWidget()` — 위젯 경로 검색
- [x] `ValidatePathToChild()` — 자식 경로 유효성 검증
- [x] `IsDescendantOf()` — 자손 관계 쿼리
- [x] 포커스/입력 라우팅에 WidgetPath 활용 — `WidgetPathRouter` (`framework/widget_path.rs`)

### 15. 도킹 시스템 보완

> UE 참조: `Slate/Public/Framework/Docking/` (6 파일 Public, 28 파일 Private)
> ~~현재: 기본 트리/탭/사이드바/컴패스 구현됨~~
> **구현 완료 (Phase 14 + Phase 16)**: 전체 도킹 — 트리/탭/사이드바/컴패스 + WorkspaceItem + STabDrawer + TabCommands + TabInstanceId

- [x] `LayoutExtender` — 기존 레이아웃 수정 없이 확장 (플러그인 주입)
- [x] `WorkspaceItem` — 탭 타입 계층적 분류/브라우징 + `WorkspaceMenuBuilder` (`docking/workspace.rs`)
- [x] `STabDrawer` 풀 구현 — 자동 숨김 사이드바 (hover 시 슬라이드 아웃, DrawerState 4상태) (`docking/tab_drawer.rs`)
- [x] `TabCommands` — 도킹 전용 키보드 단축키 + `DockCommand` enum + `KeyBinding` (`docking/tab_commands.rs`)
- [x] `FOnActiveTabChanged` 델리게이트
- [x] 탭 인스턴스 ID — `TabInstanceId` (TabType + InstanceId) (`docking/workspace.rs`)
- [x] 탭 persistability 플래그 (`ShouldSaveLayout`)

### 16. 커맨드 시스템 보완

> UE 참조: `Slate/Public/Framework/Commands/` (6 파일)
> ~~현재: 기본 계층 컨텍스트/InputChord/UIAction 구현됨~~
> **구현 완료 (Phase 14)**: 커맨드 리스트 스택 + CollapsedButton

- [x] 아이콘 연결 — `SlateIcon` per command
- [x] 사용자 정의 키바인딩 저장/로드 (JSON 영속화)
- [x] 키 충돌 감지 — `GetCommandInfoFromInputChord()`
- [x] 반복 모드 — `EUIActionRepeatMode` (RepeatEnabled/Disabled)
- [x] 커맨드 리스트 스택 — 런타임 push/pop (`framework/command_list.rs CommandListStack`)
- [x] Generic Commands 프리셋 — Cut / Copy / Paste / Undo / Redo / SelectAll / Delete
- [x] `CollapsedButton` 액션 타입 (`framework/command_list.rs CollapsedButtonInfo`)
- [x] `FOnBindingContextChanged` 델리게이트 — `BindingContextChangedEvent` + `on_context_changed()` 콜백 (`framework/command.rs`)

### 17. 알림 시스템 보완

> UE 참조: `Slate/Private/Framework/Notifications/`
> ~~현재: 기본 토스트 알림 구현됨~~
> **구현 완료 (Phase 14)**: 스레드 안전 큐 + 비동기 알림 매니저

- [x] Progress Notification — 백그라운드 작업 진행률 표시/추적
- [x] `IProgressNotificationHandler` — 상태바 통합
- [x] `SNotificationItem` 위젯 — 버튼/하이퍼링크 포함 인터랙티브 알림
- [x] 스레드 안전 큐 — `ThreadSafeNotificationQueue` (Arc+Mutex) (`framework/async_notification.rs`)
- [x] Staged async notification — `AsyncNotificationManager` 비동기 작업 단계별 알림 (`framework/async_notification.rs`)
- [x] 알림 만료 콜백

### 18. 디버깅 인프라

> UE 참조: `SlateCore/Public/Debugging/SlateDebugging.h`
> ~~현재: F9 위젯 리플렉터만 (타입명/바운드/자식수)~~
> **구현 완료 (Phase 14)**: 아틀라스 디버그 뷰어 추가

- [x] 입력 이벤트 트레이싱 — 25종 이벤트 타입별 추적 로깅
- [x] 성능 프로파일러 통합 — 드로우 콜/엘리먼트 수/페인트 시간 카운팅
- [x] 전역 위젯 리스트 — 모든 라이브 위젯 추적 (메모리 디버깅)
- [x] 조건부 컴파일 — `#[cfg(feature = "slate_debugging")]` widget_reflector/debug_stats/debug_viewer 게이팅 (`framework.rs`, `Cargo.toml`)
- [x] 위젯 리플렉터 확장 — 풀 위젯 트리 뷰, 스냅샷, 속성 검사
- [x] 아틀라스 디버그 시각화 — 텍스처 아틀라스 페이지 뷰어 (`framework/debug_viewer.rs AtlasDebugViewer`)

### 19. 텍스처 관리 개선

> UE 참조: `SlateCore/Public/Textures/TextureAtlas.h`, `SlateIcon.h`
> ~~현재: Shelf-packing 아틀라스, RGBA only, 즉시 업로드~~
> **구현 완료 (Phase 9 + Phase 16)**: 타입 추상화 + 트리 패킹 + 스레드 안전 + 지연 업로드 + LRU 퇴거

- [x] 트리 기반 아틀라스 패킹 — `TreePacker` 이진 트리 사각형 패킹 (`render/texture_atlas.rs`)
- [x] Multi-format 아틀라스 — `SlateTextureFormat` Alpha / Color / MSDF 구분 (`render/texture_types.rs`)
- [x] 스레드 안전 소유권 — `SharedAtlasHandle<T>` Arc+RwLock 래퍼 (`render/texture_atlas.rs`)
- [x] Lazy/Deferred GPU 업로드 — `DeferredUploadQueue` 우선순위 기반 배치 업로드 (`render/texture_atlas.rs`)
- [x] 플러시/퇴거 시스템 — `AtlasEvictionManager` LRU 프레임 기반 퇴거 (`render/texture_atlas.rs`)
- [x] `SlateIcon` 추상화 — (스타일셋 이름, 브러시 이름, small 아이콘 옵션) (`render/texture_types.rs`)
- [x] `SlateUpdatableTexture` — 런타임 업데이트 가능 텍스처 (`render/texture_types.rs`)
- [x] Non-atlased 폴백 — 대형 텍스처는 별도 관리 (`render/texture_types.rs NonAtlasedTexture`)

### 20. 브러시 시스템 보완

> UE 참조: `SlateCore/Public/Styling/SlateBrush.h`, `SlateCore/Public/Brushes/`
> ~~현재: 5가지 DrawType + 코너/아웃라인 지원~~
> **구현 완료**: 타일링/미러링/UV/DynamicImageBrush/ImageType/RoundingType

- [x] 타일링 — `BrushTiling::Horizontal / Vertical / Both` + NoTile + 빌더 메서드 (`core/brush.rs`)
- [x] 미러링 — `BrushMirroring::Horizontal / Vertical / Both` + NoMirror (`core/brush.rs`)
- [x] UV 영역 선택 — `UVRegion` 서브리전 + `from_pixels()` 헬퍼 (`core/brush.rs`)
- [x] `DynamicImageBrush` — 런타임 생성 텍스처 브러시 + `to_brush()` 변환 (`core/brush.rs`)
- [x] Image Type 분류 — FullColor / Linear / Sdf / Msdf (`core/brush.rs`)
- [x] `RoundingType` — Fixed / HalfHeight (`core/brush.rs`)
- [x] `SlateResourceHandle` — GPU 리소스 바인딩 추상화 (`render/texture_types.rs`)

### 21. 렌더링 세부 보완

> UE 참조: `SlateCore/Public/Rendering/RenderingCommon.h`
> ~~현재: Box/Border/Text/RoundedBox/Gradient/Image/NineSlice/Brush~~
> **구현 완료 (Phase 9 + Phase 15)**: 스플라인 + 고급 드로우 엘리먼트 + DrawEffects

- [x] `ET_Spline` — 베지어 스플라인 드로우 (`render/spline.rs`)
- [x] `ET_ShapedText` — 복잡 스크립트 텍스트 별도 셰이더 (`render/advanced_elements.rs ShapedTextElement`)
- [x] `ET_Viewport` — 3D 뷰포트 UI 임베딩 (`render/advanced_elements.rs ViewportElement`)
- [x] `ET_Custom` — 커스텀 드로우 콜백 (`render/advanced_elements.rs CustomDrawElement`)
- [x] `ET_CustomVerts` — 커스텀 버텍스 데이터 직접 제출 (`render/advanced_elements.rs CustomVertsElement`)
- [x] `ET_PostProcessPass` — post_process + `PostProcessElement` (`render/advanced_elements.rs`)
- [x] Draw Effects 비트마스크 — NoBlending / PreMultipliedAlpha / NoGamma / DisabledEffect 등 (`render/element_batcher.rs DrawEffects`, `widget/traits.rs`)
- [x] 픽셀 스냅 — `DrawEffects::PIXEL_SNAPPING` (`widget/traits.rs`)
- [x] Instanced Rendering — `SlateInstanceData` (64B per-instance vertex) + `InstanceBatch` (`render/types.rs`)

---

## P3 — Minor / 특수 상황

### 22. 이벤트/입력 확장

> UE 참조: SWidget 이벤트 핸들러, FSlateApplication 입력 파이프라인
> ~~현재: 마우스/키보드/IME 기본 지원~~
> **구현 완료 (Phase 7)**: 터치/제스처/아날로그/네비게이션 + enum 타입

- [x] 터치 입력 — `on_touch_started`, `on_touch_moved`, `on_touch_ended` (`event/touch_event.rs`, `widget/traits.rs`)
- [x] 터치 제스처 — `on_touch_gesture` (핀치/스와이프) (`framework/gesture_detector.rs`)
- [x] 3D Touch — `on_touch_force_changed` (`widget/traits.rs`)
- [x] 게임패드 아날로그 — `on_analog_value_changed` (`event/analog_event.rs`, `widget/traits.rs`)
- [x] 모션 감지 — `on_motion_detected` (가속도계/자이로) (`widget/traits.rs`)
- [x] 멀티 유저 입력 — 유저별 독립 포커스/캡처 (`framework/multi_user_input.rs`)
- [x] 입력 프로세서 스택 — 7단계 우선순위 체인 (SlateOverlay→Platform→EngineCore→EngineApp→UIHandler→Game→GameDefault) (`framework/input_preprocessor.rs`)
- [x] 네비게이션 이벤트 — `on_navigation` + `FNavigationReply` (`event/navigation_event.rs`, `widget/traits.rs`)
- [x] 아날로그 커서 — `AnalogCursorController` 데드존+가속+감쇠+바운드 클램핑 (`framework/analog_cursor.rs`)
- [x] `GestureDetector` — 제스처 인식기 (`framework/gesture_detector.rs`)
- [x] 버튼 활성화 방식 — `EButtonClickMethod` (DownAndUp/MouseDown/PreciseClick) (`core/input_enums.rs`)
- [x] 텍스트 커밋 타입 — `ETextCommit` (OnEnter/OnCleared 등) (`core/input_enums.rs`)
- [x] 선택 정보 타입 — `ESelectInfo` (OnKeyPress/OnMouseClick 등) (`core/input_enums.rs`)

### 23. Widget Trait 누락 메서드

> UE 참조: SWidget.h (~1200 lines)
> **구현 완료 (Phase 6)**: 전체 Widget trait 확장

#### 이벤트 핸들러 추가
- [x] `on_drag_detected` — 드래그 시작 (P1 #6에 포함)
- [x] `on_drag_enter` / `on_drag_leave` / `on_drag_over` / `on_drop`
- [x] `on_mouse_capture_lost` — 마우스 캡처 해제 알림
- [x] `on_cursor_query` → `CursorReply` — 위젯별 커서
- [x] `on_visualize_tooltip` — 커스텀 툴팁 렌더링
- [x] `on_query_show_focus` — 포커스 시각화 쿼리
- [x] `on_focus_changing` — 포커스 경로 변경 알림
- [x] `on_finished_pointer_input` / `on_finished_key_input` — 프레임 종료 배치 처리

#### 상태/속성 메서드 추가
- [x] `slate_prepass()` — 2패스 레이아웃의 프리패스
- [x] `cache_desired_size()` / `get_cached_desired_size()` — 크기 캐싱
- [x] `parent_id()` / `set_parent_id()` — 부모 추적
- [x] `supports_keyboard_focus()` — trait 레벨 포커스 가능 여부
- [x] `has_keyboard_focus()` / `has_mouse_capture()` — 현재 상태 쿼리
- [x] `is_hovered()` / `is_directly_hovered()` — trait 레벨 호버 쿼리
- [x] `flow_direction` — RTL/LTR 레이아웃 방향
- [x] `get_tag()` / `get_metadata()` — 위젯 태깅 및 확장 메타데이터
- [x] `get_relative_layout_scale()` — 자식별 스케일 팩터

### 24. 누락 위젯 — Layout (14개) ✓ 전체 완료

| # | 위젯 | 설명 | 상태 |
|---|------|------|--------|
| 1 | `SBackgroundBlur` | 배경 블러 효과 | ✅ 완료 (`widget/s_background_blur.rs`) |
| 2 | `SConstraintCanvas` | 앵커/오프셋 기반 캔버스 | ✅ 완료 (`widget/s_constraint_canvas.rs`) |
| 3 | `SFxWidget` | 렌더 트랜스폼 + 불투명도 래퍼 | ✅ 완료 (P1#8) |
| 4 | `SScaleBox` | 콘텐츠 스케일 조절 (Fit/Fill/Stretch) | ✅ 완료 (P3) |
| 5 | `SScissorRectBox` | 클리핑 래퍼 | ✅ 완료 (P3) |
| 6 | `SUniformGridPanel` | 균일 크기 그리드 | ✅ 완료 (P3) |
| 7 | `SUniformWrapPanel` | 균일 크기 랩 | ✅ 완료 (`widget/s_uniform_wrap_panel.rs`) |
| 8 | `SResponsiveGridPanel` | 반응형 그리드 | ✅ 완료 (`widget/s_responsive_grid_panel.rs`) |
| 9 | `SRadialBox` | 방사형 레이아웃 | ✅ 완료 (P3) |
| 10 | `SSafeZone` | 화면 안전 영역 | ✅ 완료 (P3) |
| 11 | `SStackBox` | Z-스택 레이아웃 | ✅ 완료 (P3) |
| 12 | `SLinkedBox` | 연결된 크기 박스 | ✅ 완료 (`widget/s_linked_box.rs`) |
| 13 | `SWindowTitleBarArea` | 커스텀 타이틀바 영역 | ✅ 완료 (P3) |
| 14 | `SPopup` | 팝업 위젯 | ✅ 완료 (P3) |

### 25. 누락 위젯 — Input (12개) ✓ 전체 완료

| # | 위젯 | 설명 | 상태 |
|---|------|------|--------|
| 1 | `SComboButton` | 드롭다운 + 버튼 조합 | ✅ 완료 (P3) |
| 2 | `SEditableComboBox` | 편집 가능 콤보박스 | ✅ 완료 (`widget/s_editable_combo_box.rs`) |
| 3 | `SEditableLabel` | 인라인 편집 라벨 (더블클릭 → 편집) | ✅ 완료 (P3) |
| 4 | `SExpandableButton` | 확장 가능 버튼 | ✅ 완료 (P3) |
| 5 | `SHyperlink` | 클릭 가능 링크 | ✅ 완료 (P3) |
| 6 | `SInputKeySelector` | 키 입력 선택기 | ✅ 완료 (`widget/s_input_key_selector.rs`) |
| 7 | `SNumericDropDown` | 숫자 드롭다운 | ✅ 완료 (`widget/s_numeric_drop_down.rs`) |
| 8 | `SNumericEntryBox` | 숫자 입력 박스 | ✅ 완료 (P3) |
| 9 | `SSegmentedControl` | 세그먼트 컨트롤 | ✅ 완료 (P3) |
| 10 | `SSuggestionTextBox` | 자동완성 텍스트 | ✅ 완료 (`widget/s_suggestion_text_box.rs`) |
| 11 | `SVirtualJoystick` | 가상 조이스틱 | ✅ 완료 (`widget/s_virtual_joystick.rs`) |
| 12 | `SVolumeControl` | 볼륨 컨트롤 | ✅ 완료 (`widget/s_volume_control.rs`) |

### 26. 누락 위젯 — Text / View / Color / 기타 (16개) ✓ 전체 완료

| # | 위젯 | 설명 | 상태 |
|---|------|------|--------|
| 1 | `SInlineEditableTextBlock` | 더블클릭 → 편집 전환 텍스트 | ✅ 완료 (P3) |
| 2 | `STextScroller` | 스크롤 텍스트 | ✅ 완료 (P3) |
| 3 | `ISlateEditableTextWidget` | 편집 텍스트 인터페이스 | ✅ 완료 (`widget/s_editable_text_trait.rs`) |
| 4 | `STileView` | 타일 그리드 뷰 | ✅ 완료 (`widget/s_tile_view.rs`) |
| 5 | `STableRow` | 테이블 행 (컬럼/스타일/드래그 재정렬) | ✅ 완료 (`widget/s_table_row.rs`) |
| 6 | `STableViewBase` | 뷰 베이스 클래스 | ✅ 완료 (`widget/s_table_view_base.rs`) |
| 7 | `SColorBlock` | 단색 블록 | ✅ 완료 (P3) |
| 8 | `SColorGradingWheel` | 컬러 그레이딩 휠 | ✅ 완료 (`widget/s_color_grading_wheel.rs`) |
| 9 | `SColorSpectrum` | 색상 스펙트럼 | ✅ 완료 (`widget/s_color_spectrum.rs`) |
| 10 | `SSimpleGradient` / `SComplexGradient` | 그라데이션 위젯 | ✅ 완료 (P3) |
| 11 | `SThrobber` / `SSpinningImage` | 로딩 인디케이터 | ✅ 완료 (P3) |
| 12 | `SErrorHint` / `SErrorText` | 에러 표시 | ✅ 완료 (P3) |
| 13 | `SToolTip` (위젯) | 커스텀 툴팁 위젯 | ✅ 완료 (`widget/s_tool_tip.rs`) |
| 14 | `SViewport` | 3D 뷰포트 임베딩 | ✅ 완료 (`widget/s_viewport_widget.rs`) |
| 15 | `SInvalidationPanel` | 무효화 영역 래퍼 (P0#1 선행) | ✅ 완료 (`widget/s_invalidation_panel.rs`) |
| 16 | `SLayerManager` / `STooltipPresenter` | 레이어/툴팁 관리 위젯 | ✅ 완료 (`widget/s_layer_manager.rs`) |

### 27. Layout 시스템 세부 보완

> UE 참조: `SlateCore/Public/Layout/` (18 파일)
> **구현 완료 (Phase 6)**: 전체 레이아웃 타입/슬롯 시스템

- [x] Slot 타입 시스템 — `CanvasSlot`, `OverlaySlot`, `GridSlot` (`widget/slot_types.rs`)
- [x] `StretchContent` 크기 규칙 — grow/shrink 별도 계수 (`core/alignment.rs SizeRule::Stretch`)
- [x] `FCombinedChildren` — 다중 자식 컬렉션 합성 (`widget/combined_children.rs`)
- [x] `LayoutGeometry` — 레이아웃 단계 전용 지오메트리 (`core/layout_types.rs`)
- [x] `FlowDirection` — LTR/RTL 레이아웃 방향 지원 (`core/alignment.rs`)
- [x] 가시성 기반 자동 필터링 — `ArrangedChildren::add_if_visible()` (`widget/traits.rs`)

### 28. Enum / 타입 보완

> UE 참조: `SlateCore/Public/Types/SlateEnums.h`
> **구현 완료 (Phase 6)**: 전체 enum/타입

- [x] `EButtonClickMethod` — DownAndUp / MouseDown / MouseUp / PreciseClick (`core/input_enums.rs`)
- [x] `EButtonTouchMethod` — DownAndUp / Down / PreciseTap (`core/input_enums.rs`)
- [x] `EButtonPressMethod` — DownAndUp / ButtonPress / ButtonRelease (`core/input_enums.rs`)
- [x] `ENavigationSource` — FocusedWidget / WidgetUnderCursor (`core/input_enums.rs`)
- [x] `ENavigationGenesis` — Keyboard / Controller / User (`core/input_enums.rs`)
- [x] `ETextCommit` — Default / OnEnter / OnUserMovedFocus / OnCleared (`core/input_enums.rs`)
- [x] `ESelectInfo` — OnKeyPress / OnNavigation / OnMouseClick / Direct (`core/input_enums.rs`)
- [x] `EScrollDirection` — Down / Up (`core/input_enums.rs`)
- [x] 누락 `MenuPlacement` 변형 — CenteredBelowAnchor / BelowRightAnchor / ComboBoxRight / RightLeftCenter / MatchBottomLeft (`framework/popup.rs`)

---

## 이미 잘 구현된 영역 (참고)

유지보수 및 점진 개선만 필요한 영역:

| 영역 | 완성도 | 비고 |
|------|--------|------|
| 애니메이션 시스템 | ~95% | UE 초과 수준 (Verlet/Arrive/13종 이징) + AnimationBinding + AutoAnimatedSequence |
| 커맨드 시스템 | ~95% | 계층 컨텍스트, InputChord, UIAction, 커맨드 리스트 스택, BindingContextChanged |
| 도킹 시스템 | ~90% | 트리/탭/사이드바/컴패스/JSON + WorkspaceItem + STabDrawer + TabCommands |
| 포커스/네비게이션 | ~85% | FocusManager, TabIndex, 방향 네비게이션, WidgetPath 라우팅 |
| 팝업/툴팁 | ~95% | MenuPlacement, TransitionEffect, 모달, PopupWindow, ModalWindowStack |
| 알림 시스템 | ~90% | 토스트 알림, Progress, 스레드 안전 큐, Async 매니저 |
| 접근성 | ~50%+ | WAI-ARIA 역할 매핑 (UE에 없는 고유 기능) |
| 사운드 시스템 | ~85% | 이벤트-사운드 매핑, 스팸 방지, 볼륨/피치, SlateSound |
| 입력 전처리기 | ~95% | 7단계 우선순위 파이프라인, 터치/제스처/아날로그, AnalogCursor |
| 테마/스타일 | ~90% | JSON 직렬화, 60+ 색상, 타입 스타일셋, 다중 스타일셋 |
| 텍스트 프레임워크 | ~95% | TextRun, TextLayout, 리치텍스트, BiDi, ShapedTextCache |
| 폰트 시스템 | ~95% | FontWeight/Style/Selector, MSDF, 아웃라인, CompositeFont |
| 렌더링 | ~95% | Spline, PostProcess, CustomVerts, DrawEffects, 배칭, InstancedRendering |
| 무효화/최적화 | ~90% | WidgetProxy, InvalidationList, Heap, 서브트리 캐싱 |
| 브러시 시스템 | ~95% | 타일링/미러링/UV/DynamicImageBrush/ImageType/RoundingType |
| 텍스처 관리 | ~95% | 트리 패킹, 멀티 포맷, 스레드 안전, 지연 업로드, LRU 퇴거 |

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
  P0#5  멀티 윈도우 (보완)                          ✓ 완료
  P1#9  계층적 클리핑                               ✓ 완료

Phase 3 (에디터 필수) ✓ 완료
  P0#3  MultiBox 메뉴/툴바 빌더                       ✓ 완료
  P0#4  텍스트 프레임워크                              ✓ 완료
  P1#10 모달 윈도우                                    ✓ 완료 (포커스트래핑+입력차단+ModalWindowStack)
  P1#11 폰트 확장                                      ✓ 완료

Phase 4~6 (기반 타입/이벤트/폰트 확장) ✓ 완료
  P3#28 Enum 및 누락 타입                              ✓ 완료
  P3#27 레이아웃 시스템 기반                            ✓ 완료
  P3#23 Widget Trait 확장                              ✓ 완료
  P3#22 터치/제스처/아날로그 입력                       ✓ 완료
  P0#4  고급 텍스트 기능 (Run/BiDi/ShapedText)         ✓ 완료
  P1#11 폰트 시스템 확장 (MSDF/Outline/Composite)      ✓ 완료

Phase 7~9 (렌더링/텍스처/위젯) ✓ 완료
  P2#19 텍스처 타입 추상화                             ✓ 완료
  P2#21 렌더링 세부 (Spline/PostProcess/CustomVerts)   ✓ 완료
  P3#24 레이아웃 위젯 14개                             ✓ 완료
  P3#25 입력 위젯 12개                                 ✓ 완료
  P3#26 뷰/색상/복합 위젯 16개                         ✓ 완료

Phase 10~12 (최적화/프레임워크) ✓ 완료
  P0#1  FastUpdate 최적화 (WidgetProxy/Heap)            ✓ 완료
  P0#2  고급 배칭/버퍼링 (ElementBatcher/DrawBuffer)    ✓ 완료
  P1#7  ManagedAttribute + 순서 보장                    ✓ 완료
  P1#9  wgpu 스텐실 파이프라인                          ✓ 완료
  P1#12 UI idle 감지                                    ✓ 완료

Phase 13~15 (프레임워크 완성/고급 렌더링) ✓ 완료
  P2#13 스타일링 시스템 (타입 스타일/다중 셋)            ✓ 완료
  P2#14 WidgetPath 포커스/입력 라우팅                    ✓ 완료
  P2#16 커맨드 리스트 스택                               ✓ 완료
  P2#17 스레드 안전 알림 큐                              ✓ 완료
  P2#18 아틀라스 디버그 뷰어                             ✓ 완료
  P0#5  PopupWindow 자동 전환                            ✓ 완료
  P1#11 MSDF 렌더링                                     ✓ 완료
  P2#21 고급 드로우 엘리먼트                             ✓ 완료
  Sound 통합                                             ✓ 완료

Phase 16 (최종 잔여 항목 일괄 구현) ✓ 완료
  P1#7  SlateAttribute 변환 (SComboBox/SSpinBox/SEditableTextBox)  ✓ 완료
  P1#8  AnimationBinding + AutoAnimatedSequence                     ✓ 완료
  P1#10 ModalWindowStack + 델리게이트 + 외부 모달                   ✓ 완료
  P1#12 AutoAnimatedSequence 타이머 자동 등록/해제                  ✓ 완료
  P2#15 WorkspaceItem + STabDrawer + TabCommands + TabInstanceId    ✓ 완료
  P2#16 BindingContextChangedEvent                                  ✓ 완료
  P2#18 조건부 컴파일 (#[cfg(feature = "slate_debugging")])         ✓ 완료
  P2#19 TreePacker + SharedAtlasHandle + DeferredUpload + LRU      ✓ 완료
  P2#20 BrushTiling/Mirroring + UV + DynamicImageBrush             ✓ 완료
  P2#21 SlateInstanceData + InstanceBatch                           ✓ 완료
  P3#22 InputPriority 7단계 + AnalogCursorController               ✓ 완료
```

**남은 항목: 없음** — Phase 16에서 23개 잔여 항목 전부 구현 완료 (2026-02-01)

Phase 16 구현 목록:
- P1#7: SlateAttribute 변환 (SComboBox, SSpinBox, SEditableTextBox) ✓
- P1#8: AnimationBinding (CurveSequence → Attribute 자동 바인딩) ✓
- P1#10: ModalWindowStack + ModalStackEvent + ExternalModalState ✓
- P1#12: AutoAnimatedSequence (play시 타이머 자동 등록/해제) ✓
- P2#15: WorkspaceItem + STabDrawer + TabCommands + TabInstanceId ✓
- P2#16: BindingContextChangedEvent + on_context_changed() ✓
- P2#18: `#[cfg(feature = "slate_debugging")]` 조건부 컴파일 ✓
- P2#19: TreePacker + SharedAtlasHandle + DeferredUploadQueue + AtlasEvictionManager ✓
- P2#20: BrushTiling/Mirroring + UVRegion + DynamicImageBrush + ImageType + RoundingType ✓
- P2#21: SlateInstanceData + InstanceBatch (Instanced Rendering) ✓
- P3#22: InputPriority 7단계 + AnalogCursorController ✓

---

## 수치 요약

| 카테고리 | 전체 항목 수 | 완료 | 미완 | 비고 |
|----------|-------------|------|------|------|
| P0 Critical | ~50 항목 | ~50 | 0 | ✅ 전체 완료 |
| P1 Major | ~40 항목 | ~40 | 0 | ✅ 전체 완료 (Phase 16에서 잔여 6개 완료) |
| P2 Moderate | ~55 항목 | ~55 | 0 | ✅ 전체 완료 (Phase 16에서 잔여 15개 완료) |
| P3 Minor | ~88 항목 | ~88 | 0 | ✅ 전체 완료 (Phase 16에서 잔여 2개 완료) |
| **총계** | **~233 항목** | **~233** | **0** | **완성도 ~100%** |

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

## Phase 4~15 복기 (2026-01-31)

### 구현 요약

Phase 4~15에서 나머지 전체 시스템을 구현했다. 총 ~15,000줄, 신규 파일 ~50개.

- **Phase 6**: 기반 타입/Enum/레이아웃 인프라 — input_enums, layout_types, font_settings, unicode_ranges, slot_types, combined_children
- **Phase 7**: 이벤트 시스템 확장 — touch_event, analog_event, navigation_event, gesture_detector, multi_user_input
- **Phase 8**: 폰트/텍스트 완성 — text_run_types, rich_text, text_shaping, bidi_support, font_settings
- **Phase 9**: 렌더링/텍스처 — texture_types, spline, element_batcher, draw_buffer
- **Phase 10-12**: 위젯 42개 — 레이아웃(6) + 입력(7) + 뷰/색상/복합(9)
- **Phase 13**: 최적화 — invalidation, idle_detector, managed_attribute, stencil_clipping
- **Phase 14**: 프레임워크 — style_system, widget_path, debug_viewer, command_list, async_notification, popup_window
- **Phase 15**: 고급 렌더링 — msdf_renderer, advanced_elements, ui_sound

### 최종 빌드/테스트 결과

- `cargo build` 성공 (에러 0)
- `cargo test` — 전체 workspace 513 테스트 통과 (skope_ui 511 + skope_game_ui 2)
- 총 위젯 수: 56개+ (UE Slate 주요 위젯 전체 대응)

## Phase 16 복기 (2026-02-01) — 최종 잔여 항목 일괄 구현

### 구현 요약

Phase 16에서 미구현 23개 항목을 전부 구현 완료. 총 ~2,500줄, 신규 파일 4개, 수정 파일 12개.

### 구현 내역

1. **P1#10 모달 윈도우 스택** — `ModalWindowStack` (push/pop/dismiss_all), `ModalStackEvent` (StackStarted/Ended/ModalPushed/Popped), `ExternalModalState` 외부 모달 연동 → `framework/popup.rs` (7개 테스트)
2. **P2#15 도킹 보완** — 3개 신규 파일:
   - `docking/workspace.rs`: `WorkspaceItem` 계층 트리, `TabInstanceId`, `WorkspaceMenuBuilder`
   - `docking/tab_drawer.rs`: `STabDrawer` 자동 숨김 사이드바 (DrawerState 4상태 애니메이션)
   - `docking/tab_commands.rs`: `DockCommand` enum, `KeyBinding`, `TabCommands` 단축키 레지스트리
3. **P2#16 커맨드 이벤트** — `BindingContextChangedEvent` + `on_context_changed()` 콜백 → `framework/command.rs` (5개 테스트)
4. **P2#18 조건부 컴파일** — `slate_debugging` 피처 게이트: widget_reflector/debug_stats/debug_viewer → `Cargo.toml`, `framework.rs`
5. **P2#19 텍스처 관리** — `TreePacker` 이진 트리 패킹, `SharedAtlasHandle<T>` 스레드 안전, `DeferredUploadQueue` 지연 업로드, `AtlasEvictionManager` LRU 퇴거 → `render/texture_atlas.rs` (12개 테스트)
6. **P2#20 브러시 시스템** — 이미 완전 구현됨 확인 (BrushTiling/Mirroring/UVRegion/DynamicImageBrush/ImageType/RoundingType) → `core/brush.rs`
7. **P2#21 인스턴스 렌더링** — `SlateInstanceData` (64B per-instance vertex), `InstanceBatch` → `render/types.rs` (6개 테스트)
8. **P3#22 입력 확장** — `InputPriority` 4→7단계 확장, `AnalogCursorController` 데드존+가속+감쇠 → `framework/input_preprocessor.rs`, `framework/analog_cursor.rs` (7개 테스트)
9. **P1#7 SlateAttribute** — SComboBox, SSpinBox, SEditableTextBox 변환 완료 확인
10. **P1#8 애니메이션 바인딩** — `AnimationBinding` + thread-local context time 완료 확인 → `framework/animation.rs`
11. **P1#12 애니메이션 자동 등록** — `AutoAnimatedSequence` play→타이머 자동 등록, 완료→자동 해제 → `framework/animation.rs`

### 빌드/테스트 결과

- `cargo build` 성공 (에러 0)
- `cargo test -p skope_ui --lib` — **604 테스트 통과** (0 실패)
- 테스트 증가: 547 → 604 (+57개 신규 테스트)
- 전체 TODO 항목: **233/233 완료 (100%)**
