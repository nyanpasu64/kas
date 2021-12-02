// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License in the LICENSE-APACHE file or at:
//     https://www.apache.org/licenses/LICENSE-2.0

//! A grid widget

use kas::layout::{
    DynGridStorage, GridChildInfo, GridSetter, GridSolver, RulesSetter, RulesSolver,
};
use kas::{event, prelude::*};
use std::ops::{Index, IndexMut};

/// A grid of boxed widgets
///
/// This is a parameterisation of [`Grid`]
/// This is parameterised over the handler message type.
///
/// See documentation of [`Grid`] type.
pub type BoxGrid<M> = Grid<Box<dyn Widget<Msg = M>>>;

widget! {
    /// A generic grid widget
    ///
    /// Child widgets are displayed in a grid, according to each child's
    /// [`GridChildInfo`]. This allows spans and overlapping widgets. The numbers
    /// of rows and columns is determined automatically while the sizes of rows and
    /// columns are determined based on their contents (including special handling
    /// for spans, *mostly* with good results).
    ///
    /// Note that all child widgets are stored in a list internally. The order of
    /// widgets in that list does not affect display position, but does have a few
    /// effects: (a) widgets may be accessed in this order via indexing, (b) widgets
    /// are configured and drawn in this order, (c) navigating
    /// through widgets with the Tab key currently uses the list order (though it
    /// may be changed in the future to use display order).
    ///
    /// There is no protection against multiple widgets occupying the same cell.
    /// If this does happen, the last widget in that cell will appear on top, but
    /// overlapping widget drawing may not be pretty.
    ///
    /// ## Alternatives
    ///
    /// Where the entries are fixed, also consider custom [`Widget`] implementations.
    ///
    /// ## Performance
    ///
    /// Most operations are `O(n)` in the number of children.
    #[autoimpl(Default)]
    #[derive(Clone, Debug)]
    #[handler(msg=<W as Handler>::Msg)]
    pub struct Grid<W: Widget> {
        first_id: WidgetId,
        #[widget_core]
        core: CoreData,
        widgets: Vec<(GridChildInfo, W)>,
        data: DynGridStorage,
        dim: (u32, u32, u32, u32),
    }

    impl WidgetChildren for Self {
        #[inline]
        fn first_id(&self) -> WidgetId {
            self.first_id
        }
        fn record_first_id(&mut self, id: WidgetId) {
            self.first_id = id;
        }
        #[inline]
        fn num_children(&self) -> usize {
            self.widgets.len()
        }
        #[inline]
        fn get_child(&self, index: usize) -> Option<&dyn WidgetConfig> {
            self.widgets.get(index).map(|c| c.1.as_widget())
        }
        #[inline]
        fn get_child_mut(&mut self, index: usize) -> Option<&mut dyn WidgetConfig> {
            self.widgets.get_mut(index).map(|c| c.1.as_widget_mut())
        }
    }

    impl Layout for Self {
        fn size_rules(&mut self, size_handle: &mut dyn SizeHandle, axis: AxisInfo) -> SizeRules {
            let mut solver = GridSolver::<Vec<_>, Vec<_>, _>::new(axis, self.dim, &mut self.data);
            for child in self.widgets.iter_mut() {
                solver.for_child(&mut self.data, child.0, |axis| {
                    child.1.size_rules(size_handle, axis)
                });
            }
            solver.finish(&mut self.data)
        }

        fn set_rect(&mut self, mgr: &mut Manager, rect: Rect, align: AlignHints) {
            self.core.rect = rect;
            let mut setter =
                GridSetter::<Vec<i32>, Vec<i32>, _>::new(rect, self.dim, align, &mut self.data);

            for child in self.widgets.iter_mut() {
                child
                    .1
                    .set_rect(mgr, setter.child_rect(&mut self.data, child.0), align);
            }
        }

        // TODO: we should probably implement spatial_nav (the same is true for
        // macro-generated grid widgets).
        // fn spatial_nav(&self, reverse: bool, from: Option<usize>) -> Option<usize> { .. }

        // TODO: more efficient find_id and draw?

        fn draw(&mut self, draw: &mut dyn DrawHandle, mgr: &ManagerState, disabled: bool) {
            let disabled = disabled || self.is_disabled();
            for child in &mut self.widgets {
                child.1.draw(draw, mgr, disabled)
            }
        }
    }

    impl event::SendEvent for Self {
        fn send(&mut self, mgr: &mut Manager, id: WidgetId, event: Event) -> Response<Self::Msg> {
            if !self.is_disabled() {
                for child in self.widgets.iter_mut() {
                    if id <= child.1.id() {
                        let r = child.1.send(mgr, id, event);
                        return match Response::try_from(r) {
                            Ok(r) => r,
                            Err(msg) => {
                                log::trace!(
                                    "Received by {} from {}: {:?}",
                                    self.id(),
                                    id,
                                    kas::util::TryFormat(&msg)
                                );
                                Response::Msg(msg)
                            }
                        };
                    }
                }
            }

            Response::Unhandled
        }
    }
}

