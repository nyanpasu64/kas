// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License in the LICENSE-APACHE file or at:
//     https://www.apache.org/licenses/LICENSE-2.0

//! Widget styling
//!
//! Widget size and appearance can be modified through themes.

use std::f32;
use wgpu_glyph::Font;

use kas::draw::{Colour, Draw};
use kas::event::HighlightState;
use kas::geom::{Coord, Rect};
use kas::theme::{self, TextClass, TextProperties, ThemeAction, ThemeApi};
use kas::Direction;

use super::{Dimensions, DimensionsParams, DimensionsWindow};
use crate::draw::{DrawExt, DrawPipe, DrawText};
use crate::resources::colours::ThemeColours;

/// A simple flat theme.
#[derive(Clone, Debug)]
pub struct FlatTheme {
    font_size: f32,
    cols: ThemeColours,
}

impl FlatTheme {
    /// Construct
    pub fn new() -> Self {
        FlatTheme {
            font_size: 18.0,
            cols: ThemeColours::new(),
        }
    }
}

const DIMS: DimensionsParams = DimensionsParams {
    margin: 2.0,
    frame_size: 4.0,
    button_frame: 6.0,
    scrollbar_size: 8.0,
};

pub struct DrawHandle<'a> {
    draw: &'a mut DrawPipe,
    window: &'a mut DimensionsWindow,
    cols: &'a ThemeColours,
    rect: Rect,
    offset: Coord,
    pass: usize,
}

impl theme::Theme<DrawPipe> for FlatTheme {
    type Window = DimensionsWindow;

    #[cfg(not(feature = "gat"))]
    type DrawHandle = DrawHandle<'static>;
    #[cfg(feature = "gat")]
    type DrawHandle<'a> = DrawHandle<'a>;

    fn new_window(&self, _draw: &mut DrawPipe, dpi_factor: f32) -> Self::Window {
        DimensionsWindow::new(DIMS, self.font_size, dpi_factor)
    }

    fn update_window(&self, window: &mut Self::Window, dpi_factor: f32) {
        window.dims = Dimensions::new(DIMS, self.font_size, dpi_factor);
    }

    #[cfg(not(feature = "gat"))]
    unsafe fn draw_handle<'a>(
        &'a self,
        draw: &'a mut DrawPipe,
        window: &'a mut Self::Window,
        rect: Rect,
    ) -> Self::DrawHandle {
        // We extend lifetimes (unsafe) due to the lack of associated type generics.
        use std::mem::transmute;
        DrawHandle {
            draw: transmute::<&'a mut DrawPipe, &'static mut DrawPipe>(draw),
            window: transmute::<&'a mut Self::Window, &'static mut Self::Window>(window),
            cols: transmute::<&'a ThemeColours, &'static ThemeColours>(&self.cols),
            rect,
            offset: Coord::ZERO,
            pass: 0,
        }
    }
    #[cfg(feature = "gat")]
    fn draw_handle<'a>(
        &'a self,
        draw: &'a mut DrawPipe,
        window: &'a mut Self::Window,
        rect: Rect,
    ) -> Self::DrawHandle<'a> {
        DrawHandle {
            draw,
            window,
            cols: &self.cols,
            rect,
            offset: Coord::ZERO,
            pass: 0,
        }
    }

    fn get_fonts<'a>(&self) -> Vec<Font<'a>> {
        vec![crate::font::get_font()]
    }

    fn light_direction(&self) -> (f32, f32) {
        (0.3, 0.4)
    }

    fn clear_colour(&self) -> Colour {
        self.cols.background
    }
}

impl ThemeApi for FlatTheme {
    fn set_font_size(&mut self, size: f32) -> ThemeAction {
        self.font_size = size;
        ThemeAction::ThemeResize
    }

    fn set_colours(&mut self, scheme: &str) -> ThemeAction {
        if let Some(scheme) = ThemeColours::open(scheme) {
            self.cols = scheme;
            ThemeAction::RedrawAll
        } else {
            ThemeAction::None
        }
    }
}

