// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License in the LICENSE-APACHE file or at:
//     https://www.apache.org/licenses/LICENSE-2.0

//! Window widgets

use std::fmt::{self, Debug};

use crate::class::Class;
use crate::event::{Callback, Event, Handler, Response};
use crate::geom::{AxisInfo, Coord, Rect, Size, SizeRules};
use crate::macros::Widget;
use crate::{Core, CoreData, Layout, TkWindow, Widget};

/// The main instantiation of the [`Window`] trait.
#[widget(class = Class::Window)]
#[derive(Widget)]
pub struct Window<W: Widget + 'static> {
    #[core]
    core: CoreData,
    min_size: Size,
    #[widget]
    w: W,
    fns: Vec<(Callback, &'static dyn Fn(&mut W, &mut dyn TkWindow))>,
}

impl<W: Widget> Debug for Window<W> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Window {{ core: {:?}, min_size: {:?}, solver: <omitted>, w: {:?}, fns: [",
            self.core, self.min_size, self.w
        )?;
        let mut iter = self.fns.iter();
        if let Some(first) = iter.next() {
            write!(f, "({:?}, <Fn>)", first.0)?;
            for next in iter {
                write!(f, ", ({:?}, <Fn>)", next.0)?;
            }
        }
        write!(f, "] }}")
    }
}

impl<W: Widget + Clone> Clone for Window<W> {
    fn clone(&self) -> Self {
        Window {
            core: self.core.clone(),
            min_size: self.min_size,
            w: self.w.clone(),
            fns: self.fns.clone(),
        }
    }
}

impl<W: Widget> Layout for Window<W> {
    fn size_rules(&mut self, tk: &mut dyn TkWindow, axis: AxisInfo) -> SizeRules {
        self.w.size_rules(tk, axis)
    }

    fn set_rect(&mut self, rect: Rect) {
        self.core_data_mut().rect = rect;
        self.w.set_rect(rect);
    }
}

impl<W: Widget> Window<W> {
    /// Create
    pub fn new(w: W) -> Window<W> {
        Window {
            core: Default::default(),
            min_size: Size::ZERO,
            w,
            fns: Vec::new(),
        }
    }

    /// Add a closure to be called, with a reference to self, on the given
    /// condition. The closure must be passed by reference.
    pub fn add_callback(
        &mut self,
        condition: Callback,
        f: &'static dyn Fn(&mut W, &mut dyn TkWindow),
    ) {
        self.fns.push((condition, f));
    }
}

impl<M, W: Widget + Handler<Msg = M> + 'static> Handler for Window<W> {
    type Msg = ();

    fn handle(&mut self, tk: &mut dyn TkWindow, event: Event) -> Response<Self::Msg> {
        // The window itself doesn't handle events, so we can just pass through
        // TODO: either allow a custom handler or require M=()
        let r = self.w.handle(tk, event);
        Response::try_from(r).unwrap_or_else(|_| {
            println!("TODO: widget returned custom msg to window");
            Response::None
        })
    }
}

impl<M, W: Widget + Handler<Msg = M> + 'static> kas::Window for Window<W> {
    fn resize(&mut self, tk: &mut dyn TkWindow, size: Size) {
        // We call size_rules not because we want the result, but because our
        // spec requires that we do so before calling set_rect.
        let _ = self.size_rules(tk, AxisInfo::new(false, None));
        let _ = self.size_rules(tk, AxisInfo::new(true, Some(size.0)));
        let pos = Coord(0, 0);
        self.set_rect(Rect { pos, size });

        // println!("Window:");
        // self.w.print_hierarchy(0);
    }

    fn callbacks(&self) -> Vec<(usize, Callback)> {
        self.fns.iter().map(|(cond, _)| *cond).enumerate().collect()
    }

    /// Trigger a callback (see `iter_callbacks`).
    fn trigger_callback(&mut self, index: usize, tk: &mut dyn TkWindow) {
        let cb = &mut self.fns[index].1;
        cb(&mut self.w, tk);
    }
}