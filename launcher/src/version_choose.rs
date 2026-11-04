//! "Choose", beside Play: which Minecraft the player starts next. 26.3 plays with POC404's own
//! Rust client, unchanged; every other version plays Mojang's Java Edition, downloaded once and
//! then offline, exactly as the rest of the launcher already does it.
//!
//! The picker is a drag bar, not a dropdown list: a line of steps, one per version, that the
//! player drags or clicks along, same as the memory slider. Shown inline under Play when open,
//! so there is nothing absolutely positioned that could end up clipped or hidden behind another
//! element — the bug a floating popover had here before.
use super::*;
use std::collections::HashSet;

/// POC404's Rust client's own version.
pub(super) const RUST: &str = "26.3";

/// Every version the picker offers below [`RUST`], newest first: POC404's own 26.x line, then
/// Mojang's Java Edition releases down to 1.17, the oldest this launcher starts.
pub(super) const JAVA_VERSIONS: &[&str] = &[
    "26.2", "26.1.1", "26.1", "26", "1.21.11", "1.21.10", "1.21.9", "1.21.8", "1.21.7", "1.21.6",
    "1.21.5", "1.21.4", "1.21.3", "1.21.2", "1.21.1", "1.21", "1.20.6", "1.20.5", "1.20.4",
    "1.20.3", "1.20.2", "1.20.1", "1.20", "1.19.4", "1.19.3", "1.19.2", "1.19.1", "1.19", "1.18.2",
    "1.18.1", "1.18", "1.17.1", "1.17",
];

/// `RUST`, then [`JAVA_VERSIONS`], oldest last: the slider's steps, in drag order (left to right,
/// oldest to newest, so dragging right moves to a newer version).
fn steps() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = JAVA_VERSIONS.iter().rev().copied().collect();
    v.push(RUST);
    v
}

impl Launcher {
    /// Which Java versions already have their game files on this computer: downloaded once,
    /// started offline after. The same marker `backend::java_game` writes.
    pub(super) fn downloaded_java_versions(&self) -> HashSet<String> {
        let dir = backend::data_dir().join("minecraft").join("versions");
        let Ok(read) = std::fs::read_dir(dir) else {
            return HashSet::new();
        };
        read.flatten()
            .filter(|e| e.path().join(".downloaded").is_file())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect()
    }

    /// The "Choose" button beside Play, open or closed.
    pub(super) fn version_choose_button(&self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        div().child(
            button("choose-version", "Choose", Kind::Secondary, &t).on_click(cx.listener(
                |this, _: &ClickEvent, _, cx| {
                    this.version_choose_open = !this.version_choose_open;
                    cx.notify();
                },
            )),
        )
    }

    /// The version actually playing right now: `RUST`/"POC404" for the Rust client, or a Java
    /// instance's own Minecraft version.
    fn shown_version(&self) -> String {
        match self.selected_instance() {
            Some(i) => i
                .java
                .as_ref()
                .map_or_else(|| RUST.to_string(), |j| j.minecraft.clone()),
            None => RUST.to_string(),
        }
    }

    /// The drag bar itself, shown inline under Play while open: not a popover, so it can never
    /// end up clipped or hidden — it simply takes its place in the page like any other row.
    pub(super) fn version_choose_bar(&self, cx: &mut Context<Self>) -> Option<Div> {
        if !self.version_choose_open {
            return None;
        }
        let t = self.theme;
        let n = t.night;
        let versions = steps();
        let downloaded = self.downloaded_java_versions();
        let shown = self.shown_version();
        let at = versions
            .iter()
            .position(|v| *v == shown)
            .unwrap_or(versions.len() - 1);

        let mut line = div().flex().items_center().gap(px(2.)).w_full();
        for (i, &id) in versions.iter().enumerate() {
            let filled = i <= at;
            let picked = i == at;
            let id = id.to_string();
            line = line.child(
                div()
                    .id(SharedString::from(format!("choose-step-{i}")))
                    .flex()
                    .flex_1()
                    .items_center()
                    .h(px(28.))
                    .cursor_pointer()
                    .child(
                        div()
                            .w_full()
                            .h(px(if picked { 20. } else { 10. }))
                            .bg(if filled { t.accent } else { t.well })
                            .border_1()
                            .border_color(if filled { t.accent } else { t.rule }),
                    )
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.choose_version(&id);
                        cx.notify();
                    })),
            );
        }
        let tag = if shown == RUST {
            "Rust".to_string()
        } else if downloaded.contains(&shown) {
            "Installed".to_string()
        } else {
            "Downloads on first play".to_string()
        };
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .p(px(12.))
                .mt(px(6.))
                .rounded(px(theme::night::RADIUS_CONTROL))
                .border_1()
                .border_color(plate::alpha(n.stroke, 2. * n.stroke_alpha))
                .bg(t.surface)
                .child(
                    div()
                        .flex()
                        .items_baseline()
                        .gap(px(10.))
                        .child(
                            shell::text(theme::night::UI, t.ink)
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(if shown == RUST {
                                    "POC404".to_string()
                                } else {
                                    shown.clone()
                                }),
                        )
                        .child(shell::text(theme::night::CAPTION, t.muted).child(tag)),
                )
                .child(line)
                .child(
                    shell::text(theme::night::CAPTION, t.muted)
                        .child("Drag or click along the bar: oldest release on the left, 26.3 (Rust) on the right."),
                ),
        )
    }

    /// Switches Play to `minecraft`: POC404's own Rust client for [`RUST`], else an existing Java
    /// instance for that version if there is one, else a new one made on the spot (no download
    /// yet — that happens the first time Play is pressed, like any other Java instance).
    fn choose_version(&mut self, minecraft: &str) {
        if minecraft == RUST {
            if let Some(i) = self
                .instances
                .list
                .iter()
                .find(|i| i.java.is_none())
                .cloned()
            {
                self.select_instance(&i);
            } else {
                self.go(Page::Versions);
            }
            return;
        }
        if let Some(i) = self
            .instances
            .list
            .iter()
            .find(|i| i.java.as_ref().is_some_and(|j| j.minecraft == minecraft))
            .cloned()
        {
            self.select_instance(&i);
            return;
        }
        match instances::create_java(minecraft, minecraft) {
            Ok(instance) => {
                self.reload_instances();
                self.select_instance(&instance);
            }
            Err(e) => self.client_error = Some(e),
        }
    }
}
