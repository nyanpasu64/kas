// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License in the LICENSE-APACHE file or at:
//     https://www.apache.org/licenses/LICENSE-2.0

//! Drawing APIs — draw interface

use std::any::Any;

use super::color::Rgba;
use super::{Pass, RegionClass};
use crate::geom::{Quad, Rect, Vec2};

/// Interface over a (local) draw object
pub struct Draw<'a, D: Drawable> {
    pub draw: &'a mut D,
    pass: Pass,
}

#[cfg_attr(not(feature = "internal_doc"), doc(hidden))]
impl<'a, D: Drawable> Draw<'a, D> {
    /// Construct
    pub fn new(draw: &'a mut D, pass: Pass) -> Self {
        Draw { draw, pass }
    }
}

impl<'a, D: Drawable> Draw<'a, D> {
    /// Reborrow with a new lifetime
    pub fn reborrow<'b>(&'b mut self) -> Draw<'b, D>
    where
        'a: 'b,
    {
        Draw {
            draw: &mut *self.draw,
            pass: self.pass,
        }
    }

    /// Get current pass
    pub fn pass(&self) -> Pass {
        self.pass
    }

    /// Construct a clip region
    ///
    /// A clip region is a draw target within the same window with a new scissor
    /// rect (i.e. draw operations will be clipped to this `rect`).
    pub fn new_clip_region<'b>(&'b mut self, rect: Rect, class: RegionClass) -> Draw<'b, D> {
        let pass = self.draw.add_clip_region(self.pass, rect, class);
        Draw {
            draw: &mut *self.draw,
            pass,
        }
    }

    /// Get drawable rect for current target
    ///
    /// (This may be smaller than the rect passed to [`Drawable::add_clip_region`].)
    pub fn clip_rect(&self) -> Rect {
        self.draw.get_clip_rect(self.pass)
    }

    /// Draw a rectangle of uniform colour
    pub fn rect(&mut self, rect: Quad, col: Rgba) {
        self.draw.rect(self.pass, rect, col);
    }

    /// Draw a frame of uniform colour
    ///
    /// The frame is defined by the area inside `outer` and not inside `inner`.
    pub fn frame(&mut self, outer: Quad, inner: Quad, col: Rgba) {
        self.draw.frame(self.pass, outer, inner, col);
    }
}

impl<'a, D: DrawableRounded> Draw<'a, D> {
    /// Draw a line with rounded ends and uniform colour
    ///
    /// This command draws a line segment between the points `p1` and `p2`.
    /// Pixels within the given `radius` of this segment are drawn, resulting
    /// in rounded ends and width `2 * radius`.
    ///
    /// Note that for rectangular, axis-aligned lines, [`Drawable::rect`] should be
    /// preferred.
    pub fn rounded_line(&mut self, p1: Vec2, p2: Vec2, radius: f32, col: Rgba) {
        self.draw.rounded_line(self.pass, p1, p2, radius, col);
    }

    /// Draw a circle or oval of uniform colour
    ///
    /// More generally, this shape is an axis-aligned oval which may be hollow.
    ///
    /// The `inner_radius` parameter gives the inner radius relative to the
    /// outer radius: a value of `0.0` will result in the whole shape being
    /// painted, while `1.0` will result in a zero-width line on the outer edge.
    pub fn circle(&mut self, rect: Quad, inner_radius: f32, col: Rgba) {
        self.draw.circle(self.pass, rect, inner_radius, col);
    }

    /// Draw a frame with rounded corners and uniform colour
    ///
    /// All drawing occurs within the `outer` rect and outside of the `inner`
    /// rect. Corners are circular (or more generally, ovular), centered on the
    /// inner corners.
    ///
    /// The `inner_radius` parameter gives the inner radius relative to the
    /// outer radius: a value of `0.0` will result in the whole shape being
    /// painted, while `1.0` will result in a zero-width line on the outer edge.
    /// When `inner_radius > 0`, the frame will be visually thinner than the
    /// allocated area.
    pub fn rounded_frame(&mut self, outer: Quad, inner: Quad, inner_radius: f32, col: Rgba) {
        self.draw
            .rounded_frame(self.pass, outer, inner, inner_radius, col);
    }
}

impl<'a, D: DrawableShaded> Draw<'a, D> {
    /// Add a shaded square to the draw buffer
    pub fn shaded_square(&mut self, rect: Quad, norm: (f32, f32), col: Rgba) {
        self.draw.shaded_square(self.pass, rect, norm, col);
    }

    /// Add a shaded circle to the draw buffer
    pub fn shaded_circle(&mut self, rect: Quad, norm: (f32, f32), col: Rgba) {
        self.draw.shaded_circle(self.pass, rect, norm, col);
    }

