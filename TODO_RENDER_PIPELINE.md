# SKOPE Render Pipeline - Issue Tracker

## Status Legend
- [ ] TODO
- [x] DONE
- [~] IN PROGRESS

---

## 1. Correctness Bugs

### 1-1. Triangle ID encoding mismatch
- **Files**: `src/renderer/vbuffer.rs` (CPU) vs `visibility.wgsl` (GPU)
- **Issue**: CPU uses 16|16 (mesh|prim), GPU uses 8|8|16 (mesh|material|prim)
- **Status**: [ ]

### 1-2. Normals remain in local space
- **Files**: `src/shaders/material_eval.wgsl`
- **Issue**: Interpolated normals never transformed by world matrix
- **Status**: [ ]

### 1-3. SMRT shader page table decoding error
- **Files**: VSM SMRT shader
- **Issue**: Uses 16-bit masks instead of actual 5-bit encoding
- **Status**: [ ]

### 1-4. VSM clipmap texel_size off by page_size factor
- **Files**: `crates/skope_lighting/src/vsm/`
- **Issue**: Causes shadow swimming
- **Status**: [ ]

### 1-5. Vertex stride mismatch (Shadow Atlas + VSM)
- **Files**: shadow atlas pipeline, VSM pipeline
- **Issue**: Shadow atlas=48B, VSM=64B, actual GpuVertex=80B
- **Status**: [ ]

### 1-6. DDGI active_cascade hardcoded to 0
- **Files**: `src/renderer/ddgi/pipeline.rs:560`
- **Issue**: Medium/Far cascades never receive ray trace updates
- **Status**: [ ]

### 1-7. Nanite streaming overflow check wrong units
- **Files**: `crates/skope_virtual_geometry/src/streaming.rs:358`
- **Issue**: Compares cluster count vs page capacity (incompatible units)
- **Status**: [ ]

### 1-8. Radiance cache aliasing violation
- **Files**: `crates/skope_bishop/src/radiance_cache.rs:1163-1169`
- **Issue**: Same indirection_view bound as WriteOnly storage + read texture simultaneously
- **Status**: [ ]

### 1-9. Surface cache dirty page range error
- **Files**: `crates/skope_bishop/src/surface_cache.rs:510`
- **Issue**: Returns contiguous range from dirty_pages[0] but pages are non-contiguous
- **Status**: [ ]

### 1-10. Motion vector jitter correction not applied
- **Files**: `src/shaders/motion_vectors.wgsl:88-89`
- **Issue**: jitter_correction computed but never added to velocity
- **Status**: [ ]

### 1-11. SSR history buffer never written
- **Files**: `src/renderer/ssr.rs:515`
- **Issue**: Temporal resolve reads uninitialized GPU memory
- **Status**: [ ]

### 1-12. Volumetric fog temporal blend unit mismatch
- **Files**: `src/shaders/volumetric_scatter.wgsl:109-110`
- **Issue**: Mixes per-slice transmittance with cumulative extinction (physically incompatible)
- **Status**: [ ]

### 1-13. TSR history depth resolution mismatch
- **Files**: `crates/skope_endgame/src/tsr.rs`
- **Issue**: Textures created at output resolution but written at internal resolution
- **Status**: [ ]

### 1-14. Bloom threshold reads only top-left quarter
- **Files**: `crates/skope_endgame/src/bloom.rs`
- **Issue**: No proper downsampling, reads 1/4 of HDR image
- **Status**: [ ]

---

## 2. Performance Issues

### 2-1. Per-frame GPU buffer allocation
- **Files**: `src/app/state/render.rs:433-476, 682-711`
- **Issue**: N meshes x 4 buffers + N bind groups created via create_buffer_init every frame
- **Fix**: PerViewBufferPool — persistent buffers with queue.write_buffer()
- **Status**: [x] DONE

### 2-2. Per-dirty-instance write_buffer
- **Files**: `src/renderer/gpu_scene.rs:406-417`
- **Issue**: Individual 192B writes instead of batched staging buffer
- **Status**: [ ]

### 2-3. Shadow atlas full reset every frame
- **Files**: `src/renderer/shadow_atlas.rs`
- **Issue**: LRU exists but all allocations reset each frame, re-rendering static shadows
- **Status**: [ ]

### 2-4. SW rasterizer per-frame CPU clear
- **Files**: Nanite SW rasterizer
- **Issue**: CPU alloc+upload width*height u32s instead of GPU compute clear
- **Status**: [ ]

### 2-5. GTAO/Volumetric textures missing COPY_SRC/COPY_DST
- **Files**: `src/renderer/gtao.rs:182-197`, `src/renderer/volumetric.rs:230-245`
- **Issue**: Temporal copy_texture_to_texture will fail wgpu validation
- **Status**: [ ]

### 2-6. Bind groups recreated every frame (VBuffer resolve)
- **Files**: `src/renderer/vbuffer_resolve.rs`
- **Issue**: Textures only change on resize, bind groups should be cached
- **Status**: [ ]

### 2-7. Barycentric format waste
- **Files**: `src/renderer/vbuffer_resolve.rs`
- **Issue**: Rg16Float (4B/px) upgraded to Rgba16Float (8B/px), extra channels always 0
- **Status**: [ ]

### 2-8. Instance culling Pass 1 iterates all instances
- **Files**: `src/renderer/instance_culling.rs`
- **Issue**: Should only re-test occluded instances, not full set
- **Status**: [ ]

