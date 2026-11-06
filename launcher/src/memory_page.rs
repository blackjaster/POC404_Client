//! The memory slider (`horizon_launcher::memory`): a line of steps to point at, from the least
//! the game starts on up to the most this computer can spare without stalling. The steps past
//! that are drawn in the danger colour and can't be picked. Used by the instance dialog and by
//! the Play page's Memory row.
use super::*;
use horizon_launcher::memory::{self, Limits};

/// A slider for the game's memory, `current` MiB of it chosen. `on_pick` gets the new value,
/// already inside the limits.
pub(super) fn memory_slider(
    id: &'static str,
    current: u32,
    limits: &Limits,
    t: &Theme,
    on_pick: impl Fn(u32, &mut Window, &mut App) + Clone + 'static,
) -> Div {
    let t = *t;
    let limits = *limits;
    let current = limits.clamp(current);
    let mut line = div().flex().items_center().gap(px(2.)).w_full();
    for p in limits.positions() {
        let pick = on_pick.clone();
        let filled = p <= current;
        line = line.child(
            div()
                .id(SharedString::from(format!("{id}-{p}")))
                .flex()
                .flex_1()
                .items_center()
                .h(px(32.))
                .cursor_pointer()
                .child(
                    div()
                        .w_full()
                        .h(px(if p == current { 24. } else { 14. }))
                        .bg(if filled { t.accent } else { t.well })
                        .border_1()
                        .border_color(if filled { t.accent } else { t.rule }),
                )
                .on_click(move |_: &ClickEvent, window, cx| pick(p, window, cx)),
        );
    }
    // What the computer has past the safe maximum: shown, never pickable.
    for _ in limits.danger_positions() {
        line = line.child(
            div().flex().flex_1().items_center().h(px(32.)).child(
                div()
                    .w_full()
                    .h(px(8.))
                    .bg(t.danger.opacity(0.35))
                    .border_1()
                    .border_color(t.danger.opacity(0.5)),
            ),
        );
    }
    let caption = match limits.total_mb {
        Some(total) => format!(
            "This computer has {}. Up to {} is safe; more would slow everything else down or \
             freeze it.",
            memory::gb(total),
            memory::gb(limits.safe_max_mb)
        ),
        None => format!(
            "This computer's memory couldn't be read. Up to {} is offered.",
            memory::gb(limits.safe_max_mb)
        ),
    };
    let step = |label: &'static str, suffix: &'static str, to: u32| {
        let pick = on_pick.clone();
        button(format!("{id}-{suffix}"), label, Kind::Ghost, &t)
            .on_click(move |_: &ClickEvent, window, cx| pick(to, window, cx))
    };
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(
            div()
                .flex()
                .items_baseline()
                .gap(px(12.))
                .child(
                    blocks::pixel(26., FontWeight::SEMIBOLD)
                        .text_color(t.ink)
                        .child(memory::gb(current)),
                )
                .child(
                    text(theme::CAPTION)
                        .text_color(t.muted)
                        .child(format!("of {} at most", memory::gb(limits.safe_max_mb))),
                ),
        )
        .child(line)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(step(
                    "−",
                    "less",
                    limits.clamp(current.saturating_sub(memory::STEP_MB)),
                ))
                .child(step("+", "more", limits.clamp(current + memory::STEP_MB)))
                .child(step("Recommended", "recommended", limits.recommended_mb))
                .child(step("Most", "most", limits.safe_max_mb)),
        )
        .child(text(theme::CAPTION).text_color(t.muted).child(caption))
}

impl Launcher {
    /// Settings' row for how much memory a new instance starts with: the same slider, saved as
    /// the player's own default instead of one instance's own setting.
    pub(super) fn default_memory_row(&self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let limits = Limits::detect();
        let current = self
            .settings
            .default_memory_mb
            .unwrap_or(limits.recommended_mb);
        let weak = cx.entity().downgrade();
        let slider = memory_slider("default-memory", current, &limits, &t, move |mb, _, cx| {
            let _ = weak.update(cx, |this, cx| {
                this.settings.default_memory_mb = Some(mb);
                this.settings.save();
                cx.notify();
            });
        });
        index_row(
            0,
            "Memory",
            Some(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .pt(px(4.))
                    .max_w(px(640.))
                    .child("The most memory a new instance starts with. Each instance can still be changed on its own, in Edit… or on the Play page.")
                    .child(slider)
                    .into_any_element(),
            ),
            None,
            &t,
        )
    }
}
