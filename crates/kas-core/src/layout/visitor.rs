// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License in the LICENSE-APACHE file or at:
//     https://www.apache.org/licenses/LICENSE-2.0

//! Layout visitor

use super::{AlignHints, AxisInfo, RulesSetter, RulesSolver, SizeRules, Storage};
use super::{DynRowStorage, RowPositionSolver, RowSetter, RowSolver, RowStorage};
use super::{GridChildInfo, GridDimensions, GridSetter, GridSolver, GridStorage};
use crate::draw::{DrawHandle, SizeHandle};
use crate::event::{Manager, ManagerState};
use crate::geom::{Offset, Rect, Size};
use crate::{dir::Directional, WidgetConfig};
use std::any::Any;
use std::iter::ExactSizeIterator;

/// Chaining layout storage
///
/// We support embedded layouts within a single widget which means that we must
/// support storage for multiple layouts, though commonly zero or one layout is
/// used. We therefore use a simple linked list.
#[cfg_attr(not(feature = "internal_doc"), doc(hidden))]
#[cfg_attr(doc_cfg, doc(cfg(internal_doc)))]
#[derive(Debug)]
pub struct StorageChain(Option<(Box<StorageChain>, Box<dyn Storage>)>);

impl Default for StorageChain {
    fn default() -> Self {
        StorageChain(None)
    }
}

impl StorageChain {
    /// Access layout storage
    ///
    /// This storage is allocated and initialised on first access.
    ///
    /// Panics if the type `T` differs from the initial usage.
    pub fn storage<T: Storage + Default>(&mut self) -> (&mut T, &mut StorageChain) {
        if let StorageChain(Some(ref mut b)) = self {
            let storage =
                b.1.downcast_mut()
                    .unwrap_or_else(|| panic!("StorageChain::storage::<T>(): incorrect type T"));
            return (storage, &mut b.0);
        }
        // TODO(rust#42877): store (StorageChain, dyn Storage) tuple in a single box
        let s = Box::new(StorageChain(None));
        let t: Box<dyn Storage> = Box::new(T::default());
        *self = StorageChain(Some((s, t)));
        match self {
            StorageChain(Some(b)) => (b.1.downcast_mut::<T>().unwrap(), &mut b.0),
            _ => unreachable!(),
        }
    }
}

/// Implementation helper for layout of children
trait Visitor {
    /// Get size rules for the given axis
    fn size_rules(&mut self, size_handle: &mut dyn SizeHandle, axis: AxisInfo) -> SizeRules;

    /// Apply a given `rect` to self
    fn set_rect(&mut self, mgr: &mut Manager, rect: Rect, align: AlignHints);

    fn is_reversed(&mut self) -> bool;

    fn draw(&mut self, draw: &mut dyn DrawHandle, mgr: &ManagerState, disabled: bool);
}

/// A layout visitor
///
/// This constitutes a "visitor" which iterates over each child widget. Layout
/// algorithm details are implemented over this visitor.
pub struct Layout<'a> {
    layout: LayoutType<'a>,
}

