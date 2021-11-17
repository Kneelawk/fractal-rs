//! This module contains the [`RowStitcher`] and associated things.
#![allow(dead_code)]

use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    task::Poll,
};

use crate::generator::{util::copy_region, view::View, PixelBlock, BYTES_PER_PIXEL};

/// Stitches blocks of pixels together into complete rows.
pub struct RowStitcher {
    parent: View,
    remaining_views: HashMap<ViewWrapper, usize>,
    remaining_blocks: Vec<Option<PixelBlock>>,
}

impl RowStitcher {
    /// Creates a new RowStitcher for stitching the children into rows of the
    /// parent.
    ///
    /// ## Panics
    /// * if `children` is empty.
    /// * if different children in the same row have different heights.
    pub fn new(parent: View, children: &[View]) -> RowStitcher {
        let len = children.len();
        if len == 0 {
            panic!("RowStitcher constructed with no view children");
        }

        let mut views = HashMap::new();
        let mut row_height = 0;
        let mut recalculate_row_height = true;
        for (index, child) in children.iter().enumerate() {
            // Store blocks in reverse order because we will be removing the first blocks
            // first.
            views.insert(ViewWrapper(*child), len - index - 1);

            if recalculate_row_height {
                row_height = child.image_height;
            } else {
                if row_height != child.image_height {
                    panic!("RowStitcher encountered row with different view heights");
                }
            }

            if child.image_x + child.image_width == parent.image_x + parent.image_width {
                // We've reached the end of a row, so the next row's height might be different.
                recalculate_row_height = true;
            } else {
                recalculate_row_height = false;
            }
        }

        let blocks = vec![None; len];
        RowStitcher {
            parent,
            remaining_views: views,
            remaining_blocks: blocks,
        }
    }

    /// Inserts a fractal generation message into this row stitcher at its
    /// specified location.
    pub fn insert(&mut self, message: PixelBlock) {
        // TODO: Add errors for this method.

        // Note: this is using the reverse-order index, which means that the first pixel
        // blocks will be stored at the end of the array.
        let index = self.remaining_views.get(&ViewWrapper(message.view));

        if let Some(&index) = index {
            self.remaining_blocks[index] = Some(message);
        }
    }

    /// Stitches all the currently contiguous pixel blocks together into a
    /// single row pixel block.
    ///
    /// This returns:
    /// * `Poll::Pending` if there are not enough contiguous pixel blocks for a
    ///   complete row.
    /// * `Poll::Ready(Some(block))` if a complete row of pixel blocks is
    ///   available.
    /// * `Poll::Ready(None)` if this row stitcher has stitched all rows in its
    ///   view and cannot stitch any more.
    pub fn stitch(&mut self) -> Poll<Option<PixelBlock>> {
        if self.remaining_blocks.is_empty() {
            return Poll::Ready(None);
        }

        // remaining blocks should never be empty by this point
        let first_block = self.remaining_blocks.last().unwrap();
        if let Some(first_block) = first_block {
            let blocks_len = self.remaining_blocks.len();
            let mut last_block = first_block;
            let mut last_index = blocks_len - 1;
            while last_block.view.image_x + last_block.view.image_width
                < self.parent.image_x + self.parent.image_width
            {
                last_index -= 1;
                let maybe_block = &self.remaining_blocks[last_index];
                if let Some(block) = maybe_block {
                    last_block = block;
                } else {
                    return Poll::Pending;
                }
            }

            let mut new_image =
                vec![
                    0u8;
                    first_block.view.image_height * self.parent.image_width * BYTES_PER_PIXEL
                ];
            let new_view = View {
                image_width: self.parent.image_width,
                image_height: first_block.view.image_height,
                image_x: self.parent.image_x,
                image_y: first_block.view.image_y,
                image_scale_x: self.parent.image_scale_x,
                image_scale_y: self.parent.image_scale_y,
                plane_start_x: self.parent.plane_start_x,
                plane_start_y: first_block.view.plane_start_y,
            };

            for _ in last_index..blocks_len {
                // Iteration order doesn't matter because we're obtaining our
                // blocks by calling pop().

                // These unwraps *shouldn't* be None. Remaining blocks should never be empty by
                // this point and we just iterated through to make sure none of the elements
                // were None.
                let block = self.remaining_blocks.pop().unwrap().unwrap();

                copy_region(
                    &block.image,
                    block.view.image_width,
                    0,
                    0,
                    &mut new_image,
                    new_view.image_width,
                    block.view.image_x - self.parent.image_x,
                    0,
                    block.view.image_width,
                    block.view.image_height,
                );

                self.remaining_views.remove(&ViewWrapper(block.view));
            }

            Poll::Ready(Some(PixelBlock {
                view: new_view,
                image: new_image.into_boxed_slice(),
            }))
        } else {
            Poll::Pending
        }
    }
}

struct ViewWrapper(View);

impl Hash for ViewWrapper {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.image_x.hash(state);
        self.0.image_y.hash(state);
    }
}

impl PartialEq for ViewWrapper {
    fn eq(&self, other: &Self) -> bool {
        self.0.image_x == other.0.image_x && self.0.image_y == other.0.image_y
    }
}

impl Eq for ViewWrapper {}