### 2-9. Debug mode does full lighting then discards
- **Files**: `src/shaders/material_eval.wgsl`
- **Issue**: Early return before lighting would save GPU cost in debug views
- **Status**: [ ]

### 2-10. Clustered lighting CPU cull skips X/Y bounds
- **Files**: `crates/skope_lighting/src/clustered.rs`
- **Issue**: Assigns lights to all clusters in depth slice instead of proper frustum test
- **Status**: [ ]

---

## 3. Dead Code

### 3-1. OIT build pass never dispatched
- **Files**: `src/renderer/oit.rs`
- **Status**: [ ]

### 3-2. Endgame TAA has no execute method
- **Files**: `crates/skope_endgame/src/taa.rs`
- **Status**: [ ]

### 3-3. TSR 3 phases compute data never read
- **Files**: `crates/skope_endgame/src/tsr.rs`
- **Issue**: thin geometry, rejection mask, flicker map unused by subsequent passes
- **Status**: [ ]

### 3-4. SSS blur / TSR not integrated into PostProcessPipeline
- **Files**: `crates/skope_endgame/`
- **Status**: [ ]

### 3-5. Dead RenderSettings flags (enable_bloom, exposure, enable_gpu_profiler)
- **Files**: `src/renderer/` types
- **Status**: [ ]

### 3-6. Exposure hardcoded 0.0 (ignores settings.exposure)
- **Files**: `src/renderer.rs:3196`
- **Status**: [ ]

### 3-7. vbuffer_sampler, shadow_sampler never used in shader
- **Files**: `src/shaders/material_eval.wgsl`
- **Status**: [ ]

### 3-8. nanite_instances buffer bound but never read
- **Files**: `src/shaders/material_eval.wgsl:58`
- **Status**: [ ]

### 3-9. Emissive properties declared but never evaluated
- **Files**: `src/shaders/material_eval.wgsl`
- **Status**: [ ]

### 3-10. VisibleLightHash 200+ lines never used
- **Files**: MegaLights
- **Status**: [ ]

### 3-11. MegaLights tile classification result ignored
- **Files**: MegaLights sampling shader
- **Status**: [ ]

### 3-12. prev_reservoir_buffer ~32MB allocated, never read
- **Files**: MegaLights
- **Status**: [ ]

### 3-13. VSM cache manager fully implemented but disconnected
- **Files**: `crates/skope_lighting/src/vsm/`
- **Status**: [ ]

### 3-14. GPU cluster cull shader is no-op placeholder
- **Files**: `crates/skope_lighting/src/clustered.rs`
- **Status**: [ ]

### 3-15. IBL prefilter compute never dispatched
- **Files**: `crates/skope_lighting/src/ibl.rs`
- **Issue**: from_equirectangular() skips prefilter → mip 1+ and irradiance cubemap black
- **Status**: [ ]

### 3-16. Virtual Textures entire crate not integrated
- **Files**: `crates/skope_promotion/`
- **Status**: [ ]

### 3-17. DDGI update_layout_1 dead field
- **Files**: DDGI pipeline
- **Status**: [ ]

### 3-18. SSR importance sampling functions unused
- **Files**: `src/renderer/ssr.rs`
- **Status**: [ ]

### 3-19. VRS stats never computed, instance culling readback unused
- **Files**: `src/renderer/vrs.rs`, `src/renderer/instance_culling.rs`
- **Status**: [ ]

### 3-20. _selected_entity dead field
- **Files**: `src/renderer.rs:1063`
- **Status**: [ ]

### 3-21. Bloom composite_pipeline, DoF blur_pipeline, tonemap_hejl_bd dead
- **Files**: `crates/skope_endgame/`
- **Status**: [ ]

---

## 4. Design / Hardcoding Issues

### 4-1. Game View overwrites shared renderer state
- **Files**: `src/app/state/render.rs:745-755`
- **Issue**: Lighting uniforms and MegaLights corrupted for scene view temporal effects
- **Status**: [ ]

### 4-2. static mut unsafe globals (FRAME_COUNT, FIRST_FRAME)
- **Files**: `src/app/state/render.rs:62-65`
- **Issue**: Should use AtomicU32 (same pattern already used elsewhere in file)
- **Status**: [x] DONE — AtomicU32 + std::sync::Once로 교체, unsafe 제거

### 4-3. MOVABLE flag never set
- **Files**: `src/renderer/gpu_scene.rs:449` vs `src/app/state/render.rs:592`
- **Issue**: begin_frame() is no-op, stale motion vectors for stopped objects
- **Status**: [ ]

### 4-4. bounds_radius not updated in update_transform
- **Files**: `src/renderer/gpu_scene.rs:325-337`
- **Issue**: Scale changes not reflected in bounding sphere → culling errors
- **Status**: [ ]

### 4-5. set_bloom_enabled overwrites all tonemap params
- **Files**: `crates/skope_endgame/`
- **Status**: [ ]

### 4-6. Auto exposure uses hardcoded dt=0.016
- **Files**: `crates/skope_endgame/src/auto_exposure.rs`
- **Status**: [ ]

### 4-7. PostProcessConfig / DebugView duplicate definitions
- **Files**: `crates/skope_endgame/src/lib.rs` vs `pipeline.rs`
- **Status**: [ ]

### 4-8. new_with_depth_equal duplicates ~120 lines
- **Files**: `src/renderer/vbuffer.rs`
- **Status**: [ ]

### 4-9. Mesh shader camera uniform visibility comment mismatch
- **Files**: Nanite mesh shader
- **Status**: [ ]