/// Items which can be placed in a layout
enum LayoutType<'a> {
    /// No layout
    None,
    /// A single child widget
    Single(&'a mut dyn WidgetConfig),
    /// A single child widget with alignment
    AlignSingle(&'a mut dyn WidgetConfig, AlignHints),
    /// Apply alignment hints to some sub-layout
    AlignLayout(Box<Layout<'a>>, AlignHints),
    /// An embedded layout
    Visitor(Box<dyn Visitor + 'a>),
}

impl<'a> Default for Layout<'a> {
    fn default() -> Self {
        Layout::none()
    }
}

impl<'a> Layout<'a> {
    /// Construct an empty layout
    pub fn none() -> Self {
        let layout = LayoutType::None;
        Layout { layout }
    }

    /// Construct a single-item layout
    pub fn single(widget: &'a mut dyn WidgetConfig) -> Self {
        let layout = LayoutType::Single(widget);
        Layout { layout }
    }

    /// Construct a single-item layout with alignment hints
    pub fn align_single(widget: &'a mut dyn WidgetConfig, hints: AlignHints) -> Self {
        let layout = LayoutType::AlignSingle(widget, hints);
        Layout { layout }
    }

    /// Align a sub-layout
    pub fn align(layout: Self, hints: AlignHints) -> Self {
        let layout = LayoutType::AlignLayout(Box::new(layout), hints);
        Layout { layout }
    }

    /// Construct a frame around a sub-layout
    ///
    /// This frame has dimensions according to [`SizeHandle::frame`].
    pub fn frame(data: &'a mut FrameStorage, child: Self) -> Self {
        let layout = LayoutType::Visitor(Box::new(Frame { data, child }));
        Layout { layout }
    }

    /// Construct a row/column layout over an iterator of layouts
    pub fn list<I, D, S>(list: I, direction: D, data: &'a mut S) -> Self
    where
        I: ExactSizeIterator<Item = Layout<'a>> + 'a,
        D: Directional,
        S: RowStorage,
    {
        let layout = LayoutType::Visitor(Box::new(List {
            data,
            direction,
            children: list,
        }));
        Layout { layout }
    }

    /// Construct a row/column layout over a slice of widgets
    ///
    /// In contrast to [`Layout::list`], `slice` can only be used over a slice
    /// of a single type of widget, enabling some optimisations: `O(log n)` for
    /// `draw` and `find_id`. Some other methods, however, remain `O(n)`, thus
    /// the optimisations are not (currently) so useful.
    pub fn slice<W, D>(slice: &'a mut [W], direction: D, data: &'a mut DynRowStorage) -> Self
    where
        W: WidgetConfig,
        D: Directional,
    {
        let layout = LayoutType::Visitor(Box::new(Slice {
            data,
            direction,
            children: slice,
        }));
        Layout { layout }
    }

    /// Construct a grid layout over an iterator of `(cell, layout)` items
    pub fn grid<I, S>(iter: I, dim: GridDimensions, data: &'a mut S) -> Self
    where
        I: Iterator<Item = (GridChildInfo, Layout<'a>)> + 'a,
        S: GridStorage,
    {
        let layout = LayoutType::Visitor(Box::new(Grid {
            data,
            dim,
            children: iter,
        }));
        Layout { layout }
    }

    /// Get size rules for the given axis
    #[inline]
    pub fn size_rules(mut self, sh: &mut dyn SizeHandle, axis: AxisInfo) -> SizeRules {
        self.size_rules_(sh, axis)
    }
    fn size_rules_(&mut self, sh: &mut dyn SizeHandle, axis: AxisInfo) -> SizeRules {
        match &mut self.layout {
            LayoutType::None => SizeRules::EMPTY,
            LayoutType::Single(child) => child.size_rules(sh, axis),
            LayoutType::AlignSingle(child, _) => child.size_rules(sh, axis),
            LayoutType::AlignLayout(layout, _) => layout.size_rules_(sh, axis),
            LayoutType::Visitor(visitor) => visitor.size_rules(sh, axis),
        }
    }

    /// Apply a given `rect` to self
    #[inline]
    pub fn set_rect(mut self, mgr: &mut Manager, rect: Rect, align: AlignHints) {
        self.set_rect_(mgr, rect, align);
    }
    fn set_rect_(&mut self, mgr: &mut Manager, rect: Rect, align: AlignHints) {
        match &mut self.layout {
            LayoutType::None => (),
            LayoutType::Single(child) => child.set_rect(mgr, rect, align),
            LayoutType::AlignSingle(child, hints) => {
                let align = hints.combine(align);
                child.set_rect(mgr, rect, align);
            }
            LayoutType::AlignLayout(layout, hints) => {
                let align = hints.combine(align);
                layout.set_rect_(mgr, rect, align);
            }
            LayoutType::Visitor(layout) => layout.set_rect(mgr, rect, align),
        }
    }

    /// Return true if layout is up/left
    ///
    /// This is a lazy method of implementing tab order for reversible layouts.
    #[inline]
    pub fn is_reversed(mut self) -> bool {
        self.is_reversed_()
    }
    fn is_reversed_(&mut self) -> bool {
        match &mut self.layout {
            LayoutType::None => false,
            LayoutType::Single(_) => false,
            LayoutType::AlignSingle(_, _) => false,
            LayoutType::AlignLayout(layout, _) => layout.is_reversed_(),
            LayoutType::Visitor(layout) => layout.is_reversed(),
        }
    }

    /// Draw a widget's children
    #[inline]
    pub fn draw(mut self, draw: &mut dyn DrawHandle, mgr: &ManagerState, disabled: bool) {
        self.draw_(draw, mgr, disabled);
    }
    fn draw_(&mut self, draw: &mut dyn DrawHandle, mgr: &ManagerState, disabled: bool) {
        match &mut self.layout {
            LayoutType::None => (),
            LayoutType::Single(child) => child.draw(draw, mgr, disabled),
            LayoutType::AlignSingle(child, _) => child.draw(draw, mgr, disabled),
            LayoutType::AlignLayout(layout, _) => layout.draw_(draw, mgr, disabled),
            LayoutType::Visitor(layout) => layout.draw(draw, mgr, disabled),
        }
    }
}

/// Implement row/column layout for children
struct List<'a, S, D, I> {
    data: &'a mut S,
    direction: D,
    children: I,
}

impl<'a, S: RowStorage, D: Directional, I> Visitor for List<'a, S, D, I>
where
    I: ExactSizeIterator<Item = Layout<'a>>,
{
    fn size_rules(&mut self, sh: &mut dyn SizeHandle, axis: AxisInfo) -> SizeRules {
        let dim = (self.direction, self.children.len());
        let mut solver = RowSolver::new(axis, dim, self.data);
        for (n, child) in (&mut self.children).enumerate() {
            solver.for_child(self.data, n, |axis| child.size_rules(sh, axis));
        }
        solver.finish(self.data)
    }

    fn set_rect(&mut self, mgr: &mut Manager, rect: Rect, align: AlignHints) {
        let dim = (self.direction, self.children.len());
        let mut setter = RowSetter::<D, Vec<i32>, _>::new(rect, dim, align, self.data);

        for (n, child) in (&mut self.children).enumerate() {
            child.set_rect(mgr, setter.child_rect(self.data, n), align);
        }
    }

    fn is_reversed(&mut self) -> bool {
        self.direction.is_reversed()
    }

    fn draw(&mut self, draw: &mut dyn DrawHandle, mgr: &ManagerState, disabled: bool) {
        for child in &mut self.children {
            child.draw(draw, mgr, disabled);
        }
    }
}

/// A row/column over a slice
struct Slice<'a, W: WidgetConfig, D: Directional> {
    data: &'a mut DynRowStorage,
    direction: D,
    children: &'a mut [W],
}

impl<'a, W: WidgetConfig, D: Directional> Visitor for Slice<'a, W, D> {
    fn size_rules(&mut self, sh: &mut dyn SizeHandle, axis: AxisInfo) -> SizeRules {
        let dim = (self.direction, self.children.len());
        let mut solver = RowSolver::new(axis, dim, self.data);
        for (n, child) in self.children.iter_mut().enumerate() {
            solver.for_child(self.data, n, |axis| child.size_rules(sh, axis));
        }
        solver.finish(self.data)
    }

    fn set_rect(&mut self, mgr: &mut Manager, rect: Rect, align: AlignHints) {
        let dim = (self.direction, self.children.len());
        let mut setter = RowSetter::<D, Vec<i32>, _>::new(rect, dim, align, self.data);

        for (n, child) in self.children.iter_mut().enumerate() {
            child.set_rect(mgr, setter.child_rect(self.data, n), align);
        }
    }

    fn is_reversed(&mut self) -> bool {
        self.direction.is_reversed()
    }

    fn draw(&mut self, draw: &mut dyn DrawHandle, mgr: &ManagerState, disabled: bool) {
        let solver = RowPositionSolver::new(self.direction);
        solver.for_children(self.children, draw.get_clip_rect(), |w| {
            w.draw(draw, mgr, disabled)
        });
    }
}

/// Implement grid layout for children
struct Grid<'a, S, I> {
    data: &'a mut S,
    dim: GridDimensions,
    children: I,
}

impl<'a, S: GridStorage, I> Visitor for Grid<'a, S, I>
where
    I: Iterator<Item = (GridChildInfo, Layout<'a>)>,
{
    fn size_rules(&mut self, sh: &mut dyn SizeHandle, axis: AxisInfo) -> SizeRules {
        let mut solver = GridSolver::<Vec<_>, Vec<_>, _>::new(axis, self.dim, self.data);
        for (info, child) in &mut self.children {
            solver.for_child(self.data, info, |axis| child.size_rules(sh, axis));
        }
        solver.finish(self.data)
    }

    fn set_rect(&mut self, mgr: &mut Manager, rect: Rect, align: AlignHints) {
        let mut setter = GridSetter::<Vec<_>, Vec<_>, _>::new(rect, self.dim, align, self.data);
        for (info, child) in &mut self.children {
            child.set_rect(mgr, setter.child_rect(self.data, info), align);
        }
    }

    fn is_reversed(&mut self) -> bool {
        // TODO: replace is_reversed with direct implementation of spatial_nav
        false
    }

    fn draw(&mut self, draw: &mut dyn DrawHandle, mgr: &ManagerState, disabled: bool) {
        for (_, child) in &mut self.children {
            child.draw(draw, mgr, disabled);
        }
    }
}

/// Layout storage for frame layout
#[derive(Default, Debug)]
pub struct FrameStorage {
    // NOTE: potentially rect is redundant (e.g. with widget's rect) but if we
    // want an alternative as a generic solution then all draw methods must
    // calculate and pass the child's rect, which is probably worse.
    rect: Rect,
    offset: Offset,
    size: Size,
}
impl Storage for FrameStorage {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// A frame around other content
struct Frame<'a> {
    data: &'a mut FrameStorage,
    child: Layout<'a>,
}

impl<'a> Visitor for Frame<'a> {
    fn size_rules(&mut self, size_handle: &mut dyn SizeHandle, axis: AxisInfo) -> SizeRules {
        let frame_rules = size_handle.frame(axis.is_vertical());
        let child_rules = self.child.size_rules_(size_handle, axis);
        let (rules, offset, size) = frame_rules.surround_as_margin(child_rules);
        self.data.offset.set_component(axis, offset);
        self.data.size.set_component(axis, size);
        rules
    }

    fn set_rect(&mut self, mgr: &mut Manager, mut rect: Rect, align: AlignHints) {
        self.data.rect = rect;
        rect.pos += self.data.offset;
        rect.size -= self.data.size;
        self.child.set_rect_(mgr, rect, align);
    }

    fn is_reversed(&mut self) -> bool {
        self.child.is_reversed_()
    }

    fn draw(&mut self, draw: &mut dyn DrawHandle, mgr: &ManagerState, disabled: bool) {
        draw.outer_frame(self.data.rect);
        self.child.draw_(draw, mgr, disabled);
    }
}
