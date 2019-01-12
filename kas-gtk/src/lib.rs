// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License in the LICENSE-APACHE file or at:
//     https://www.apache.org/licenses/LICENSE-2.0

//! GTK toolkit for kas

#![feature(const_vec_new)]

mod event;
mod widget;
mod window;
mod tkd;

use std::marker::PhantomData;
use std::{cell::RefCell, rc::Rc};


/// Object used to initialise GTK and create windows.
/// 
/// You should only create a single instance of this type. It is neither
/// `Send` nor `Sync`, thus is constrained to the thread on which it is
/// created. On OS X, it must be created on the "main thread".
pub struct Toolkit {
    // we store no real data: it is all thread-local
    _phantom: PhantomData<Rc<()>>,  // not Send or Sync
}

impl Toolkit {
    /// Construct a new instance. This initialises GTK. This should only be
    /// constructed once.
    pub fn new() -> Result<Self, Error> {
        (gtk::init().map_err(|e| Error(e.0)))?;
        
        gdk::Event::set_handler(Some(event::handler));
        
        Ok(Toolkit { _phantom: Default::default() })
    }
}

impl kas::Toolkit for Toolkit {
    fn add_rc(&self, win: Rc<RefCell<kas::Window>>) {
        window::with_list(|list| list.add_window(win))
    }
    
    fn main(&mut self) {
        window::with_list(|list| {
            for window in &list.windows {
                window.win.borrow_mut().on_start(&widget::Toolkit);
            }
        });
        gtk::main();
    }
    
    fn tk_widget(&self) -> &kas::TkWidget {
        &widget::Toolkit
    }
}


/// Error type.
#[derive(Debug)]
pub struct Error(pub &'static str);
