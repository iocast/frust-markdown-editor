//! Replays the core's display list onto Frust's `PaintScene`, skipping what
//! is not visible.

use crate::fonts;
use crate::geometry::{Rectangle, Size, ToFrust as _, ToKurbo as _};
use frust::authoring::PaintScene;
use frust::authoring::scene::{Glyph, GlyphRun};
use frust::kurbo::{Affine, BezPath};
use frust::peniko::{Blob, Brush, ImageAlphaType, ImageData, ImageFormat};
use markdown_editor_core::draw::{Image, PathSegment, TextRun, Transform};
use std::sync::Arc;

pub(crate) use markdown_editor_core::draw::{Frame, Op, Path, Stroke, Text};

/// An empty frame measuring text with the bundled fonts.
pub(crate) fn frame(size: Size) -> Frame {
    Frame::new(size, fonts::shared().clone())
}

/// Replays `ops` onto `scene`; operations outside `visible` are skipped.
pub(crate) fn replay(ops: &[Op], scene: &mut dyn PaintScene, visible: Option<Rectangle>) {
    for op in ops {
        if op.is_visible(visible) {
            draw(op, scene, visible);
        }
    }
}

fn draw(op: &Op, scene: &mut dyn PaintScene, visible: Option<Rectangle>) {
    match op {
        Op::Rect { rect, color } => {
            scene.fill_rect(
                rect.position().to_kurbo(),
                rect.size().to_kurbo(),
                color.to_frust(),
            );
        }
        Op::Fill { path, color, .. } => {
            scene.fill_path(
                frust::kurbo::Point::ZERO,
                &bez_path(path),
                &Brush::Solid(color.to_frust()),
            );
        }
        Op::Stroke {
            path, width, color, ..
        } => {
            scene.stroke_path(
                frust::kurbo::Point::ZERO,
                &bez_path(path),
                *width as f64,
                &Brush::Solid(color.to_frust()),
            );
        }
        Op::Text { run, .. } => scene.draw_glyph_run(glyph_run(run)),
        Op::Image { rect, image } => scene.draw_image(image_data(image), rect.to_kurbo()),
        Op::Transformed { transform, op } => {
            scene.push_transform(affine(*transform));
            draw(op, scene, None);
            scene.pop_transform();
        }
        Op::Clip {
            rect,
            transform,
            ops,
        } => {
            // The clip keeps the transform it was pushed with; the operations
            // inside carry their own.
            let transformed = *transform != Transform::IDENTITY;
            if transformed {
                scene.push_transform(affine(*transform));
            }
            scene.push_clip(rect.position().to_kurbo(), rect.size().to_kurbo());
            if transformed {
                scene.pop_transform();
            }
            replay(ops, scene, visible);
            scene.pop_clip();
        }
        Op::Group { ops, .. } => replay(ops, scene, None),
    }
}

fn affine(t: Transform) -> Affine {
    Affine::new([t.a, t.b, t.c, t.d, t.e, t.f].map(f64::from))
}

fn bez_path(path: &Path) -> BezPath {
    let mut bez = BezPath::new();
    for segment in path.segments() {
        match *segment {
            PathSegment::MoveTo(to) => bez.move_to(to.to_kurbo()),
            PathSegment::LineTo(to) => bez.line_to(to.to_kurbo()),
            PathSegment::QuadTo(control, to) => bez.quad_to(control.to_kurbo(), to.to_kurbo()),
            PathSegment::CubicTo(a, b, to) => {
                bez.curve_to(a.to_kurbo(), b.to_kurbo(), to.to_kurbo());
            }
            PathSegment::Close => bez.close_path(),
        }
    }
    bez
}

fn glyph_run(run: &TextRun) -> GlyphRun {
    GlyphRun {
        font: fonts::handle(run.face),
        font_size: run.size,
        brush: Brush::Solid(run.color.to_frust()),
        transform: Affine::IDENTITY,
        glyphs: run
            .glyphs
            .iter()
            .map(|glyph| Glyph {
                id: glyph.id,
                x: glyph.x,
                y: glyph.y,
            })
            .collect(),
    }
}

/// Frust's image of `image`, created once so its texture stays cached.
fn image_data(image: &Image) -> &Arc<ImageData> {
    image.handle(|image| {
        let pixels: Arc<dyn AsRef<[u8]> + Send + Sync> = image.pixels().clone();
        Arc::new(ImageData {
            data: Blob::new(pixels),
            format: ImageFormat::Rgba8,
            alpha_type: ImageAlphaType::Alpha,
            width: image.width(),
            height: image.height(),
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Color, Point, Vector};

    #[derive(Default)]
    struct Recorder {
        rects: usize,
        runs: usize,
        transforms: usize,
    }

    impl PaintScene for Recorder {
        fn fill_rect(&mut self, _: frust::kurbo::Point, _: frust::kurbo::Size, _: frust::Color) {
            self.rects += 1;
        }
        fn draw_text(&mut self, _: frust::kurbo::Point, _: &str) {}
        fn draw_glyph_run(&mut self, _: GlyphRun) {
            self.runs += 1;
        }
        fn push_transform(&mut self, _: Affine) {
            self.transforms += 1;
        }
    }

    #[test]
    fn culls_invisible_operations() {
        let mut frame = frame(Size::new(100.0, 1000.0));
        frame.fill_rectangle(Point::new(0.0, 0.0), Size::new(10.0, 10.0), Color::BLACK);
        frame.fill_rectangle(Point::new(0.0, 500.0), Size::new(10.0, 10.0), Color::BLACK);
        frame.fill_text(Text {
            content: "hello".into(),
            position: Point::new(0.0, 900.0),
            ..Text::default()
        });
        let mut all = Recorder::default();
        replay(frame.ops(), &mut all, None);
        assert_eq!((all.rects, all.runs), (2, 1));

        let mut top = Recorder::default();
        let visible = Rectangle::new(Point::ORIGIN, Size::new(100.0, 100.0));
        replay(frame.ops(), &mut top, Some(visible));
        assert_eq!((top.rects, top.runs), (1, 0));
    }

    #[test]
    fn transforms_are_saved_and_restored() {
        let mut frame = frame(Size::new(100.0, 100.0));
        frame.with_save(|frame| {
            frame.translate(Vector::new(5.0, 5.0));
            frame.fill_rectangle(Point::ORIGIN, Size::new(1.0, 1.0), Color::BLACK);
        });
        frame.fill_rectangle(Point::ORIGIN, Size::new(1.0, 1.0), Color::BLACK);
        let mut recorder = Recorder::default();
        replay(frame.ops(), &mut recorder, None);
        assert_eq!(recorder.transforms, 1);
        assert_eq!(recorder.rects, 2);
    }
}
