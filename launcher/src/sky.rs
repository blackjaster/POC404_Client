//! The background: a black page with POC404's snake eating its own tail, turning slowly and
//! without end, drawn from the traced line art (`assets/ouroboros.svg`, the picture's white paper
//! removed) in white. The opening's falling matrix (`matrix`) lives here too.
use crate::theme;
use gpui::{div, prelude::*, px, radians, svg, Div, Hsla, SharedString, Transformation};
use horizon_theme::night::Night;
use std::time::{Duration, Instant};

/// The text's size and its row height, in logical pixels; a column is 0.6 of the size (Geist Mono).
const FONT: f32 = 13.;
const LINE: f32 = 13.;
const CELL: f32 = FONT * 0.6;
/// The snake redraws this often while the window is in front.
const FRAME: Duration = Duration::from_millis(40);
/// After this long without focus the snake stands still (no frames are drawn for nobody).
const UNFOCUSED: Duration = Duration::from_secs(10);

#[derive(Default)]
pub struct Sky {
    /// The animation's own clock, in seconds: it only runs while the window is in front.
    t: f32,
    last: Option<Instant>,
    unfocused: Option<Instant>,
}

impl Sky {
    /// The background for a window of `w` by `h` logical pixels, and how long until it next
    /// changes (`None`: it stands still until something else redraws the window).
    pub fn render(
        &mut self,
        _n: &Night,
        (w, h): (f32, f32),
        _scale: f32,
        focused: bool,
        now: Instant,
    ) -> (Div, Option<Duration>) {
        if focused {
            self.unfocused = None;
        } else {
            self.unfocused.get_or_insert(now);
        }
        let run = self.unfocused.is_none_or(|since| now - since < UNFOCUSED);
        let dt = self.last.map_or(0., |last| (now - last).as_secs_f32());
        self.last = Some(now);
        if run {
            // A long gap (the window was hidden) is not a jump of the animation.
            self.t += dt.min(0.25);
        }
        // One full turn every 40 seconds, the snake always chasing its tail; it breathes a little.
        let turn = self.t * std::f32::consts::TAU / 40.;
        let side = 0.8 * w.min(h) * (1. + 0.02 * (self.t * 1.1).sin());
        let ink: Hsla = gpui::white().opacity(0.5);
        let sky = div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .overflow_hidden()
            .bg(gpui::black())
            .flex()
            .items_center()
            .justify_center()
            .child(
                svg()
                    .path("icons/ouroboros.svg")
                    .size(px(side))
                    .flex_none()
                    .text_color(ink)
                    .with_transformation(Transformation::rotate(radians(turn))),
            );
        (sky, run.then_some(FRAME))
    }

    /// Nothing is kept in the window's image atlas any more.
    pub fn free_replaced(&mut self, _window: &mut gpui::Window) {}
}

/// A cell's number, scrambled: stable per cell, the stars' positions.
fn hash(col: u32, row: u32) -> u32 {
    let mut h = col.wrapping_mul(0x9e37_79b1) ^ row.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^ (h >> 12)
}

/// The matrix of the opening: characters falling in columns, one beat per frame.
pub fn matrix_rows(w: f32, h: f32, t: f32) -> Vec<String> {
    const GLYPHS: &[u8] = b"01POC404#$%&*+=<>|/";
    const TAIL: f32 = 14.;
    let cols = (w / CELL).ceil().max(1.) as u32;
    let rows = (h / LINE).ceil().max(1.) as u32;
    let span = rows as f32 + TAIL + 6.;
    let mut out = Vec::with_capacity(rows as usize);
    for row in 0..rows {
        let mut line = String::with_capacity(cols as usize);
        for col in 0..cols {
            let seed = hash(col, 0);
            // Some columns stay empty; each of the others falls at its own speed.
            let ch = if seed % 10 < 7 {
                let speed = 7. + (seed >> 4) as f32 % 9.;
                let offset = (hash(col, 1) % 1000) as f32 / 1000. * span;
                let dist = (t * speed + offset).rem_euclid(span) - row as f32;
                if (0. ..TAIL).contains(&dist) {
                    let beat = (t * 12.) as u32;
                    let pick = hash(col, row.wrapping_add(beat.wrapping_mul(7919)));
                    // The tail thins out as it goes.
                    if dist > TAIL * 0.55 && pick.is_multiple_of(3) {
                        ' '
                    } else {
                        GLYPHS[(pick >> 3) as usize % GLYPHS.len()] as char
                    }
                } else {
                    ' '
                }
            } else {
                ' '
            };
            line.push(ch);
        }
        out.push(line.trim_end().to_string());
    }
    out
}

/// The matrix as a layer over the opening, white, fading with `fade` (0 to 1).
pub fn matrix(w: f32, h: f32, t: f32, fade: f32) -> Div {
    let mut layer = div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .overflow_hidden()
        .font_family(theme::MONO)
        .text_size(px(FONT))
        .line_height(px(LINE))
        .text_color(gpui::white().opacity(0.85 * fade));
    for row in matrix_rows(w, h, t) {
        layer = layer.child(
            div()
                .h(px(LINE))
                .flex_none()
                .whitespace_nowrap()
                .child(SharedString::from(row)),
        );
    }
    layer
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_matrix_falls_and_is_ascii() {
        let a = super::matrix_rows(800., 600., 0.5);
        let b = super::matrix_rows(800., 600., 1.0);
        assert_ne!(a, b);
        assert!(a.join("").is_ascii() && a.join("").contains('0'));
    }
}
