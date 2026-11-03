//! The background: POC404's six-pointed star drawn in ASCII, turning slowly, with a bright runner
//! travelling along its edges and a few characters twinkling around it. It replaces the pixel
//! sky: the page is black, so this is the only thing behind the plates.
//!
//! Each frame is text in rows (`frame`), a pure function of the window's size and the time.
use crate::theme;
use gpui::{div, prelude::*, px, Div, Hsla, SharedString};
use horizon_theme::night::Night;
use std::time::{Duration, Instant};

/// The text's size and its row height, in logical pixels; a column is 0.6 of the size (Geist Mono).
const FONT: f32 = 13.;
const LINE: f32 = 13.;
const CELL: f32 = FONT * 0.6;
/// The star redraws this often while the window is in front.
const FRAME: Duration = Duration::from_millis(66);
/// After this long without focus the star stands still (no frames are drawn for nobody).
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
        let rows = frame(w, h, self.t);
        let ink: Hsla = gpui::white().opacity(0.62);
        let mut sky = div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .overflow_hidden()
            .bg(gpui::black())
            .font_family(theme::MONO)
            .text_size(px(FONT))
            .line_height(px(LINE))
            .text_color(ink);
        for row in rows {
            sky = sky.child(
                div()
                    .h(px(LINE))
                    .flex_none()
                    .whitespace_nowrap()
                    .child(SharedString::from(row)),
            );
        }
        (sky, run.then_some(FRAME))
    }

    /// Nothing is kept in the window's image atlas any more.
    pub fn free_replaced(&mut self, _window: &mut gpui::Window) {}
}

/// How far `(px, py)` is from the segment `a`-`b`.
fn segment_distance((px, py): (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (vx, vy) = (b.0 - a.0, b.1 - a.1);
    let len2 = (vx * vx + vy * vy).max(1e-6);
    let u = (((px - a.0) * vx + (py - a.1) * vy) / len2).clamp(0., 1.);
    let (cx, cy) = (a.0 + u * vx, a.1 + u * vy);
    ((px - cx).powi(2) + (py - cy).powi(2)).sqrt()
}

/// A cell's number, scrambled: stable per cell, the stars' positions.
fn hash(col: u32, row: u32) -> u32 {
    let mut h = col.wrapping_mul(0x9e37_79b1) ^ row.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^ (h >> 12)
}

/// The rows of text for a window of `w` by `h` pixels `t` seconds in.
pub fn frame(w: f32, h: f32, t: f32) -> Vec<String> {
    let cols = (w / CELL).ceil().max(1.) as u32;
    let rows = (h / LINE).ceil().max(1.) as u32;
    let (cx, cy) = (w * 0.5, h * 0.5);
    // The star breathes a little and turns once in about 40 seconds.
    let radius = 0.40 * w.min(h) * (1. + 0.025 * (t * 1.3).sin());
    let spin = t * std::f32::consts::TAU / 40.;
    let vertex = |k: usize, offset: f32| {
        let a = spin + offset + k as f32 * std::f32::consts::TAU / 3. - std::f32::consts::FRAC_PI_2;
        (cx + radius * a.cos(), cy + radius * a.sin())
    };
    let mut edges = Vec::with_capacity(6);
    for offset in [0., std::f32::consts::FRAC_PI_3] {
        for k in 0..3 {
            edges.push((vertex(k, offset), vertex((k + 1) % 3, offset)));
        }
    }
    let thick = LINE * 0.5;
    let mut out = Vec::with_capacity(rows as usize);
    for row in 0..rows {
        let mut line = String::with_capacity(cols as usize);
        for col in 0..cols {
            let p = ((col as f32 + 0.5) * CELL, (row as f32 + 0.5) * LINE);
            let d = edges
                .iter()
                .map(|&(a, b)| segment_distance(p, a, b))
                .fold(f32::MAX, f32::min);
            // A bright runner: a short stretch of the star's outline, going round.
            let angle = (p.1 - cy).atan2(p.0 - cx) / std::f32::consts::TAU + 0.5;
            let runner = ((angle - t * 0.12).rem_euclid(1.)) < 0.035;
            let ch = if d < thick * 0.45 {
                if runner {
                    '@'
                } else {
                    '#'
                }
            } else if d < thick * 0.95 {
                '+'
            } else if d < thick * 1.6 {
                ':'
            } else if d < thick * 2.3 {
                '.'
            } else {
                // The sky around: a few cells twinkle, each on its own beat.
                let h = hash(col, row);
                if h % 61 == 0 {
                    match (((t * 1.6 + (h >> 8 & 255) as f32 / 40.).sin() + 1.) * 1.5) as u32 {
                        0 => ' ',
                        1 => '.',
                        2 => '+',
                        _ => '*',
                    }
                } else {
                    ' '
                }
            };
            line.push(ch);
        }
        out.push(line.trim_end().to_string());
    }
    out
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
                    if dist > TAIL * 0.55 && pick % 3 == 0 {
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

    use super::*;

    #[test]
    fn the_star_is_drawn_and_fills_the_window_height() {
        let rows = frame(1180., 760., 0.);
        assert_eq!(rows.len(), (760. / LINE).ceil() as usize);
        let text = rows.join("\n");
        assert!(text.contains('#') && text.contains('+'));
        // Only ASCII: it has to read as ASCII art anywhere.
        assert!(text.is_ascii());
    }

    #[test]
    fn it_turns_and_the_runner_moves() {
        assert_ne!(frame(800., 600., 0.), frame(800., 600., 3.));
        assert!(frame(800., 600., 1.).join("").contains('@'));
    }

    #[test]
    fn a_tiny_window_is_fine() {
        assert!(!frame(1., 1., 5.).is_empty());
    }
}
