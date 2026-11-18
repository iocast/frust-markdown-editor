//! A recorded display list: the editor (and [`BlockRenderer`]s) draw into a
//! [`Frame`]; the widget replays it onto Frust's `PaintScene`, skipping what
//! is not visible.
//!
//! [`BlockRenderer`]: crate::BlockRenderer

use crate::fonts::{self, Face};
use crate::geometry::{Color, Point, Rectangle, Size, Vector};
use frust::authoring::scene::GlyphRun;
use frust::authoring::{PaintScene, Shape as _};
use frust::kurbo::{Affine, BezPath};
use frust::peniko::{Brush, ImageData};
use std::sync::Arc;

/// One recorded drawing operation. Bounds (in the frame's coordinates) are
/// `None` when unknown, e.g. under a transform; such operations are always
/// replayed.
// Glyph runs are large, but boxing them costs an allocation per run.
#[allow(clippy::large_enum_variant)]
#[derive(Clone)]
pub(crate) enum Op {
    Rect {
        rect: Rectangle,
        color: Color,
    },
    Fill {
        path: BezPath,
        color: Color,
        bounds: Option<Rectangle>,
    },
    Stroke {
        path: BezPath,
        width: f32,
        color: Color,
        bounds: Option<Rectangle>,
    },
    Text {
        run: GlyphRun,
        bounds: Option<Rectangle>,
    },
    Image {
        rect: Rectangle,
        data: Arc<ImageData>,
    },
    PushTransform(Affine),
    PopTransform,
    PushClip(Rectangle),
    PopClip,
    /// Operations culled together by their common bounds.
    Group {
        bounds: Rectangle,
        ops: Vec<Op>,
    },
}

impl Op {
    fn bounds(&self) -> Option<Rectangle> {
        match self {
            Op::Rect { rect, .. } | Op::Image { rect, .. } => Some(*rect),
            Op::Fill { bounds, .. } | Op::Stroke { bounds, .. } | Op::Text { bounds, .. } => {
                *bounds
            }
            Op::Group { bounds, .. } => Some(*bounds),
            Op::PushTransform(_) | Op::PopTransform | Op::PushClip(_) | Op::PopClip => None,
        }
    }
}

/// Replays `ops` onto `scene`; operations outside `visible` are skipped.
pub(crate) fn replay(ops: &[Op], scene: &mut dyn PaintScene, visible: Option<Rectangle>) {
    for op in ops {
        if let (Some(visible), Some(bounds)) = (visible, op.bounds())
            && !visible.intersects(&bounds.expand(1.0))
        {
            continue;
        }
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
                    path,
                    &Brush::Solid(color.to_frust()),
                );
            }
            Op::Stroke {
                path, width, color, ..
            } => {
                scene.stroke_path(
                    frust::kurbo::Point::ZERO,
                    path,
                    *width as f64,
                    &Brush::Solid(color.to_frust()),
                );
            }
            Op::Text { run, .. } => scene.draw_glyph_run(run.clone()),
            Op::Image { rect, data } => scene.draw_image(data, rect.to_kurbo()),
            Op::PushTransform(transform) => scene.push_transform(*transform),
            Op::PopTransform => scene.pop_transform(),
            Op::PushClip(rect) => {
                scene.push_clip(rect.position().to_kurbo(), rect.size().to_kurbo());
            }
            Op::PopClip => scene.pop_clip(),
            Op::Group { ops, .. } => replay(ops, scene, None),
        }
    }
}

/// The face of text drawn with [`Frame::fill_text`]: the bundled DejaVu
/// fonts.
pub type Font = Face;

/// Horizontal alignment of [`Text`] relative to its position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// Vertical alignment of [`Text`]'s line box relative to its position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextBaseline {
    /// The position is the top of the line box.
    #[default]
    Top,
    Middle,
    Bottom,
}

/// A line of text for [`Frame::fill_text`].
#[derive(Debug, Clone, PartialEq)]
pub struct Text {
    pub content: String,
    /// Top-left of the line box (see [`Text::align_x`] and [`Text::align_y`]).
    pub position: Point,
    pub color: Color,
    /// Font size in pixels.
    pub size: f32,
    /// Height of the line box; `None` = 1.3 × size.
    pub line_height: Option<f32>,
    pub font: Font,
    pub align_x: TextAlign,
    pub align_y: TextBaseline,
}

