//! Colours and geometry in logical pixels, from the core, and their
//! conversion to Frust's types (`peniko` colours, `kurbo` geometry).

pub use markdown_editor_core::geometry::{Color, Point, Rectangle, Size, Vector};

/// Conversion of a colour to Frust.
pub(crate) trait ToFrust {
    fn to_frust(self) -> frust::Color;
}

impl ToFrust for Color {
    fn to_frust(self) -> frust::Color {
        frust::Color::new([self.r, self.g, self.b, self.a])
    }
}

/// A colour of a Frust theme.
pub(crate) fn color_from_frust(color: frust::Color) -> Color {
    let [r, g, b, a] = color.components;
    Color { r, g, b, a }
}

/// Conversion of geometry to `kurbo`.
pub(crate) trait ToKurbo {
    type Output;

    fn to_kurbo(self) -> Self::Output;
}

impl ToKurbo for Point {
    type Output = frust::kurbo::Point;

    fn to_kurbo(self) -> frust::kurbo::Point {
        frust::kurbo::Point::new(self.x as f64, self.y as f64)
    }
}

impl ToKurbo for Size {
    type Output = frust::kurbo::Size;

    fn to_kurbo(self) -> frust::kurbo::Size {
        frust::kurbo::Size::new(self.width as f64, self.height as f64)
    }
}

impl ToKurbo for Rectangle {
    type Output = frust::kurbo::Rect;

    fn to_kurbo(self) -> frust::kurbo::Rect {
        frust::kurbo::Rect::new(
            self.x as f64,
            self.y as f64,
            (self.x + self.width) as f64,
            (self.y + self.height) as f64,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_round_trip_through_frust() {
        let color = Color::from_rgba(0.1, 0.2, 0.3, 0.4);
        assert_eq!(color_from_frust(color.to_frust()), color);
    }
}
