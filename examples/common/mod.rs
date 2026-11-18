//! Small helpers shared by the examples (not part of the library).
//!
//! * [`Inbox`] + [`perform`]: run a future in the background and get its
//!   result back into the component state.
//! * [`pick_list`]: a drop-down built from a button and an overlay.
//! * [`toggle`], [`action`], [`gap`]: buttons and spacing.

#![allow(dead_code)]

use frust::{
    ButtonStyle, CrossAxisAlignment, EdgeInsets, OverlayPlacement, OverlaySide, Padding, RwSignal,
    SizedBox, Update, View, With, button, column, overlay_portal, row,
};
use std::fmt::Display;
use std::future::Future;

/// Results of background work, delivered on the UI thread.
pub type Inbox<T> = RwSignal<Vec<T>>;

/// A new, empty inbox. Create it in `Component::init`.
pub fn inbox<T: Send + Sync + 'static>() -> Inbox<T> {
    RwSignal::new(Vec::new())
}

/// Runs `work` in the background; its result lands in `inbox`.
pub fn perform<T, F>(inbox: Inbox<T>, work: F)
where
    T: Send + Sync + 'static,
    F: Future<Output = T> + Send + 'static,
{
    let (sender, receiver) = tokio::sync::oneshot::channel();
    frust::spawn(async move {
        let _ = sender.send(work.await);
    });
    // The signal is written on the UI thread, which wakes the next frame.
    frust::spawn_local(async move {
        if let Ok(result) = receiver.await {
            inbox.update(|queue| queue.push(result));
        }
    });
}

/// Takes the delivered results. Call it at the start of `build`; the read
/// subscribes, so a delivery wakes the application.
pub fn drain<T: Send + Sync + 'static>(inbox: Inbox<T>) -> Vec<T> {
    if inbox.with(|queue| queue.is_empty()) {
        return Vec::new();
    }
    let mut taken = Vec::new();
    inbox.update(|queue| taken = std::mem::take(queue));
    taken
}

/// Horizontal space.
pub fn gap<S: 'static>(width: f64) -> impl View<S> {
    SizedBox(Some(width), Some(1.0))
}

/// A secondary button.
pub fn action<S: 'static>(label: &str, on_press: impl Fn(&mut S) + 'static) -> impl View<S> {
    button(label.to_string(), on_press)
        .style(ButtonStyle::Secondary)
        .small()
}

/// A disabled-looking button that does nothing when `enabled` is false.
pub fn action_if<S: 'static>(
    label: &str,
    enabled: bool,
    on_press: impl Fn(&mut S) + 'static,
) -> impl View<S> {
    button(label.to_string(), move |state: &mut S| {
        if enabled {
            on_press(state)
        }
    })
    .style(ButtonStyle::Secondary)
    .small()
    .disabled(!enabled)
}

/// A button showing an on/off state.
pub fn toggle<S: 'static>(
    label: &str,
    on: bool,
    on_press: impl Fn(&mut S) + 'static,
) -> impl View<S> {
    let mark = if on { "\u{2611}" } else { "\u{2610}" };
    button(format!("{mark} {label}"), on_press)
        .style(if on {
            ButtonStyle::Primary
        } else {
            ButtonStyle::Secondary
        })
        .small()
}

/// A row of items with spacing, vertically centred and padded.
pub fn bar<S: 'static>(items: Vec<frust::AnyView<S>>) -> impl View<S> {
    let mut line = row::<S>().cross_axis(CrossAxisAlignment::Center);
    let count = items.len();
    for (index, item) in items.into_iter().enumerate() {
        line = line.child(item);
        if index + 1 < count {
            line = line.child(gap(8.0));
        }
    }
    Padding(EdgeInsets::symmetric(10.0, 6.0), line)
}

/// A drop-down list: a button with the current choice that opens a list.
///
/// `id` names this list; `open` is the id of the open list (kept in the
/// application state and changed through `set_open`).
pub fn pick_list<S: 'static, T: Clone + Display + PartialEq + 'static>(
    id: &'static str,
    open: Option<&'static str>,
    options: Vec<T>,
    selected: T,
    set_open: impl Fn(&mut S, Option<&'static str>) + Clone + 'static,
    on_select: impl Fn(&mut S, T) + Clone + 'static,
) -> impl View<S> {
    let is_open = open == Some(id);
    let toggle_open = set_open.clone();
    let trigger = button(format!("{selected} \u{25BE}"), move |state: &mut S| {
        toggle_open(state, (!is_open).then_some(id))
    })
    .style(ButtonStyle::Secondary)
    .small();

    let list = is_open.then(|| {
        let mut list = column::<S>();
        for option in options {
            let close = set_open.clone();
            let select = on_select.clone();
            let label = if option == selected {
                format!("\u{2713} {option}")
            } else {
                format!("   {option}")
            };
            let value = option.clone();
            list = list.child(
                button(label, move |state: &mut S| {
                    close(state, None);
                    select(state, value.clone());
                })
                .style(ButtonStyle::Ghost)
                .small(),
            );
        }
        frust::container(list)
            .fill(frust::Color::from_rgb8(0xFA, 0xFA, 0xFB))
            .border(frust::Color::from_rgb8(0xCC, 0xCE, 0xD4), 1.0)
            .radius(4.0)
    });

    let close = set_open;
    overlay_portal(trigger)
        .overlay(list)
        .placement(OverlayPlacement::on(OverlaySide::Bottom))
        .on_outside_tap(move |state: &mut S| close(state, None))
}