impl Default for Text {
    fn default() -> Self {
        Self {
            content: String::new(),
            position: Point::ORIGIN,
            color: Color::BLACK,
            size: 16.0,
            line_height: None,
            font: Font::default(),
            align_x: TextAlign::Left,
            align_y: TextBaseline::Top,
        }
    }
}

impl From<&str> for Text {
    fn from(content: &str) -> Self {
        Text {
            content: content.to_string(),
            ..Text::default()
        }
    }
}

impl From<String> for Text {
    fn from(content: String) -> Self {
        Text {
            content,
            ..Text::default()
        }
    }
}

/// How a [`Path`] is outlined.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stroke {
    pub width: f32,
    pub color: Color,
}

impl Default for Stroke {
    fn default() -> Self {
        Self {
            width: 1.0,
            color: Color::BLACK,
        }
    }
}

impl Stroke {
    pub fn with_width(self, width: f32) -> Self {
        Stroke { width, ..self }
    }

    pub fn with_color(self, color: Color) -> Self {
        Stroke { color, ..self }
    }
}

/// A vector shape.
#[derive(Debug, Clone, PartialEq)]
pub struct Path(BezPath);

impl Path {
    /// Builds a path with a [`PathBuilder`].
    pub fn new(build: impl FnOnce(&mut PathBuilder)) -> Path {
        let mut builder = PathBuilder(BezPath::new());
        build(&mut builder);
        Path(builder.0)
    }

    pub fn line(from: Point, to: Point) -> Path {
        Path::new(|path| {
            path.move_to(from);
            path.line_to(to);
        })
    }

    pub fn rectangle(top_left: Point, size: Size) -> Path {
        Path::new(|path| path.rectangle(top_left, size))
    }

    pub fn rounded_rectangle(top_left: Point, size: Size, radius: f32) -> Path {
        let rect = Rectangle::new(top_left, size).to_kurbo();
        Path(frust::kurbo::RoundedRect::from_rect(rect, radius as f64).to_path(0.1))
    }

    pub fn circle(center: Point, radius: f32) -> Path {
        Path::new(|path| path.circle(center, radius))
    }

    fn bounds(&self) -> Rectangle {
        let rect = self.0.bounding_box();
        Rectangle {
            x: rect.x0 as f32,
            y: rect.y0 as f32,
            width: rect.width() as f32,
            height: rect.height() as f32,
        }
    }
}

/// Builds a [`Path`].
pub struct PathBuilder(BezPath);

impl PathBuilder {
    pub fn move_to(&mut self, point: Point) {
        self.0.move_to(point.to_kurbo());
    }

    pub fn line_to(&mut self, point: Point) {
        self.0.line_to(point.to_kurbo());
    }

    pub fn quadratic_curve_to(&mut self, control: Point, to: Point) {
        self.0.quad_to(control.to_kurbo(), to.to_kurbo());
    }

    pub fn bezier_curve_to(&mut self, control_a: Point, control_b: Point, to: Point) {
        self.0
            .curve_to(control_a.to_kurbo(), control_b.to_kurbo(), to.to_kurbo());
    }

    pub fn rectangle(&mut self, top_left: Point, size: Size) {
        self.move_to(top_left);
        self.line_to(Point::new(top_left.x + size.width, top_left.y));
        self.line_to(Point::new(
            top_left.x + size.width,
            top_left.y + size.height,
        ));
        self.line_to(Point::new(top_left.x, top_left.y + size.height));
        self.close();
    }

    pub fn circle(&mut self, center: Point, radius: f32) {
        let circle = frust::kurbo::Circle::new(center.to_kurbo(), radius as f64);
        self.0.extend(circle.path_elements(0.1));
    }

    pub fn close(&mut self) {
        self.0.close_path();
    }
}

/// Decoded pixels for [`Frame::draw_image`].
#[derive(Debug, Clone)]
pub struct Image {
    pub(crate) data: Arc<ImageData>,
}

impl Image {
    /// Straight-alpha RGBA8 pixels, `width * height * 4` bytes.
    pub fn from_rgba(width: u32, height: u32, pixels: Vec<u8>) -> Image {
        Image {
            data: Arc::new(ImageData {
                data: frust::peniko::Blob::from(pixels),
                format: frust::peniko::ImageFormat::Rgba8,
                alpha_type: frust::peniko::ImageAlphaType::Alpha,
                width,
                height,
            }),
        }
    }

