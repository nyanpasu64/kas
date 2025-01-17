// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License in the LICENSE-APACHE file or at:
//     https://www.apache.org/licenses/LICENSE-2.0

//! A separator

use std::fmt::Debug;
use std::marker::PhantomData;

use crate::Menu;
use kas::{event, prelude::*};

widget! {
    /// A separator
    ///
    /// This widget draws a bar when in a list.
    #[derive(Clone, Debug, Default)]
    #[handler(msg=M)]
    pub struct Separator<M: Debug + 'static> {
        #[widget_core]
        core: CoreData,
        _msg: PhantomData<M>,
    }

    impl Separator<event::VoidMsg> {
        /// Construct a frame, with void message type
        #[inline]
        pub fn new() -> Self {
            Separator {
                core: Default::default(),
                _msg: Default::default(),
            }
        }
    }

    impl Self {
        /// Construct a frame, with inferred message type
        ///
        /// This may be useful when embedding a separator in a list with
        /// a given message type.
        #[inline]
        pub fn infer() -> Self {
            Separator {
                core: Default::default(),
                _msg: Default::default(),
            }
        }
    }

    impl Layout for Self {
        fn size_rules(&mut self, size_handle: &mut dyn SizeHandle, axis: AxisInfo) -> SizeRules {
            let margins = size_handle.frame_margins();
            SizeRules::extract_fixed(axis, size_handle.separator(), margins)
        }

        fn draw(&mut self, draw: &mut dyn DrawHandle, _: &ManagerState, _: bool) {
            draw.separator(self.core.rect);
        }
    }

    /// A separator is a valid menu widget
    impl Menu for Self {}
}
