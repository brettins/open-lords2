use super::*;
use l2_view::Canvas;

impl<'a> Pen<'a> {
    /// `FUN_0040328E(group, index, x, y, width, …)` (`0x0040328E`): one `L2.eng`
    /// string wrapped by [`crate::message::break_lines`], a line per
    /// `y += 0x10` (`0x18` in `g_fontHeading`). [V] Returns the height painted.
    pub fn body_wrapped(&self, canvas: &mut Canvas, x: i32, y: i32, width: i32, s: &str, colour: u8) -> i32 {
        l2_net::trace_leaf!("text", 0x0040328E, "FUN_0040328E", s = s, x = x, y = y, width = width; {
            self.lines(canvas, x, y, &self.wrap(s, width), colour) })
    }

    /// `FUN_0040352F(buffer, x, y, width, font, colour)` (`0x0040352F`): a raw
    /// buffer, whose wrapped lines keep their leading spaces. [V]
    pub fn buffer_wrapped(&self, canvas: &mut Canvas, x: i32, y: i32, width: i32, s: &str, colour: u8) -> i32 {
        l2_net::trace_leaf!("text", 0x0040352F, "FUN_0040352F", s = s, x = x, y = y, width = width; {
            self.lines(canvas, x, y, &self.wrap_buffer(s, width), colour) })
    }

    fn lines(&self, canvas: &mut Canvas, x: i32, y: i32, lines: &[String], colour: u8) -> i32 {
        let step = self.assets.body.as_ref().map_or(16, |f| f.line);
        let mut line = y;
        for text in lines {
            self.body(canvas, x, line, text, colour);
            line += step;
        }
        line - y
    }

    /// `FUN_004015B9` (`0x004015B9`): a glyph's frame width plus one, 4 for a space.
    fn glyph(&self, c: char) -> i32 {
        let s = c.to_string();
        match &self.assets.body {
            Some(f) => f.width(&s),
            None => l2_view::text::width(&s),
        }
    }

    pub fn wrap(&self, s: &str, width: i32) -> Vec<String> {
        crate::message::break_lines(s, width, |c| self.glyph(c))
    }

    pub fn wrap_buffer(&self, s: &str, width: i32) -> Vec<String> {
        crate::message::break_buffer(s, width, |c| self.glyph(c))
    }
}