impl<'a> DrawHandle<'a> {
    /// Draw an edit region with optional navigation highlight.
    /// Return the inner rect.
    fn draw_edit_region(&mut self, outer: Rect, nav_col: Option<Colour>) -> Rect {
        let inner1 = outer.shrink(self.window.dims.frame / 2);
        let inner2 = outer.shrink(self.window.dims.frame);

        self.draw.rect(self.pass, inner1, self.cols.text_area);

        // We draw over the inner rect, taking advantage of the fact that
        // rounded frames get drawn after flat rects.
        self.draw
            .rounded_frame(self.pass, outer, inner2, 0.333, self.cols.frame);

        if let Some(col) = nav_col {
            self.draw.rounded_frame(self.pass, inner1, inner2, 0.0, col);
        }

        inner2
    }
}

impl<'a> theme::DrawHandle for DrawHandle<'a> {
    fn clip_region(
        &mut self,
        rect: Rect,
        offset: Coord,
        f: &mut dyn FnMut(&mut dyn theme::DrawHandle),
    ) {
        let rect = rect + self.offset;
        let pass = self.draw.add_clip_region(rect);
        let mut handle = DrawHandle {
            draw: self.draw,
            window: self.window,
            cols: self.cols,
            rect,
            offset: self.offset - offset,
            pass,
        };
        f(&mut handle);
    }

    fn target_rect(&self) -> Rect {
        // Translate to local coordinates
        self.rect - self.offset
    }

    fn outer_frame(&mut self, rect: Rect) {
        let outer = rect + self.offset;
        let inner = outer.shrink(self.window.dims.frame);
        self.draw
            .rounded_frame(self.pass, outer, inner, 0.5, self.cols.frame);
    }

    fn text(&mut self, rect: Rect, text: &str, props: TextProperties) {
        let scale = self.window.dims.font_scale;
        let col = match props.class {
            TextClass::Label => self.cols.label_text,
            TextClass::Button => self.cols.button_text,
            TextClass::Edit | TextClass::EditMulti => self.cols.text,
        };
        self.draw.text(rect + self.offset, text, scale, props, col);
    }

    fn button(&mut self, rect: Rect, highlights: HighlightState) {
        let outer = rect + self.offset;
        let col = self.cols.button_state(highlights);

        let inner = outer.shrink(self.window.dims.button_frame);
        self.draw.rounded_frame(self.pass, outer, inner, 0.0, col);
        self.draw.rect(self.pass, inner, col);

        if let Some(col) = self.cols.nav_region(highlights) {
            let outer = outer.shrink(self.window.dims.button_frame / 3);
            self.draw.rounded_frame(self.pass, outer, inner, 0.5, col);
        }
    }

    fn edit_box(&mut self, rect: Rect, highlights: HighlightState) {
        self.draw_edit_region(rect + self.offset, self.cols.nav_region(highlights));
    }

    fn checkbox(&mut self, rect: Rect, checked: bool, highlights: HighlightState) {
        let nav_col = self.cols.nav_region(highlights).or_else(|| {
            if checked {
                Some(self.cols.text_area)
            } else {
                None
            }
        });

        let inner = self.draw_edit_region(rect + self.offset, nav_col);

        // TODO: draw an X, not a square!
        if let Some(col) = self.cols.check_mark_state(highlights, checked) {
            self.draw.rect(self.pass, inner, col);
        }
    }

    #[inline]
    fn radiobox(&mut self, rect: Rect, checked: bool, highlights: HighlightState) {
        let nav_col = self.cols.nav_region(highlights).or_else(|| {
            if checked {
                Some(self.cols.text_area)
            } else {
                None
            }
        });

        let inner = self.draw_edit_region(rect + self.offset, nav_col);

        if let Some(col) = self.cols.check_mark_state(highlights, checked) {
            self.draw.circle(self.pass, inner, 0.3, col);
        }
    }

    fn scrollbar(
        &mut self,
        _rect: Rect,
        h_rect: Rect,
        _dir: Direction,
        highlights: HighlightState,
    ) {
        // TODO: also draw slider behind handle: needs an extra layer?

        let outer = h_rect + self.offset;
        let half_width = outer.size.0.min(outer.size.1) / 2;
        let inner = outer.shrink(half_width);
        let col = self.cols.scrollbar_state(highlights);
        self.draw.rounded_frame(self.pass, outer, inner, 0.0, col);
        self.draw.rect(self.pass, inner, col);
    }
}
