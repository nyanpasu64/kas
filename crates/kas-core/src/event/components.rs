// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License in the LICENSE-APACHE file or at:
//     https://www.apache.org/licenses/LICENSE-2.0

//! Event handling components

use super::{Event, GrabMode, Manager, PressSource};
use crate::geom::{Coord, Offset};
#[allow(unused)]
use crate::text::SelectionHelper;
use crate::WidgetId;

const TIMER_ID: u64 = 1 << 60;

#[derive(Clone, Debug, PartialEq)]
enum TouchPhase {
    None,
    Start(u64, Coord), // id, coord
    Pan(u64),          // id
    Cursor(u64),       // id
}

impl Default for TouchPhase {
    fn default() -> Self {
        TouchPhase::None
    }
}

/// Handles text selection and panning from mouse and touch events
#[derive(Clone, Debug, Default)]
pub struct TextInput {
    touch_phase: TouchPhase,
}

/// Result of [`TextInput::handle`]
pub enum TextInputAction {
    /// No action (event consumed)
    None,
    /// Event not used
    Unhandled,
    /// Pan text using the given `delta`
    Pan(Offset),
    /// Keyboard focus should be requested (if not already active)
    ///
    /// This is also the case for variant `Cursor(_, true, _, _)` (i.e. if
    /// `anchor == true`).
    Focus,
    /// Update cursor and/or selection: `(coord, anchor, clear, repeats)`
    ///
    /// The cursor position should be moved to `coord`.
    ///
    /// If `anchor`, the anchor position (used for word and line selection mode)
    /// should be set to the new cursor position.
    ///
    /// If `clear`, the selection should be cleared (move selection position to
    /// edit position).
    ///
    /// If `repeats > 1`, [`SelectionHelper::expand`] should be called with
    /// this parameter to enable word/line selection mode.
    Cursor(Coord, bool, bool, u32),
}

impl TextInput {
    /// Handle input events
    ///
    /// Consumes the following events: `PressStart`, `PressMove`, `PressEnd`,
    /// `TimerUpdate(1 << 60)`. May request press grabs and timer updates.
    pub fn handle(&mut self, mgr: &mut Manager, w_id: WidgetId, event: Event) -> TextInputAction {
        use TextInputAction as Action;
        match event {
            Event::PressStart { source, coord, .. } if source.is_primary() => {
                let grab = mgr.request_grab(w_id, source, coord, GrabMode::Grab, None);
                match source {
                    PressSource::Touch(touch_id) => {
                        if grab && self.touch_phase == TouchPhase::None {
                            self.touch_phase = TouchPhase::Start(touch_id, coord);
                            let delay = mgr.config().touch_text_sel_delay();
                            mgr.update_on_timer(delay, w_id, TIMER_ID);
                        }
                        Action::Focus
                    }
                    PressSource::Mouse(..) if mgr.config_enable_mouse_text_pan() => Action::Focus,
                    PressSource::Mouse(_, repeats) => {
                        Action::Cursor(coord, true, !mgr.modifiers().shift(), repeats)
                    }
                }
            }
            Event::PressMove {
                source,
                coord,
                delta,
                ..
            } => match source {
                PressSource::Touch(touch_id) => match self.touch_phase {
                    TouchPhase::None => {
                        self.touch_phase = TouchPhase::Pan(touch_id);
                        Action::Pan(delta)
                    }
                    TouchPhase::Start(id, start_coord) if id == touch_id => {
                        if mgr.config_test_pan_thresh(coord - start_coord) {
                            self.touch_phase = TouchPhase::Pan(id);
                            Action::Pan(delta)
                        } else {
                            Action::None
                        }
                    }
                    TouchPhase::Pan(id) if id == touch_id => Action::Pan(delta),
                    _ => Action::Cursor(coord, false, false, 1),
                },
                PressSource::Mouse(..) if mgr.config_enable_mouse_text_pan() => Action::Pan(delta),
                PressSource::Mouse(_, repeats) => Action::Cursor(coord, false, false, repeats),
            },
            Event::PressEnd { source, .. } => {
                match self.touch_phase {
                    TouchPhase::Start(id, ..) | TouchPhase::Pan(id) | TouchPhase::Cursor(id)
                        if source == PressSource::Touch(id) =>
                    {
                        self.touch_phase = TouchPhase::None;
                    }
                    _ => (),
                }
                Action::None
            }
            Event::TimerUpdate(TIMER_ID) => {
                match self.touch_phase {
                    TouchPhase::Start(touch_id, coord) => {
                        self.touch_phase = TouchPhase::Cursor(touch_id);
                        Action::Cursor(coord, false, !mgr.modifiers().shift(), 1)
                    }
                    // Note: if the TimerUpdate were from another requester it
                    // should technically be Unhandled, but it doesn't matter
                    // so long as other consumers match this first.
                    _ => Action::None,
                }
            }
            _ => Action::Unhandled,
        }
    }
}
