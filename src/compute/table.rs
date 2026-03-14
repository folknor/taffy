//! Computes CSS Table layout (CSS 2.1 §17)
//!
//! Implements the automatic table layout algorithm with support for:
//! - Column count determination
//! - Column width resolution (auto, fixed px, percentage)
//! - Row height computation
//! - Cell placement with colspan support
//! - Border-spacing (cellspacing)
//!
//! ## Limitations
//!
//! - `table-layout: fixed` is partially implemented: fixed-width columns skip the
//!   min-content-width floor, but the algorithm still scans all rows (not just the
//!   first) for column width hints, and auto/percentage columns still use content
//!   widths. A full CSS 2.1 §17.5.2.1 fixed-layout implementation would determine
//!   column widths from the first row only.

use crate::geometry::{Line, Point, Size};
use crate::style::{AvailableSpace, CoreStyle, Overflow, TableContainerStyle, TableItemStyle, TableLayout};
use crate::style::CompactLength;
use crate::tree::{Layout, LayoutInput, LayoutOutput, NodeId, RunMode, SizingMode};
use crate::tree::traits::LayoutTableContainer;
use crate::util::sys::Vec;
use crate::util::MaybeMath;
use crate::util::ResolveOrZero;
use crate::{BoxSizing, MaybeResolve};
use crate::tree::traits::LayoutPartialTreeExt;

/// A resolved cell in the table
struct TableCell {
    /// The node id of the cell
    node_id: NodeId,
    /// Column index (0-based)
    col_start: usize,
    /// Number of columns spanned
    colspan: usize,
    /// Row index (0-based)
    row_index: usize,
    /// Child index within the parent row (for layout order)
    cell_index: usize,
}

/// Information about a column
#[derive(Clone)]
struct ColumnInfo {
    /// The width type for this column
    width_type: ColumnWidthType,
    /// Minimum content width
    min_content_width: f32,
    /// Maximum content width
    max_content_width: f32,
    /// Resolved width (after algorithm runs)
    resolved_width: f32,
}

/// How a column's width is specified
#[derive(Clone, Debug)]
enum ColumnWidthType {
    /// Width determined by content
    Auto,
    /// Fixed pixel width
    Fixed(f32),
    /// Percentage of table width
    Percent(f32),
}

