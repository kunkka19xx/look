//! The running apps strip at the right end of the top bar: up to nine
//! icons, each wearing the digit that activates it with Alt. Read on each
//! open; hidden in the compact layout, while translating, and when the
//! `running_apps_placement` setting says none.

use gpui::{
    Animation, AnimationExt, Context, Div, FontWeight, SharedString, div, img, prelude::*, px, svg,
};
use linows_backend::process::{self, RunningApp};

use crate::Shell;
use crate::bg;
use crate::glyphs;
use crate::icons::{IconRequest, IconStore};
use crate::launcher::Launcher;
use crate::motion;
use crate::theme::{self, Theme};

const MAX_ITEMS: usize = 9;
/// Easy-to-reach keys first; the picked set is then shown in ascending order.
const EASINESS_ORDER: [u8; 9] = [1, 2, 3, 9, 8, 4, 7, 6, 5];
const GAP: f32 = 8.0;
const BADGE: f32 = 14.0;
const BADGE_OFFSET: f32 = -4.0;
const BADGE_FONT: f32 = 9.0;
const ICON_OPACITY: f32 = 0.82;
const SCRIM_ALPHA: f32 = 0.72;
const PLACEMENT_KEY: &str = "running_apps_placement";
const PLACEMENT_NONE: &str = "none";

#[derive(Default)]
pub struct RunningApps {
    apps: Vec<RunningApp>,
    enabled: bool,
}

/// The keys the first `total` apps answer to, in display order.
fn badge_keys(total: usize) -> Vec<u8> {
    let mut keys: Vec<u8> = EASINESS_ORDER[..total.min(MAX_ITEMS)].to_vec();
    keys.sort_unstable();
    keys
}

impl RunningApps {
    /// The setting, then the list, off the UI thread.
    pub fn refresh(&mut self, cx: &mut Context<Launcher>) {
        bg::fetch(
            cx,
            || {
                let enabled = linows_backend::config::get_config()
                    .entries
                    .into_iter()
                    .find(|e| e.key == PLACEMENT_KEY)
                    .is_none_or(|e| e.value != PLACEMENT_NONE);
                let apps = if enabled {
                    let mut apps = process::list_running_apps();
                    apps.truncate(MAX_ITEMS);
                    apps
                } else {
                    Vec::new()
                };
                (enabled, apps)
            },
            |this, (enabled, apps), _| {
                this.running.enabled = enabled;
                this.running.apps = apps;
            },
        );
    }

    /// Alt+digit. True when an app answered to it.
    pub fn activate_key(&self, key: u8, shell: &Shell, cx: &mut Context<Launcher>) -> bool {
        if !self.enabled {
            return false;
        }
        let keys = badge_keys(self.apps.len());
        let Some(at) = keys.iter().position(|k| *k == key) else {
            return false;
        };
        self.activate(at, shell, cx);
        true
    }

    /// The shell comes from the caller: this runs inside the launcher's own
    /// update, where reading the entity again panics.
    fn activate(&self, at: usize, shell: &Shell, cx: &mut Context<Launcher>) {
        let Some(app) = self.apps.get(at).cloned() else {
            return;
        };
        let shell = shell.clone();
        cx.background_executor()
            .spawn(async move {
                if let Err(err) =
                    process::activate_running_app(&shell, app.pid, app.desktop_id, app.exec)
                {
                    eprintln!("[running-apps] activate failed: {err}");
                }
            })
            .detach();
    }

    /// The strip, or nothing to show.
    pub fn render(
        &self,
        icons: &gpui::Entity<IconStore>,
        th: &Theme,
        cx: &mut Context<Launcher>,
    ) -> Option<Div> {
        if !self.enabled || self.apps.is_empty() {
            return None;
        }
        let keys = badge_keys(self.apps.len());
        let items = self.apps.iter().enumerate().map(|(i, app)| {
            let picture = app.desktop_id.as_ref().and_then(|desktop_id| {
                let path = desktop_id
                    .strip_prefix("app:")
                    .unwrap_or(desktop_id)
                    .to_string();
                icons.update(cx, |store, cx| {
                    store.get(
                        IconRequest {
                            kind: "app".into(),
                            path,
                            id: Some(desktop_id.clone()),
                        },
                        cx,
                    )
                })
            });
            let icon = div()
                .size(px(theme::TOP_ROW_HEIGHT))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(theme::TOP_ROW_HEIGHT * 0.22))
                .overflow_hidden()
                .opacity(ICON_OPACITY)
                .child(match picture {
                    Some(image) => img(image)
                        .size(px(theme::TOP_ROW_HEIGHT))
                        .into_any_element(),
                    None => svg()
                        .path(glyphs::APP)
                        .size(px(theme::SEARCH_ICON))
                        .text_color(th.text_muted)
                        .into_any_element(),
                });
            let badge = div()
                .absolute()
                .top(px(BADGE_OFFSET))
                .right(px(BADGE_OFFSET))
                .size(px(BADGE))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(BADGE / 2.0))
                .bg(theme::wash(
                    th.card_face(),
                    theme::scrim_for(th.text),
                    SCRIM_ALPHA,
                ))
                .border(px(1.0))
                .border_color(theme::mix(th.text, th.border, 0.45))
                .font_family(th.mono_family.clone())
                .text_size(px(BADGE_FONT))
                .font_weight(FontWeight::BOLD)
                .text_color(th.text)
                .child(SharedString::from(keys[i].to_string()));
            // Each tile slides in from the left behind the bar, staggered.
            let delay = motion::slide_delay(i);
            let duration = motion::dur(motion::SLIDE_MS);
            let total = delay + duration;
            div()
                .id(("running", i))
                .relative()
                .flex_shrink_0()
                .cursor_pointer()
                .hover(|s| s.opacity(1.0))
                .on_click(
                    cx.listener(move |this, _, _, cx| this.running.activate(i, &this.shell, cx)),
                )
                .child(icon)
                .child(badge)
                .with_animation(
                    ("strip", i),
                    Animation::new(total),
                    move |tile, progress| {
                        let t = motion::curve(motion::staggered(progress, total, delay, duration));
                        tile.opacity(t)
                            .left(px(motion::rise(0.0, motion::STRIP_SHIFT, t)))
                    },
                )
        });
        Some(
            div()
                .flex_shrink_0()
                .flex()
                .items_center()
                .gap(px(GAP))
                .children(items),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badge_keys_are_the_easy_ones_in_order() {
        assert_eq!(badge_keys(3), vec![1, 2, 3]);
        assert_eq!(badge_keys(5), vec![1, 2, 3, 8, 9]);
        assert_eq!(badge_keys(12).len(), 9);
        assert!(badge_keys(0).is_empty());
    }
}