    /// Add a square shaded frame to the draw buffer.
    pub fn shaded_square_frame(
        &mut self,
        outer: Quad,
        inner: Quad,
        norm: (f32, f32),
        outer_col: Rgba,
        inner_col: Rgba,
    ) {
        self.draw
            .shaded_square_frame(self.pass, outer, inner, norm, outer_col, inner_col);
    }

    /// Add a rounded shaded frame to the draw buffer.
    pub fn shaded_round_frame(&mut self, outer: Quad, inner: Quad, norm: (f32, f32), col: Rgba) {
        self.draw
            .shaded_round_frame(self.pass, outer, inner, norm, col);
    }
}

/// Base abstraction over drawing
///
/// Unlike [`DrawHandle`], coordinates are specified via a [`Vec2`] and
/// rectangular regions via [`Quad`]. The same coordinate system is used, hence
/// type conversions can be performed with `from` and `into`. Integral
/// coordinates align with pixels, non-integral coordinates may also be used.
///
/// All draw operations may be batched; when drawn primitives overlap, the
/// results are only loosely defined. Draw operations involving transparency
/// should be ordered after those without transparency.
///
/// Draw operations take place over multiple render passes, identified by a
/// handle of type [`Pass`]. In general the user only needs to pass this value
/// into methods as required. [`Drawable::add_clip_region`] creates a new [`Pass`].
#[cfg_attr(not(feature = "internal_doc"), doc(hidden))]
pub trait Drawable: Any {
    /// Cast self to [`std::any::Any`] reference.
    ///
    /// A downcast on this value may be used to obtain a reference to a
    /// shell-specific API.
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// Add a clip region
    fn add_clip_region(&mut self, pass: Pass, rect: Rect, class: RegionClass) -> Pass;

    /// Get drawable rect for a clip region
    ///
    /// (This may be smaller than the rect passed to [`Drawable::add_clip_region`].)
    fn get_clip_rect(&self, pass: Pass) -> Rect;

    /// Draw a rectangle of uniform colour
    fn rect(&mut self, pass: Pass, rect: Quad, col: Rgba);

    /// Draw a frame of uniform colour
    fn frame(&mut self, pass: Pass, outer: Quad, inner: Quad, col: Rgba);
}

/// Drawing commands for rounded shapes
///
/// This trait is an extension over [`Draw`] providing rounded shapes.
///
/// The primitives provided by this trait are partially transparent.
/// If the implementation buffers draw commands, it should draw these
/// primitives after solid primitives.
#[cfg_attr(not(feature = "internal_doc"), doc(hidden))]
pub trait DrawableRounded: Drawable {
    /// Draw a line with rounded ends and uniform colour
    fn rounded_line(&mut self, pass: Pass, p1: Vec2, p2: Vec2, radius: f32, col: Rgba);

    /// Draw a circle or oval of uniform colour
    fn circle(&mut self, pass: Pass, rect: Quad, inner_radius: f32, col: Rgba);

    /// Draw a frame with rounded corners and uniform colour
    fn rounded_frame(&mut self, pass: Pass, outer: Quad, inner: Quad, inner_radius: f32, col: Rgba);
}

/// Drawing commands for shaded shapes
///
/// This trait is an extension over [`Drawable`] providing solid shaded shapes.
///
/// Some drawing primitives (the "round" ones) are partially transparent.
/// If the implementation buffers draw commands, it should draw these
/// primitives after solid primitives.
///
/// These are parameterised via a pair of normals, `(inner, outer)`. These may
/// have values from the closed range `[-1, 1]`, where -1 points inwards,
/// 0 is perpendicular to the screen towards the viewer, and 1 points outwards.
#[cfg_attr(not(feature = "internal_doc"), doc(hidden))]
pub trait DrawableShaded: Drawable {
    /// Add a shaded square to the draw buffer
    fn shaded_square(&mut self, pass: Pass, rect: Quad, norm: (f32, f32), col: Rgba);

    /// Add a shaded circle to the draw buffer
    fn shaded_circle(&mut self, pass: Pass, rect: Quad, norm: (f32, f32), col: Rgba);

    /// Add a square shaded frame to the draw buffer.
    fn shaded_square_frame(
        &mut self,
        pass: Pass,
        outer: Quad,
        inner: Quad,
        norm: (f32, f32),
        outer_col: Rgba,
        inner_col: Rgba,
    );

    /// Add a rounded shaded frame to the draw buffer.
    fn shaded_round_frame(
        &mut self,
        pass: Pass,
        outer: Quad,
        inner: Quad,
        norm: (f32, f32),
        col: Rgba,
    );
}