/// Compute the layout of a table container and its children
pub fn compute_table_layout(
    tree: &mut impl LayoutTableContainer,
    node_id: NodeId,
    inputs: LayoutInput,
) -> LayoutOutput {
    let LayoutInput {
        known_dimensions,
        parent_size,
        available_space,
        run_mode,
        ..
    } = inputs;

    let style = tree.get_table_container_style(node_id);
    let raw_padding = style.padding();
    let raw_border = style.border();
    let raw_margin = style.margin();
    let raw_size = style.size();
    let raw_min_size = style.min_size();
    let raw_max_size = style.max_size();
    let box_sizing = style.box_sizing();
    let aspect_ratio = style.aspect_ratio();
    let border_spacing = style.border_spacing();
    let table_layout = style.table_layout();
    drop(style);

    let parent_width = parent_size.width;

    let padding = raw_padding.resolve_or_zero(parent_width, |v, b| tree.calc(v, b));
    let border = raw_border.resolve_or_zero(parent_width, |v, b| tree.calc(v, b));
    let margin = raw_margin.resolve_or_zero(parent_width, |v, b| tree.calc(v, b));
    let padding_border = padding + border;
    let padding_border_size = padding_border.sum_axes();

    let box_sizing_adjustment = if box_sizing == BoxSizing::ContentBox { padding_border_size } else { Size::ZERO };

    let min_size = raw_min_size
        .maybe_resolve(parent_size, |v, b| tree.calc(v, b))
        .maybe_apply_aspect_ratio(aspect_ratio)
        .maybe_add(box_sizing_adjustment);
    let max_size = raw_max_size
        .maybe_resolve(parent_size, |v, b| tree.calc(v, b))
        .maybe_apply_aspect_ratio(aspect_ratio)
        .maybe_add(box_sizing_adjustment);
    let specified_size = raw_size
        .maybe_resolve(parent_size, |v, b| tree.calc(v, b))
        .maybe_apply_aspect_ratio(aspect_ratio)
        .maybe_add(box_sizing_adjustment)
        .maybe_clamp(min_size, max_size);

    let styled_known_dimensions = known_dimensions.or(specified_size);

    // Resolve border-spacing
    let h_spacing = border_spacing.width.resolve_or_zero(parent_width, |v, b| tree.calc(v, b));
    let v_spacing = border_spacing.height.resolve_or_zero(parent_width, |v, b| tree.calc(v, b));

    // Phase 1: Gather table structure (rows, cells)
    let child_count = tree.child_count(node_id);
    let mut rows: Vec<NodeId> = Vec::new();
    let mut cells: Vec<TableCell> = Vec::new();
    let mut max_columns: usize = 0;

    for child_idx in 0..child_count {
        let child_id = tree.get_child_id(node_id, child_idx);
        let child_style = tree.get_table_child_style(child_id);
        let is_row = child_style.is_table_row();
        let is_row_group = child_style.is_table_row_group();
        drop(child_style);

        if is_row {
            let row_index = rows.len();
            rows.push(child_id);
            collect_cells_from_row(tree, child_id, row_index, &mut cells, &mut max_columns);
        } else if is_row_group {
            // Row groups contain rows
            let group_child_count = tree.child_count(child_id);
            for group_child_idx in 0..group_child_count {
                let row_id = tree.get_child_id(child_id, group_child_idx);
                let row_index = rows.len();
                rows.push(row_id);
                collect_cells_from_row(tree, row_id, row_index, &mut cells, &mut max_columns);
            }
        } else {
            // Check if the child is a direct TableCell (CSS anonymous table object case).
            // Per CSS 2.1 §17.2.1, consecutive cells not wrapped in a row should be
            // grouped into an anonymous table-row. We handle this by treating each
            // direct cell as a single-cell row where the cell node doubles as the row.
            let child_style_2 = tree.get_table_child_style(child_id);
            let is_cell = child_style_2.is_table_cell();
            drop(child_style_2);

            if is_cell {
                let row_index = rows.len();
                rows.push(child_id);

                let cell_style_2 = tree.get_table_child_style(child_id);
                let colspan = cell_style_2.colspan().max(1) as usize;
                drop(cell_style_2);

                cells.push(TableCell {
                    node_id: child_id,
                    col_start: 0,
                    colspan,
                    row_index,
                    cell_index: 0,
                });

                if colspan > max_columns {
                    max_columns = colspan;
                }
            } else {
                // Non-cell, non-row, non-row-group child — treat as anonymous row
                let row_index = rows.len();
                rows.push(child_id);
                collect_cells_from_row(tree, child_id, row_index, &mut cells, &mut max_columns);
            }
        }
    }

    if max_columns == 0 || rows.is_empty() {
        // Empty table
        let size = Size {
            width: styled_known_dimensions.width.unwrap_or(padding_border_size.width)
                .maybe_clamp(min_size.width, max_size.width),
            height: styled_known_dimensions.height.unwrap_or(padding_border_size.height)
                .maybe_clamp(min_size.height, max_size.height),
        };

        if run_mode == RunMode::PerformLayout {
            for child_idx in 0..child_count {
                let child_id = tree.get_child_id(node_id, child_idx);
                tree.set_unrounded_layout(child_id, &Layout::with_order(child_idx as u32));
            }
        }

        return LayoutOutput::from_outer_size(size);
    }

    // Phase 2: Determine column widths
    let mut columns: Vec<ColumnInfo> = (0..max_columns)
        .map(|_| ColumnInfo {
            width_type: ColumnWidthType::Auto,
            min_content_width: 0.0,
            max_content_width: 0.0,
            resolved_width: 0.0,
        })
        .collect();

    // Scan cells to determine column width types and intrinsic sizes
    for cell in &cells {
        if cell.colspan > 1 {
            continue; // Handle single-column cells first
        }

        let col = cell.col_start;
        if col >= max_columns {
            continue;
        }

        let cell_core = tree.get_core_container_style(cell.node_id);
        let cell_size = cell_core.size();
        let cell_padding = cell_core.padding().resolve_or_zero(parent_width, |v, b| tree.calc(v, b));
        let cell_border = cell_core.border().resolve_or_zero(parent_width, |v, b| tree.calc(v, b));
        let cell_pb = (cell_padding + cell_border).horizontal_axis_sum();

        // Determine column width type from cell's specified width
        let width_dim = cell_size.width;
        let width_tag = width_dim.tag();
        let cell_box_sizing = cell_core.box_sizing();
        drop(cell_core);

        if width_tag == CompactLength::LENGTH_TAG {
            if let ColumnWidthType::Auto = columns[col].width_type {
                // resolved_width represents the full column width (including cell padding/border),
                // so for content-box cells we must add cell_pb to the CSS width value.
                let fixed_w = if cell_box_sizing == BoxSizing::ContentBox {
                    width_dim.value() + cell_pb
                } else {
                    width_dim.value()
                };
                columns[col].width_type = ColumnWidthType::Fixed(fixed_w);
            }
        } else if width_tag == CompactLength::PERCENT_TAG {
            if let ColumnWidthType::Auto = columns[col].width_type {
                columns[col].width_type = ColumnWidthType::Percent(width_dim.value());
            }
        }

        // Measure intrinsic cell size.
        // measure_child_size_both with SizingMode::ContentSize returns the outer size
        // (including padding/border), so we must NOT add cell_pb again.
        let cell_intrinsic = tree.measure_child_size_both(
            cell.node_id,
            Size::NONE,
            parent_size,
            Size { width: AvailableSpace::MinContent, height: AvailableSpace::MinContent },
            SizingMode::ContentSize,
            Line::FALSE,
        );

        let min_w = cell_intrinsic.width;
        if min_w > columns[col].min_content_width {
            columns[col].min_content_width = min_w;
        }

        let cell_max_intrinsic = tree.measure_child_size_both(
            cell.node_id,
            Size::NONE,
            parent_size,
            Size { width: AvailableSpace::MaxContent, height: AvailableSpace::MaxContent },
            SizingMode::ContentSize,
            Line::FALSE,
        );

        let max_w = cell_max_intrinsic.width;
        if max_w > columns[col].max_content_width {
            columns[col].max_content_width = max_w;
        }
    }

    // Determine available width for columns
    let total_spacing = h_spacing * (max_columns as f32 + 1.0);
    let available_for_columns = match styled_known_dimensions.width {
        Some(w) => w - padding_border_size.width - total_spacing,
        None => match available_space.width {
            AvailableSpace::Definite(w) => w - padding_border_size.width - total_spacing - margin.horizontal_axis_sum(),
            AvailableSpace::MaxContent => f32::INFINITY,
            AvailableSpace::MinContent => 0.0,
        },
    };

    // Resolve column widths
    let table_has_explicit_width = styled_known_dimensions.width.is_some();
    let is_fixed_layout = table_layout == TableLayout::Fixed;
    resolve_column_widths(&mut columns, available_for_columns, table_has_explicit_width, is_fixed_layout);

    // Handle colspan: distribute extra width needed
    for cell in &cells {
        if cell.colspan <= 1 {
            continue;
        }

        let col_end = (cell.col_start + cell.colspan).min(max_columns);
        let spanned_width: f32 = (cell.col_start..col_end).map(|c| columns[c].resolved_width).sum::<f32>()
            + h_spacing * (cell.colspan as f32 - 1.0);

        // Measure cell min content
        let cell_intrinsic = tree.measure_child_size_both(
            cell.node_id,
            Size::NONE,
            parent_size,
            Size { width: AvailableSpace::MinContent, height: AvailableSpace::MinContent },
            SizingMode::ContentSize,
            Line::FALSE,
        );

        // cell_intrinsic already includes padding/border (outer size)
        let needed = cell_intrinsic.width;

        if needed > spanned_width {
            let extra = needed - spanned_width;
            let auto_cols: Vec<usize> = (cell.col_start..col_end)
                .filter(|&c| matches!(columns[c].width_type, ColumnWidthType::Auto))
                .collect();

            if !auto_cols.is_empty() {
                let per_col = extra / auto_cols.len() as f32;
                for &c in &auto_cols {
                    columns[c].resolved_width += per_col;
                }
            } else {
                let per_col = extra / (col_end - cell.col_start) as f32;
                for c in cell.col_start..col_end {
                    columns[c].resolved_width += per_col;
                }
            }
        }
    }

    let total_columns_width: f32 = columns.iter().map(|c| c.resolved_width).sum();
    let table_content_width = total_columns_width + total_spacing;
    let table_width = styled_known_dimensions.width.unwrap_or(
        (table_content_width + padding_border_size.width)
            .maybe_clamp(min_size.width, max_size.width)
    );

    // If the table has a known width larger than needed, redistribute extra space
    let actual_content_width = table_width - padding_border_size.width;
    if actual_content_width > table_content_width {
        let extra = actual_content_width - table_content_width;
        let auto_cols: Vec<usize> = (0..max_columns)
            .filter(|&c| matches!(columns[c].width_type, ColumnWidthType::Auto))
            .collect();

        if !auto_cols.is_empty() {
            let per_col = extra / auto_cols.len() as f32;
            for &c in &auto_cols {
                columns[c].resolved_width += per_col;
            }
        } else if max_columns > 0 {
            let per_col = extra / max_columns as f32;
            for col in &mut columns {
                col.resolved_width += per_col;
            }
        }
    }

    // Phase 3: Compute row heights by laying out cells with resolved column widths
    let num_rows = rows.len();
    let mut row_heights: Vec<f32> = {
        let mut v = Vec::new();
        for _ in 0..num_rows {
            v.push(0.0);
        }
        v
    };

    for cell in &cells {
        let col_end = (cell.col_start + cell.colspan).min(max_columns);
        let cell_width: f32 = (cell.col_start..col_end).map(|c| columns[c].resolved_width).sum::<f32>()
            + if cell.colspan > 1 { h_spacing * (cell.colspan as f32 - 1.0) } else { 0.0 };

        let cell_output = tree.perform_child_layout(
            cell.node_id,
            Size { width: Some(cell_width), height: None },
            Size { width: Some(table_width), height: parent_size.height },
            Size { width: AvailableSpace::Definite(cell_width), height: AvailableSpace::MaxContent },
            SizingMode::InherentSize,
            Line::FALSE,
        );

        if cell.row_index < num_rows && cell_output.size.height > row_heights[cell.row_index] {
            row_heights[cell.row_index] = cell_output.size.height;
        }
    }

    let total_row_height: f32 = row_heights.iter().sum();
    let total_v_spacing = v_spacing * (num_rows as f32 + 1.0);
    let table_content_height = total_row_height + total_v_spacing;
    let table_height = styled_known_dimensions.height.unwrap_or(
        (table_content_height + padding_border_size.height)
            .maybe_clamp(min_size.height, max_size.height)
    );

    let final_size = Size { width: table_width, height: table_height };

    if run_mode == RunMode::ComputeSize {
        return LayoutOutput::from_outer_size(final_size);
    }

    // Phase 4: Position cells
    if run_mode == RunMode::PerformLayout {
        // Compute column x-offsets
        let mut col_x_offsets: Vec<f32> = Vec::with_capacity(max_columns);
        let mut x = padding_border.left + h_spacing;
        for col in &columns {
            col_x_offsets.push(x);
            x += col.resolved_width + h_spacing;
        }

        // Compute row y-offsets
        let mut row_y_offsets: Vec<f32> = Vec::with_capacity(num_rows);
        let mut y = padding_border.top + v_spacing;
        for &rh in &row_heights {
            row_y_offsets.push(y);
            y += rh + v_spacing;
        }

        // Position each cell
        for cell in &cells {
            if cell.row_index >= num_rows || cell.col_start >= max_columns {
                continue;
            }

            let col_end = (cell.col_start + cell.colspan).min(max_columns);
            let cell_width: f32 = (cell.col_start..col_end).map(|c| columns[c].resolved_width).sum::<f32>()
                + if cell.colspan > 1 { h_spacing * (cell.colspan as f32 - 1.0) } else { 0.0 };
            let cell_height = row_heights[cell.row_index];

            // Re-layout the cell with final dimensions
            let _cell_output = tree.perform_child_layout(
                cell.node_id,
                Size { width: Some(cell_width), height: Some(cell_height) },
                Size { width: Some(table_width), height: Some(table_height) },
                Size {
                    width: AvailableSpace::Definite(cell_width),
                    height: AvailableSpace::Definite(cell_height),
                },
                SizingMode::InherentSize,
                Line::FALSE,
            );

            let cell_style = tree.get_core_container_style(cell.node_id);
            let cell_padding = cell_style.padding().resolve_or_zero(parent_width, |v, b| tree.calc(v, b));
            let cell_border_val = cell_style.border().resolve_or_zero(parent_width, |v, b| tree.calc(v, b));
            let cell_margin = cell_style.margin().resolve_or_zero(parent_width, |v, b| tree.calc(v, b));
            let scrollbar_size = Size {
                width: if cell_style.overflow().y == Overflow::Scroll { cell_style.scrollbar_width() } else { 0.0 },
                height: if cell_style.overflow().x == Overflow::Scroll { cell_style.scrollbar_width() } else { 0.0 },
            };
            drop(cell_style);

            tree.set_unrounded_layout(
                cell.node_id,
                &Layout {
                    order: cell.cell_index as u32,
                    location: Point {
                        x: col_x_offsets[cell.col_start] - padding_border.left,
                        y: 0.0,
                    },
                    size: Size { width: cell_width, height: cell_height },
                    #[cfg(feature = "content_size")]
                    content_size: Size::ZERO,
                    scrollbar_size,
                    padding: cell_padding,
                    border: cell_border_val,
                    margin: cell_margin,
                },
            );
        }

        // Build a mapping from row index to parent row group's start_y offset.
        // Rows inside a row group need their location relative to the group, not the table.
        let mut row_parent_offset_y: Vec<f32> = vec![0.0; rows.len()];
        let mut row_parent_offset_x: Vec<f32> = vec![0.0; rows.len()];

        // Set layouts for row group nodes (do this first so we know parent offsets for rows)
        for child_idx in 0..child_count {
            let child_id = tree.get_child_id(node_id, child_idx);
            let child_style = tree.get_table_child_style(child_id);
            let is_row_group = child_style.is_table_row_group();
            drop(child_style);

            if is_row_group {
                let group_child_count = tree.child_count(child_id);
                if group_child_count == 0 {
                    tree.set_unrounded_layout(child_id, &Layout::with_order(child_idx as u32));
                    continue;
                }

                // Find the y range of rows in this group
                let mut start_y = padding_border.top + v_spacing;
                let mut end_y = start_y;
                for gi in 0..group_child_count {
                    let row_in_group = tree.get_child_id(child_id, gi);
                    if let Some(ri) = rows.iter().position(|&r| r == row_in_group) {
                        let ry = row_y_offsets.get(ri).copied().unwrap_or(0.0);
                        let rh = row_heights.get(ri).copied().unwrap_or(0.0);
                        if gi == 0 {
                            start_y = ry;
                        }
                        end_y = ry + rh;
                        // Record parent offset so row position can be made relative
                        row_parent_offset_y[ri] = start_y;
                        row_parent_offset_x[ri] = padding_border.left;
                    }
                }

                let child_s = tree.get_core_container_style(child_id);
                let group_padding = child_s.padding().resolve_or_zero(parent_width, |v, b| tree.calc(v, b));
                let group_border = child_s.border().resolve_or_zero(parent_width, |v, b| tree.calc(v, b));
                let group_margin = child_s.margin().resolve_or_zero(parent_width, |v, b| tree.calc(v, b));
                drop(child_s);

                tree.set_unrounded_layout(
                    child_id,
                    &Layout {
                        order: child_idx as u32,
                        location: Point { x: padding_border.left, y: start_y },
                        size: Size {
                            width: table_width - padding_border_size.width,
                            height: end_y - start_y,
                        },
                        #[cfg(feature = "content_size")]
                        content_size: Size::ZERO,
                        scrollbar_size: Size::ZERO,
                        padding: group_padding,
                        border: group_border,
                        margin: group_margin,
                    },
                );
            }
        }

        // Set layouts for row nodes
        // Row locations are relative to their parent (row group or table)
        for (row_idx, &row_id) in rows.iter().enumerate() {
            let row_y = row_y_offsets.get(row_idx).copied().unwrap_or(0.0);
            let row_h = row_heights.get(row_idx).copied().unwrap_or(0.0);
            let row_w = table_width - padding_border_size.width;

            let row_style = tree.get_core_container_style(row_id);
            let row_padding = row_style.padding().resolve_or_zero(parent_width, |v, b| tree.calc(v, b));
            let row_border = row_style.border().resolve_or_zero(parent_width, |v, b| tree.calc(v, b));
            let row_margin = row_style.margin().resolve_or_zero(parent_width, |v, b| tree.calc(v, b));
            drop(row_style);

            // Subtract parent row group offset to make position relative to parent
            let relative_y = row_y - row_parent_offset_y[row_idx];
            let relative_x = padding_border.left - row_parent_offset_x[row_idx];

            tree.set_unrounded_layout(
                row_id,
                &Layout {
                    order: row_idx as u32,
                    location: Point { x: relative_x, y: relative_y },
                    size: Size { width: row_w, height: row_h },
                    #[cfg(feature = "content_size")]
                    content_size: Size::ZERO,
                    scrollbar_size: Size::ZERO,
                    padding: row_padding,
                    border: row_border,
                    margin: row_margin,
                },
            );
        }
    }

    LayoutOutput::from_outer_size(final_size)
}