impl<W: Widget> Grid<W> {
    /// Construct a new instance
    pub fn new(widgets: Vec<(GridChildInfo, W)>) -> Self {
        let mut grid = Grid {
            widgets,
            ..Default::default()
        };
        grid.calc_dim();
        grid
    }

    fn calc_dim(&mut self) {
        let (mut cols, mut rows) = (0, 0);
        let (mut col_spans, mut row_spans) = (0, 0);
        for child in &self.widgets {
            cols = cols.max(child.0.col_end);
            rows = rows.max(child.0.row_end);
            if child.0.col_end - child.0.col > 1 {
                col_spans += 1;
            }
            if child.0.row_end - child.0.row > 1 {
                row_spans += 1;
            }
        }
        self.dim = (cols, rows, col_spans, row_spans);
    }

    /// Construct via a builder
    pub fn build<F: FnOnce(GridBuilder<W>)>(f: F) -> Self {
        let mut grid = Self::default();
        let _ = grid.edit(f);
        grid
    }

    /// Edit an existing grid via a builder
    pub fn edit<F: FnOnce(GridBuilder<W>)>(&mut self, f: F) -> TkAction {
        f(GridBuilder(&mut self.widgets));
        self.calc_dim();
        TkAction::RECONFIGURE // just assume this is requried
    }

    /// True if there are no child widgets
    pub fn is_empty(&self) -> bool {
        self.widgets.is_empty()
    }

    /// Returns the number of child widgets
    pub fn len(&self) -> usize {
        self.widgets.len()
    }

    /// Iterate over childern
    pub fn iter(&self) -> impl Iterator<Item = &(GridChildInfo, W)> {
        ListIter {
            list: &self.widgets,
        }
    }

    /// Mutably iterate over childern
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut (GridChildInfo, W)> {
        ListIterMut {
            list: &mut self.widgets,
        }
    }
}

