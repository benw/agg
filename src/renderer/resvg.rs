use super::{color_to_rgb, text_attrs, Renderer, Settings, TextAttrs};
use crate::theme::Theme;
use imgref::ImgVec;
use rgb::{FromSlice, RGBA8};
use std::fmt::Write;
use tiny_skia::Pixmap;

pub struct ResvgRenderer<'a> {
    settings: Settings,
    char_width: f64,
    row_height: f64,
    options: usvg::Options<'a>,
    transform: tiny_skia::Transform,
}

fn color_to_style(color: &avt::Color, theme: &Theme) -> String {
    let c = color_to_rgb(color, theme);

    format!("fill: rgb({},{},{})", c.r, c.g, c.b)
}

fn text_class(attrs: &TextAttrs) -> String {
    let mut class = "".to_owned();

    if attrs.bold {
        class.push_str("br");
    }

    if attrs.italic {
        class.push_str(" it");
    }

    if attrs.underline {
        class.push_str(" un");
    }

    class
}

fn text_style(attrs: &TextAttrs, theme: &Theme) -> String {
    attrs
        .foreground
        .map(|c| color_to_style(&c, theme))
        .unwrap_or_else(|| "".to_owned())
}

fn rect_style(attrs: &TextAttrs, theme: &Theme) -> String {
    attrs
        .background
        .map(|c| color_to_style(&c, theme))
        .unwrap_or_else(|| "".to_owned())
}

impl<'a> ResvgRenderer<'a> {
    pub fn new(settings: Settings) -> Self {
        let font_size = settings.font_size as f64;
        let row_height = font_size * settings.line_height;
        let char_width = font_size * 0.6; // HACK

        let options = usvg::Options {
            fontdb: settings.font_db.clone(),
            ..Default::default()
        };

        let transform = tiny_skia::Transform::default();

        Self {
            settings,
            char_width,
            row_height,
            options,
            transform,
        }
    }

    fn push_header(&self, svg: &mut String) {
        writeln!(svg,
            r#"<svg version="1.1" xmlns="http://www.w3.org/2000/svg" width="{}" height="{}">
<style>
svg {{
    font-size: {}px;
    font-family: {};
    fill: {};
}}
.br {{ font-weight: bold }}
.it {{ font-style: italic }}
.un {{ text-decoration: underline }}
</style>
"#,
            self.settings.pixel_width,
            self.settings.pixel_height,
            self.settings.font_size,
            self.settings.font_families.join(","),
            self.settings.theme.foreground,
        ).unwrap();
        if self.settings.fill_background {
            writeln!(
                svg,
                r#"<rect width="100%" height="100%" rx="{}" ry="{}" style="fill: {}" />"#,
                0, 0, self.settings.theme.background
            )
            .unwrap();
        }
    }

    fn footer() -> &'static str {
        "</svg>"
    }

    fn push_lines(&self, svg: &mut String, lines: &[avt::Line], cursor: Option<(usize, usize)>) {
        self.push_background(svg, &lines, cursor);
        self.push_text(svg, &lines, cursor);
    }

    fn push_background(
        &self,
        svg: &mut String,
        lines: &[avt::Line],
        cursor: Option<(usize, usize)>,
    ) {
        let _ = writeln!(svg, r#"<g style="shape-rendering: optimizeSpeed">"#);

        for (row, line) in lines.iter().enumerate() {
            let y = (row as f64) * self.row_height;
            let mut col = 0;

            for cell in line.cells() {
                let attrs = text_attrs(cell.pen(), &cursor, col, row, &self.settings.theme);

                if attrs.background.is_none() {
                    col += cell.width();
                    continue;
                }

                let x = (col as f64) * self.char_width;
                let style = rect_style(&attrs, &self.settings.theme);
                let width = self.char_width * cell.width() as f64;

                let _ = writeln!(
                    svg,
                    r#"<rect x="{:.3}" y="{:.3}" width="{:.3}" height="{:.3}" style="{}" />"#,
                    x, y, width, self.row_height, style
                );

                col += cell.width();
            }
        }

        let _ = writeln!(svg, "</g>");
    }

    fn push_text(&self, svg: &mut String, lines: &[avt::Line], cursor: Option<(usize, usize)>) {
        let _ = writeln!(svg, r#"<text class="default-text-fill">"#);

        for (row, line) in lines.iter().enumerate() {
            let y = (row as f64) * self.row_height;
            let mut did_dy = false;

            let _ = write!(svg, r#"<tspan y="{y:.3}">"#);
            let mut col = 0;

            for cell in line.cells() {
                let ch = cell.char();

                if ch == ' ' {
                    col += cell.width();
                    continue;
                }

                let attrs = text_attrs(cell.pen(), &cursor, col, row, &self.settings.theme);

                svg.push_str("<tspan ");

                if !did_dy {
                    svg.push_str(r#"dy="1em" "#);
                    did_dy = true;
                }

                let x = col as f64 * self.char_width;
                let class = text_class(&attrs);
                let style = text_style(&attrs, &self.settings.theme);

                let _ = write!(svg, r#"x="{x:.3}" class="{class}" style="{style}">"#);

                match ch {
                    '\'' => {
                        svg.push_str("&#39;");
                    }

                    '"' => {
                        svg.push_str("&quot;");
                    }

                    '&' => {
                        svg.push_str("&amp;");
                    }

                    '>' => {
                        svg.push_str("&gt;");
                    }

                    '<' => {
                        svg.push_str("&lt;");
                    }

                    _ => {
                        svg.push(ch);
                    }
                }

                let _ = writeln!(svg, "</tspan>");
                col += cell.width();
            }

            let _ = writeln!(svg, "</tspan>");
        }

        let _ = writeln!(svg, "</text>");
    }

    pub fn render_svg(&self, lines: &[avt::Line], cursor: Option<(usize, usize)>) -> String {
        let mut svg = String::new();
        self.push_header(&mut svg);
        self.push_lines(&mut svg, lines, cursor);
        svg.push_str(Self::footer());
        svg
    }

    pub fn render_pixmap(&self, svg: &str) -> Pixmap {
        let tree = usvg::Tree::from_str(svg, &self.options).unwrap();

        let mut pixmap =
            tiny_skia::Pixmap::new(self.settings.pixel_width as u32, self.settings.pixel_height as u32).unwrap();

        resvg::render(&tree, self.transform, &mut pixmap.as_mut());
        pixmap
    }
}

impl<'a> Renderer for ResvgRenderer<'a> {
    fn render(&mut self, lines: &[avt::Line], cursor: Option<(usize, usize)>) -> ImgVec<RGBA8> {
        let svg = self.render_svg(lines, cursor);
        let pixmap = self.render_pixmap(&svg);
        let buf = pixmap.take().as_rgba().to_vec();

        ImgVec::new(buf, self.settings.pixel_width, self.settings.pixel_height)
    }

    fn pixel_size(&self) -> (usize, usize) {
        (self.settings.pixel_width, self.settings.pixel_height)
    }
}
