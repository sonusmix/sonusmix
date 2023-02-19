use csscolorparser::Color;

pub trait CssColorExt {
    fn mult_alpha(&self, alpha: f64) -> Self;
}

impl CssColorExt for Color {
    fn mult_alpha(&self, alpha: f64) -> Self {
        let mut new_color = self.clone();
        new_color.a *= alpha;
        new_color
    }
}