/// Collect cells from a row node's children
fn collect_cells_from_row(
    tree: &mut impl LayoutTableContainer,
    row_id: NodeId,
    row_index: usize,
    cells: &mut Vec<TableCell>,
    max_columns: &mut usize,
) {
    let cell_count = tree.child_count(row_id);
    let mut col = 0;

    for cell_idx in 0..cell_count {
        let cell_id = tree.get_child_id(row_id, cell_idx);
        let cell_style = tree.get_table_child_style(cell_id);
        let colspan = cell_style.colspan().max(1) as usize;
        drop(cell_style);

        cells.push(TableCell {
            node_id: cell_id,
            col_start: col,
            colspan,
            row_index,
            cell_index: cell_idx,
        });

        col += colspan;
    }

    if col > *max_columns {
        *max_columns = col;
    }
}

/// Resolve column widths using the automatic table layout algorithm.
/// When `has_explicit_width` is true, auto columns fill available space.
/// When false (auto-width table), auto columns use their max-content width.
/// When `is_fixed_layout` is true (table-layout: fixed), fixed-width columns use their
/// specified width exactly without the min_content_width floor.
fn resolve_column_widths(columns: &mut [ColumnInfo], available_width: f32, has_explicit_width: bool, is_fixed_layout: bool) {
    let num_columns = columns.len();
    if num_columns == 0 {
        return;
    }

    // Step 1: Assign fixed and percentage widths
    let mut remaining = available_width;
    let mut auto_count = 0;

    for col in columns.iter_mut() {
        match col.width_type {
            ColumnWidthType::Fixed(w) => {
                // With table-layout: fixed, use specified width exactly (content may overflow/clip).
                // With table-layout: auto, ensure column is at least as wide as min content.
                col.resolved_width = if is_fixed_layout { w } else { w.max(col.min_content_width) };
                remaining -= col.resolved_width;
            }
            ColumnWidthType::Percent(pct) => {
                let w = if available_width.is_finite() {
                    (available_width * pct).max(col.min_content_width)
                } else {
                    col.max_content_width.max(col.min_content_width)
                };
                col.resolved_width = w;
                remaining -= col.resolved_width;
            }
            ColumnWidthType::Auto => {
                auto_count += 1;
            }
        }
    }

    // Step 2: Distribute remaining space to auto columns
    if auto_count > 0 {
        if !has_explicit_width {
            // Auto-width table: columns use their max-content width (shrink to fit)
            for col in columns.iter_mut() {
                if matches!(col.width_type, ColumnWidthType::Auto) {
                    col.resolved_width = col.max_content_width.max(col.min_content_width);
                }
            }
        } else if remaining > 0.0 && available_width.is_finite() {
            // Explicit-width table: distribute remaining space proportionally
            let total_max_content: f32 = columns
                .iter()
                .filter(|c| matches!(c.width_type, ColumnWidthType::Auto))
                .map(|c| c.max_content_width.max(1.0))
                .sum();

            if total_max_content > 0.0 {
                for col in columns.iter_mut() {
                    if matches!(col.width_type, ColumnWidthType::Auto) {
                        let proportion = col.max_content_width.max(1.0) / total_max_content;
                        col.resolved_width = remaining * proportion;
                    }
                }
            } else {
                let per_col = remaining / auto_count as f32;
                for col in columns.iter_mut() {
                    if matches!(col.width_type, ColumnWidthType::Auto) {
                        col.resolved_width = per_col;
                    }
                }
            }
        } else {
            for col in columns.iter_mut() {
                if matches!(col.width_type, ColumnWidthType::Auto) {
                    col.resolved_width = col.max_content_width.max(col.min_content_width);
                }
            }
        }
    }
}