pub struct GridBuilder<'a, W: Widget>(&'a mut Vec<(GridChildInfo, W)>);
impl<'a, W: Widget> GridBuilder<'a, W> {
    /// True if there are no child widgets
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the number of child widgets
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns the number of elements the vector can hold without reallocating.
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }

    /// Reserves capacity for at least `additional` more elements to be inserted
    /// into the list. See documentation of [`Vec::reserve`].
    pub fn reserve(&mut self, additional: usize) {
        self.0.reserve(additional);
    }

    /// Remove all child widgets
    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Add a child widget
    ///
    /// The child is added to the end of the "list", thus appears last in
    /// navigation order.
    pub fn push(&mut self, info: GridChildInfo, widget: W) {
        self.0.push((info, widget));
    }

    /// Add a child widget to the given cell
    ///
    /// The child is added to the end of the "list", thus appears last in
    /// navigation order.
    pub fn push_cell(&mut self, col: u32, row: u32, widget: W) {
        let info = GridChildInfo::new(col, row);
        self.push(info, widget);
    }

    /// Add a child widget to the given cell, builder style
    ///
    /// The child is added to the end of the "list", thus appears last in
    /// navigation order.
    pub fn with_cell(self, col: u32, row: u32, widget: W) -> Self {
        self.with_cell_span(col, row, 1, 1, widget)
    }

    /// Add a child widget to the given cell, with spans
    ///
    /// Parameters `col_span` and `row_span` are the number of columns/rows
    /// spanned and should each be at least 1.
    ///
    /// The child is added to the end of the "list", thus appears last in
    /// navigation order.
    pub fn push_cell_span(&mut self, col: u32, row: u32, col_span: u32, row_span: u32, widget: W) {
        let info = GridChildInfo {
            col,
            col_end: col + col_span,
            row,
            row_end: row + row_span,
        };
        self.push(info, widget);
    }

    /// Add a child widget to the given cell, with spans, builder style
    ///
    /// Parameters `col_span` and `row_span` are the number of columns/rows
    /// spanned and should each be at least 1.
    ///
    /// The child is added to the end of the "list", thus appears last in
    /// navigation order.
    pub fn with_cell_span(
        mut self,
        col: u32,
        row: u32,
        col_span: u32,
        row_span: u32,
        widget: W,
    ) -> Self {
        self.push_cell_span(col, row, col_span, row_span, widget);
        self
    }

    /// Remove the last child widget
    ///
    /// Returns `None` if there are no children. Otherwise, this
    /// triggers a reconfigure before the next draw operation.
    pub fn pop(&mut self) -> Option<(GridChildInfo, W)> {
        self.0.pop()
    }

    /// Inserts a child widget position `index`
    ///
    /// Panics if `index > len`.
    pub fn insert(&mut self, index: usize, info: GridChildInfo, widget: W) {
        self.0.insert(index, (info, widget));
    }

    /// Removes the child widget at position `index`
    ///
    /// Panics if `index` is out of bounds.
    pub fn remove(&mut self, index: usize) -> (GridChildInfo, W) {
        self.0.remove(index)
    }

    /// Replace the child at `index`
    ///
    /// Panics if `index` is out of bounds.
    pub fn replace(&mut self, index: usize, info: GridChildInfo, widget: W) -> (GridChildInfo, W) {
        let mut item = (info, widget);
        std::mem::swap(&mut item, &mut self.0[index]);
        item
    }

    /// Append child widgets from an iterator
    pub fn extend<T: IntoIterator<Item = (GridChildInfo, W)>>(&mut self, iter: T) {
        self.0.extend(iter);
    }

    /// Resize, using the given closure to construct new widgets
    pub fn resize_with<F: Fn(usize) -> (GridChildInfo, W)>(&mut self, len: usize, f: F) {
        let l0 = self.0.len();
        if l0 > len {
            self.0.truncate(len);
        } else if l0 < len {
            self.0.reserve(len);
            for i in l0..len {
                self.0.push(f(i));
            }
        }
    }

    /// Retain only widgets satisfying predicate `f`
    ///
    /// See documentation of [`Vec::retain`].
    pub fn retain<F: FnMut(&(GridChildInfo, W)) -> bool>(&mut self, f: F) {
        self.0.retain(f);
    }

    /// Get the first index of a child occupying the given cell, if any
    pub fn find_child_cell(&self, col: u32, row: u32) -> Option<usize> {
        for (i, (info, _)) in self.0.iter().enumerate() {
            if info.col <= col && col < info.col_end && info.row <= row && row < info.row_end {
                return Some(i);
            }
        }
        None
    }

    /// Iterate over childern
    pub fn iter(&self) -> impl Iterator<Item = &(GridChildInfo, W)> {
        ListIter { list: self.0 }
    }

    /// Mutably iterate over childern
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut (GridChildInfo, W)> {
        ListIterMut { list: self.0 }
    }
}

impl<W: Widget> Index<usize> for Grid<W> {
    type Output = (GridChildInfo, W);

    fn index(&self, index: usize) -> &Self::Output {
        &self.widgets[index]
    }
}

impl<W: Widget> IndexMut<usize> for Grid<W> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.widgets[index]
    }
}

struct ListIter<'a, W: Widget> {
    list: &'a [(GridChildInfo, W)],
}
impl<'a, W: Widget> Iterator for ListIter<'a, W> {
    type Item = &'a (GridChildInfo, W);
    fn next(&mut self) -> Option<Self::Item> {
        if let Some((first, rest)) = self.list.split_first() {
            self.list = rest;
            Some(first)
        } else {
            None
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}
impl<'a, W: Widget> ExactSizeIterator for ListIter<'a, W> {
    fn len(&self) -> usize {
        self.list.len()
    }
}

struct ListIterMut<'a, W: Widget> {
    list: &'a mut [(GridChildInfo, W)],
}
impl<'a, W: Widget> Iterator for ListIterMut<'a, W> {
    type Item = &'a mut (GridChildInfo, W);
    fn next(&mut self) -> Option<Self::Item> {
        let list = std::mem::take(&mut self.list);
        if let Some((first, rest)) = list.split_first_mut() {
            self.list = rest;
            Some(first)
        } else {
            None
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}
impl<'a, W: Widget> ExactSizeIterator for ListIterMut<'a, W> {
    fn len(&self) -> usize {
        self.list.len()
    }
}
