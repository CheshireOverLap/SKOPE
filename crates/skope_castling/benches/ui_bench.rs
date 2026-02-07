//! skope_castling 벤치마크
//!
//! 실행: cargo bench -p skope_castling

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

use skope_castling::prelude::*;
use skope_castling::DrawElementList;
use skope_castling::PaintGeometry;
use glam::Vec2;

/// 간단한 위젯 트리 생성 벤치마크
fn bench_widget_creation(c: &mut Criterion) {
    c.bench_function("create_text_block", |b| {
        b.iter(|| {
            black_box(
                STextBlock::new()
                    .text("Hello, World!")
                    .font_size(16.0)
                    .color(Color::rgba(1.0, 1.0, 1.0, 1.0))
                    .build()
            )
        })
    });

    c.bench_function("create_button", |b| {
        b.iter(|| {
            black_box(
                SButton::new()
                    .background_color(Color::rgba(0.2, 0.4, 0.8, 1.0))
                    .content(
                        STextBlock::new()
                            .text("Click")
                            .build()
                    )
                    .build()
            )
        })
    });

    c.bench_function("create_vbox_4_children", |b| {
        b.iter(|| {
            black_box(
                SVerticalBox::new()
                    .slot()
                        .auto_height()
                        .content(STextBlock::new().text("Item 1").build())
                    .slot()
                        .auto_height()
                        .content(STextBlock::new().text("Item 2").build())
                    .slot()
                        .auto_height()
                        .content(STextBlock::new().text("Item 3").build())
                    .slot()
                        .auto_height()
                        .content(STextBlock::new().text("Item 4").build())
                    .build()
            )
        })
    });
}

/// 레이아웃 계산 벤치마크
fn bench_layout_computation(c: &mut Criterion) {
    // 간단한 레이아웃
    let simple_widget = SVerticalBox::new()
        .slot()
            .auto_height()
            .content(STextBlock::new().text("Header").font_size(24.0).build())
        .slot()
            .fill_height()
            .content(STextBlock::new().text("Content").build())
        .slot()
            .auto_height()
            .content(STextBlock::new().text("Footer").build())
        .build();

    c.bench_function("layout_simple_vbox", |b| {
        b.iter(|| {
            black_box(simple_widget.compute_desired_size(1.0))
        })
    });

    // 복잡한 레이아웃 (중첩)
    let complex_widget = SVerticalBox::new()
        .slot()
            .auto_height()
            .padding(Margin::uniform(10.0))
            .content(
                SHorizontalBox::new()
                    .slot()
                        .auto_width()
                        .content(STextBlock::new().text("Left").build())
                    .slot()
                        .fill_width()
                        .content(STextBlock::new().text("Center").build())
                    .slot()
                        .auto_width()
                        .content(STextBlock::new().text("Right").build())
                    .build()
            )
        .slot()
            .fill_height()
            .content(
                SBox::new()
                    .padding(Margin::uniform(20.0))
                    .content(
                        SButton::new()
                            .content(STextBlock::new().text("Button").build())
                            .build()
                    )
                    .build()
            )
        .build();

    c.bench_function("layout_complex_nested", |b| {
        b.iter(|| {
            black_box(complex_widget.compute_desired_size(1.0))
        })
    });
}

/// 다양한 자식 수에 따른 스케일링 벤치마크
fn bench_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("vbox_scaling");

    for n in [10, 50, 100, 500].iter() {
        group.bench_with_input(BenchmarkId::new("children", n), n, |b, &n| {
            let mut builder = SVerticalBox::new();
            for i in 0..n {
                builder = builder
                    .slot()
                    .auto_height()
                    .content(STextBlock::new().text(format!("Item {}", i)).build());
            }
            let widget = builder.build();

            b.iter(|| {
                black_box(widget.compute_desired_size(1.0))
            })
        });
    }

    group.finish();
}

/// Geometry 연산 벤치마크
fn bench_geometry(c: &mut Criterion) {
    let root_geo = Geometry::make_root(Vec2::new(1920.0, 1080.0), 1.0);

    c.bench_function("geometry_make_child", |b| {
        b.iter(|| {
            black_box(root_geo.make_child(Vec2::new(100.0, 100.0), Vec2::new(400.0, 300.0)))
        })
    });

    c.bench_function("geometry_contains_point", |b| {
        let child_geo = root_geo.make_child(Vec2::new(100.0, 100.0), Vec2::new(400.0, 300.0));
        let point = Vec2::new(250.0, 250.0);
        b.iter(|| {
            black_box(child_geo.contains_absolute(point))
        })
    });
}

/// DrawElementList 벤치마크
fn bench_draw_elements(c: &mut Criterion) {
    c.bench_function("draw_list_add_100_boxes", |b| {
        b.iter(|| {
            let mut list = DrawElementList::new();
            let geo = PaintGeometry {
                position: Vec2::ZERO,
                size: Vec2::new(100.0, 50.0),
                scale: 1.0,
            };
            for i in 0..100 {
                list.add_box(i, geo, Color::rgba(0.5, 0.5, 0.5, 1.0));
            }
            black_box(list)
        })
    });
}

criterion_group!(
    benches,
    bench_widget_creation,
    bench_layout_computation,
    bench_scaling,
    bench_geometry,
    bench_draw_elements,
);
criterion_main!(benches);