    pub fn width(&self) -> u32 {
        self.data.width
    }

    pub fn height(&self) -> u32 {
        self.data.height
    }
}

/// A drawing surface that records what is drawn.
///
/// Block renderers receive one in [`BlockRenderer::draw`]; coordinates are
/// document coordinates (the bounds passed to `draw`).
///
/// [`BlockRenderer::draw`]: crate::BlockRenderer::draw
pub struct Frame {
    size: Size,
    ops: Vec<Op>,
    transform: Affine,
    saved: Vec<Affine>,
}

impl Frame {
    /// An empty frame of the given size.
    pub fn new(size: Size) -> Frame {
        Frame {
            size,
            ops: Vec::new(),
            transform: Affine::IDENTITY,
            saved: Vec::new(),
        }
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn width(&self) -> f32 {
        self.size.width
    }

    pub fn height(&self) -> f32 {
        self.size.height
    }

    pub fn center(&self) -> Point {
        Point::new(self.size.width / 2.0, self.size.height / 2.0)
    }

    fn transformed(&self) -> bool {
        self.transform != Affine::IDENTITY
    }

    /// Records `op`, wrapped in the current transform if there is one.
    fn push(&mut self, op: Op) {
        if self.transformed() {
            self.ops.push(Op::PushTransform(self.transform));
            self.ops.push(op);
            self.ops.push(Op::PopTransform);
        } else {
            self.ops.push(op);
        }
    }

    /// Bounds for culling, unless a transform makes them unknown.
    fn known(&self, bounds: Rectangle) -> Option<Rectangle> {
        (!self.transformed()).then_some(bounds)
    }

    pub fn fill_rectangle(&mut self, top_left: Point, size: Size, color: impl Into<Color>) {
        if size.width <= 0.0 || size.height <= 0.0 {
            return;
        }
        self.push(Op::Rect {
            rect: Rectangle::new(top_left, size),
            color: color.into(),
        });
    }

    pub fn fill(&mut self, path: &Path, color: impl Into<Color>) {
        let bounds = self.known(path.bounds());
        self.push(Op::Fill {
            path: path.0.clone(),
            color: color.into(),
            bounds,
        });
    }

    pub fn stroke(&mut self, path: &Path, stroke: impl Into<Stroke>) {
        let stroke = stroke.into();
        let bounds = self.known(path.bounds().expand(stroke.width));
        self.push(Op::Stroke {
            path: path.0.clone(),
            width: stroke.width,
            color: stroke.color,
            bounds,
        });
    }

    pub fn stroke_rectangle(&mut self, top_left: Point, size: Size, stroke: impl Into<Stroke>) {
        self.stroke(&Path::rectangle(top_left, size), stroke);
    }

    /// Width of `content` in `font` at `size`.
    pub fn measure_text(&self, content: &str, font: Font, size: f32) -> f32 {
        fonts::shared().width(font, content, size)
    }

    /// Draws one line of text (no wrapping).
    pub fn fill_text(&mut self, text: impl Into<Text>) {
        let text = text.into();
        if text.content.is_empty() {
            return;
        }
        let line_height = text.line_height.unwrap_or(text.size * 1.3);
        let width = self.measure_text(&text.content, text.font, text.size);
        let x = match text.align_x {
            TextAlign::Left => text.position.x,
            TextAlign::Center => text.position.x - width / 2.0,
            TextAlign::Right => text.position.x - width,
        };
        let top = match text.align_y {
            TextBaseline::Top => text.position.y,
            TextBaseline::Middle => text.position.y - line_height / 2.0,
            TextBaseline::Bottom => text.position.y - line_height,
        };
        let baseline = top + fonts::baseline_offset(text.font, text.size, line_height);
        let run = fonts::run(&text.content, text.font, text.size, x, baseline, text.color);
        let bounds = self.known(Rectangle::new(
            Point::new(x, top),
            Size::new(width, line_height),
        ));
        self.push(Op::Text { run, bounds });
    }

    /// Draws `image` scaled into `bounds`.
    pub fn draw_image(&mut self, bounds: Rectangle, image: &Image) {
        self.draw_image_data(bounds, image.data.clone());
    }

    pub(crate) fn draw_image_data(&mut self, bounds: Rectangle, data: Arc<ImageData>) {
        self.push(Op::Image { rect: bounds, data });
    }

    /// Glyphs placed by the caller (document text).
    pub(crate) fn draw_run(&mut self, run: GlyphRun, bounds: Rectangle) {
        let bounds = self.known(bounds);
        self.push(Op::Text { run, bounds });
    }

    /// Moves the origin of later drawing.
    pub fn translate(&mut self, translation: Vector) {
        self.transform *= Affine::translate((translation.x as f64, translation.y as f64));
    }

    /// Rotates later drawing by `angle` radians around the origin.
    pub fn rotate(&mut self, angle: f32) {
        self.transform *= Affine::rotate(angle as f64);
    }

    /// Scales later drawing uniformly.
    pub fn scale(&mut self, scale: f32) {
        self.transform *= Affine::scale(scale as f64);
    }

    pub fn scale_nonuniform(&mut self, scale: Vector) {
        self.transform *= Affine::scale_non_uniform(scale.x as f64, scale.y as f64);
    }

    /// Runs `draw`; transforms made inside are undone afterwards.
    pub fn with_save<R>(&mut self, draw: impl FnOnce(&mut Frame) -> R) -> R {
        self.saved.push(self.transform);
        let result = draw(self);
        self.transform = self.saved.pop().unwrap_or(Affine::IDENTITY);
        result
    }

    /// Runs `draw` clipped to `region`.
    pub fn with_clip<R>(&mut self, region: Rectangle, draw: impl FnOnce(&mut Frame) -> R) -> R {
        self.push_raw(Op::PushClip(region));
        let result = draw(self);
        self.ops.push(Op::PopClip);
        result
    }

    fn push_raw(&mut self, op: Op) {
        if self.transformed() {
            self.ops.push(Op::PushTransform(self.transform));
            self.ops.push(op);
            // The clip keeps the transform it was pushed with.
            self.ops.push(Op::PopTransform);
        } else {
            self.ops.push(op);
        }
    }

    /// Records what `draw` draws as one group, culled by `bounds`.
    pub(crate) fn group(&mut self, bounds: Rectangle, draw: impl FnOnce(&mut Frame)) {
        let mut inner = Frame::new(self.size);
        inner.transform = self.transform;
        draw(&mut inner);
        if !inner.ops.is_empty() {
            self.ops.push(Op::Group {
                bounds,
                ops: inner.ops,
            });
        }
    }

    pub(crate) fn into_ops(self) -> Vec<Op> {
        self.ops
    }

    /// Replays the recording onto `scene`.
    pub(crate) fn replay(&self, scene: &mut dyn PaintScene, visible: Option<Rectangle>) {
        replay(&self.ops, scene, visible);
    }

    #[cfg(test)]
    pub(crate) fn ops(&self) -> &[Op] {
        &self.ops
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let mut frame = Frame::new(Size::new(100.0, 1000.0));
        frame.fill_rectangle(Point::new(0.0, 0.0), Size::new(10.0, 10.0), Color::BLACK);
        frame.fill_rectangle(Point::new(0.0, 500.0), Size::new(10.0, 10.0), Color::BLACK);
        frame.fill_text(Text {
            content: "hello".into(),
            position: Point::new(0.0, 900.0),
            ..Text::default()
        });
        let mut all = Recorder::default();
        frame.replay(&mut all, None);
        assert_eq!((all.rects, all.runs), (2, 1));

        let mut top = Recorder::default();
        let visible = Rectangle::new(Point::ORIGIN, Size::new(100.0, 100.0));
        frame.replay(&mut top, Some(visible));
        assert_eq!((top.rects, top.runs), (1, 0));
    }

    #[test]
    fn transforms_are_saved_and_restored() {
        let mut frame = Frame::new(Size::new(100.0, 100.0));
        frame.with_save(|frame| {
            frame.translate(Vector::new(5.0, 5.0));
            frame.fill_rectangle(Point::ORIGIN, Size::new(1.0, 1.0), Color::BLACK);
        });
        frame.fill_rectangle(Point::ORIGIN, Size::new(1.0, 1.0), Color::BLACK);
        assert_eq!(frame.ops().len(), 4);
        let mut recorder = Recorder::default();
        frame.replay(&mut recorder, None);
        assert_eq!(recorder.transforms, 1);
        assert_eq!(recorder.rects, 2);
    }
}
