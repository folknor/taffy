//! Computes CSS Table layout (CSS 2.1 §17)
//!
//! Implements the automatic table layout algorithm with support for:
//! - Column count determination
//! - Column width resolution (auto, fixed px, percentage) with min/max-content sizing
//! - Row height computation
//! - Cell placement with colspan support
//! - Border-spacing (cellspacing)
//! - Anonymous rows/cells for mis-parented children (CSS 2.1 §17.2.1)
//! - First-row baselines
//!
//! ## Limitations
//!
//! - No rowspan support
//! - No captions
//! - No `<col>`/`<colgroup>` width support
//! - No `border-collapse` support
//! - No `vertical-align` support in cells: cell boxes stretch to the row height and
//!   their content is top-anchored (CSS default is baseline alignment)
//! - Percentage columns are resolved against the table's used content width but do
//!   not influence the table's own width determination
//! - `table-layout: fixed` is partially implemented: fixed-width and percentage
//!   columns use their specified width exactly (no min-content floor), but the
//!   algorithm still scans all rows (not just the first) for column width hints. A
//!   full CSS 2.1 §17.5.2.1 fixed-layout implementation would determine column
//!   widths from the first row only.
//! - Children of row groups are assumed to be rows (no anonymous box fix-up inside
//!   row groups)

#[cfg(feature = "content_size")]
use crate::compute::common::content_size::compute_content_size_contribution;
use crate::geometry::{Line, Point, Size};
use crate::style::{
    AvailableSpace, CompactLength, CoreStyle, Overflow, TableContainerStyle, TableItemStyle, TableLayout,
};
use crate::tree::traits::{LayoutPartialTreeExt, LayoutTableContainer};
use crate::tree::{Layout, LayoutInput, LayoutOutput, NodeId, RequestedAxis, RunMode, SizingMode};
use crate::util::sys::{f32_max, Vec};
use crate::util::{MaybeMath, ResolveOrZero};
use crate::{BoxSizing, MaybeResolve};

/// A row in the table grid. Anonymous rows (generated for cells that are direct
/// children of the table, per CSS 2.1 §17.2.1) have no backing node.
struct TableRowEntry {
    /// The node id of the row, or `None` for an anonymous row
    node: Option<NodeId>,
    /// Child index within the row's parent (used as the layout `order`)
    order: u32,
}

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
    /// Child index within the cell's parent (used as the layout `order`)
    order: u32,
    /// Whether the cell's parent node is the table itself (anonymous row member).
    /// Such cells are positioned in table coordinates rather than row coordinates.
    parent_is_table: bool,
}

/// Information about a column
#[derive(Clone)]
struct ColumnInfo {
    /// Largest fixed (px) width specified by any cell in this column (outer width)
    fixed: Option<f32>,
    /// Largest percentage width specified by any cell in this column (as a fraction)
    percent: Option<f32>,
    /// Minimum content width
    min_content_width: f32,
    /// Maximum content width
    max_content_width: f32,
    /// Resolved width (after algorithm runs)
    resolved_width: f32,
}

impl ColumnInfo {
    /// Whether no cell in this column specified a width
    fn is_auto(&self) -> bool {
        self.fixed.is_none() && self.percent.is_none()
    }
}

/// The width this column contributes to the table's minimum content width
fn col_min_w(col: &ColumnInfo, is_fixed_layout: bool) -> f32 {
    match (col.percent, col.fixed) {
        (Some(_), _) => {
            if is_fixed_layout {
                0.0
            } else {
                col.min_content_width
            }
        }
        (None, Some(fixed)) => {
            if is_fixed_layout {
                fixed
            } else {
                f32_max(fixed, col.min_content_width)
            }
        }
        (None, None) => col.min_content_width,
    }
}

/// The width this column contributes to the table's maximum content width
fn col_max_w(col: &ColumnInfo, is_fixed_layout: bool) -> f32 {
    match (col.percent, col.fixed) {
        (Some(_), _) => f32_max(col.max_content_width, col.min_content_width),
        (None, Some(fixed)) => {
            if is_fixed_layout {
                fixed
            } else {
                f32_max(fixed, col.min_content_width)
            }
        }
        (None, None) => f32_max(col.max_content_width, col.min_content_width),
    }
}

/// Compute the layout of a table container and its children
pub fn compute_table_layout(
    tree: &mut impl LayoutTableContainer,
    node_id: NodeId,
    inputs: LayoutInput,
) -> LayoutOutput {
    let LayoutInput { known_dimensions, parent_size, available_space, run_mode, .. } = inputs;

    let style = tree.get_table_container_style(node_id);
    let raw_padding = style.padding();
    let raw_border = style.border();
    let raw_size = style.size();
    let raw_min_size = style.min_size();
    let raw_max_size = style.max_size();
    let box_sizing = style.box_sizing();
    let aspect_ratio = style.aspect_ratio();
    let border_spacing = style.border_spacing();
    let is_fixed_layout = style.table_layout() == TableLayout::Fixed;
    drop(style);

    let parent_width = parent_size.width;

    let padding = raw_padding.resolve_or_zero(parent_width, |v, b| tree.calc(v, b));
    let border = raw_border.resolve_or_zero(parent_width, |v, b| tree.calc(v, b));
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
    //
    // Direct children that are neither rows nor row groups become cells in an
    // anonymous row per CSS 2.1 §17.2.1: consecutive such children share one
    // anonymous row, and non-cell children are treated as anonymous cells.
    let child_count = tree.child_count(node_id);
    let mut rows: Vec<TableRowEntry> = Vec::new();
    let mut cells: Vec<TableCell> = Vec::new();
    let mut max_columns: usize = 0;
    // Index into `rows` of the anonymous row currently being accumulated, plus the
    // next free column within it. Reset whenever a real row or row group appears.
    let mut open_anonymous_row: Option<usize> = None;
    let mut anonymous_col: usize = 0;

    for child_idx in 0..child_count {
        let child_id = tree.get_child_id(node_id, child_idx);
        let child_style = tree.get_table_child_style(child_id);
        let is_row = child_style.is_table_row();
        let is_row_group = child_style.is_table_row_group();
        let colspan = if child_style.is_table_cell() { child_style.colspan().max(1) as usize } else { 1 };
        drop(child_style);

        if is_row {
            open_anonymous_row = None;
            let row_index = rows.len();
            rows.push(TableRowEntry { node: Some(child_id), order: child_idx as u32 });
            collect_cells_from_row(tree, child_id, row_index, &mut cells, &mut max_columns);
        } else if is_row_group {
            open_anonymous_row = None;
            // Row groups contain rows
            let group_child_count = tree.child_count(child_id);
            for group_child_idx in 0..group_child_count {
                let row_id = tree.get_child_id(child_id, group_child_idx);
                let row_index = rows.len();
                rows.push(TableRowEntry { node: Some(row_id), order: group_child_idx as u32 });
                collect_cells_from_row(tree, row_id, row_index, &mut cells, &mut max_columns);
            }
        } else {
            // Anonymous row member: a real TableCell, or any other child which gets
            // wrapped in an anonymous cell (i.e. treated as the cell itself)
            let row_index = match open_anonymous_row {
                Some(index) => index,
                None => {
                    let index = rows.len();
                    rows.push(TableRowEntry { node: None, order: child_idx as u32 });
                    open_anonymous_row = Some(index);
                    anonymous_col = 0;
                    index
                }
            };

            cells.push(TableCell {
                node_id: child_id,
                col_start: anonymous_col,
                colspan,
                row_index,
                order: child_idx as u32,
                parent_is_table: true,
            });

            anonymous_col += colspan;
            if anonymous_col > max_columns {
                max_columns = anonymous_col;
            }
        }
    }

    if max_columns == 0 || rows.is_empty() {
        // Empty table
        let size = Size {
            width: styled_known_dimensions
                .width
                .unwrap_or(padding_border_size.width)
                .maybe_clamp(min_size.width, max_size.width),
            height: styled_known_dimensions
                .height
                .unwrap_or(padding_border_size.height)
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

    // Phase 2: Determine column intrinsic sizes and width types
    let mut columns: Vec<ColumnInfo> = (0..max_columns)
        .map(|_| ColumnInfo {
            fixed: None,
            percent: None,
            min_content_width: 0.0,
            max_content_width: 0.0,
            resolved_width: 0.0,
        })
        .collect();

    // Scan single-column cells to determine column width types and intrinsic sizes
    for cell in &cells {
        if cell.colspan > 1 {
            continue; // Spanning cells are handled below
        }

        let col = cell.col_start;
        if col >= max_columns {
            continue;
        }

        let cell_core = tree.get_core_container_style(cell.node_id);
        let width_dim = cell_core.size().width;
        let width_tag = width_dim.tag();
        let cell_box_sizing = cell_core.box_sizing();
        let cell_pb = (cell_core.padding().resolve_or_zero(parent_width, |v, b| tree.calc(v, b))
            + cell_core.border().resolve_or_zero(parent_width, |v, b| tree.calc(v, b)))
        .horizontal_axis_sum();
        drop(cell_core);

        // A column's specified width is the largest width specified by any of its
        // cells. Percentage widths take priority over fixed widths.
        if width_tag == CompactLength::LENGTH_TAG {
            // resolved_width represents the full column width (including cell padding/border),
            // so for content-box cells we must add cell_pb to the CSS width value.
            let fixed_w =
                if cell_box_sizing == BoxSizing::ContentBox { width_dim.value() + cell_pb } else { width_dim.value() };
            columns[col].fixed = Some(match columns[col].fixed {
                Some(current) => f32_max(current, fixed_w),
                None => fixed_w,
            });
        } else if width_tag == CompactLength::PERCENT_TAG {
            let pct = width_dim.value();
            columns[col].percent = Some(match columns[col].percent {
                Some(current) => f32_max(current, pct),
                None => pct,
            });
        }

        // Measure intrinsic cell size.
        // measure_child_size_both with SizingMode::ContentSize returns the outer size
        // (including padding/border), so we must NOT add cell_pb again.
        let min_w = tree
            .measure_child_size_both(
                cell.node_id,
                Size::NONE,
                parent_size,
                Size { width: AvailableSpace::MinContent, height: AvailableSpace::MinContent },
                SizingMode::ContentSize,
                Line::FALSE,
            )
            .width;
        if min_w > columns[col].min_content_width {
            columns[col].min_content_width = min_w;
        }

        let max_w = tree
            .measure_child_size_both(
                cell.node_id,
                Size::NONE,
                parent_size,
                Size { width: AvailableSpace::MaxContent, height: AvailableSpace::MaxContent },
                SizingMode::ContentSize,
                Line::FALSE,
            )
            .width;
        if max_w > columns[col].max_content_width {
            columns[col].max_content_width = max_w;
        }
    }

    // Spanning cells: raise the min/max content widths of the columns they span so
    // that the span can accommodate the cell's intrinsic sizes.
    for cell in &cells {
        if cell.colspan <= 1 {
            continue;
        }
        let col_end = (cell.col_start + cell.colspan).min(max_columns);
        if cell.col_start >= col_end {
            continue;
        }
        let spacing_in_span = h_spacing * (col_end - cell.col_start).saturating_sub(1) as f32;

        let span_min = tree
            .measure_child_size_both(
                cell.node_id,
                Size::NONE,
                parent_size,
                Size { width: AvailableSpace::MinContent, height: AvailableSpace::MinContent },
                SizingMode::ContentSize,
                Line::FALSE,
            )
            .width;
        raise_columns_to_fit(&mut columns[cell.col_start..col_end], span_min - spacing_in_span, |c| {
            &mut c.min_content_width
        });

        let span_max = tree
            .measure_child_size_both(
                cell.node_id,
                Size::NONE,
                parent_size,
                Size { width: AvailableSpace::MaxContent, height: AvailableSpace::MaxContent },
                SizingMode::ContentSize,
                Line::FALSE,
            )
            .width;
        raise_columns_to_fit(&mut columns[cell.col_start..col_end], span_max - spacing_in_span, |c| {
            &mut c.max_content_width
        });
    }

    // Determine the table's used width from its min/max content widths (CSS 2.1 §17.5.2.2)
    let total_spacing = h_spacing * (max_columns as f32 + 1.0);
    let outer_min: f32 =
        columns.iter().map(|c| col_min_w(c, is_fixed_layout)).sum::<f32>() + total_spacing + padding_border_size.width;
    let outer_max: f32 =
        columns.iter().map(|c| col_max_w(c, is_fixed_layout)).sum::<f32>() + total_spacing + padding_border_size.width;

    let table_width = match styled_known_dimensions.width {
        // An explicitly sized table still grows to fit its columns' min-content
        // widths (except under table-layout: fixed, which never grows)
        Some(w) => {
            if is_fixed_layout {
                w
            } else {
                f32_max(w, outer_min)
            }
        }
        // Auto-width table: shrink-to-fit = max(min-content, min(available, max-content)).
        // Note: the parent is responsible for subtracting this node's margins from
        // definite available space (block/flex parents already do).
        None => {
            let candidate = match available_space.width {
                AvailableSpace::Definite(w) => w.min(outer_max),
                AvailableSpace::MaxContent => outer_max,
                AvailableSpace::MinContent => outer_min,
            };
            f32_max(candidate.maybe_clamp(min_size.width, max_size.width), outer_min)
        }
    };

    // Distribute the table's content width to columns
    let width_for_columns = f32_max(table_width - padding_border_size.width - total_spacing, 0.0);
    distribute_column_widths(&mut columns, width_for_columns, is_fixed_layout);

    let table_content_width: f32 = columns.iter().map(|c| c.resolved_width).sum::<f32>() + total_spacing;

    // Cache each cell's final width (spanning cells cover their columns plus the
    // spacing between them)
    let cell_widths: Vec<f32> = cells
        .iter()
        .map(|cell| {
            let col_end = (cell.col_start + cell.colspan).min(max_columns);
            (cell.col_start..col_end).map(|c| columns[c].resolved_width).sum::<f32>()
                + h_spacing * (col_end.saturating_sub(cell.col_start + 1)) as f32
        })
        .collect();

    // Phase 3: Measure cells at their resolved widths to compute row heights and the
    // table's first-row baseline. This is measure-only: layouts are not written here,
    // so pure ComputeSize passes never mutate the tree (matching block layout).
    let num_rows = rows.len();
    let mut row_heights: Vec<f32> = vec![0.0; num_rows];
    let mut first_row_baseline: Option<f32> = None;
    #[cfg(feature = "content_size")]
    let mut measured_cell_content_sizes: Vec<Size<f32>> = Vec::with_capacity(cells.len());
    #[cfg(feature = "content_size")]
    let mut cell_overflows: Vec<Point<Overflow>> = Vec::with_capacity(cells.len());

    for (cell_idx, cell) in cells.iter().enumerate() {
        let cell_width = cell_widths[cell_idx];
        let measured = tree.compute_child_layout(
            cell.node_id,
            LayoutInput {
                run_mode: RunMode::ComputeSize,
                sizing_mode: SizingMode::InherentSize,
                axis: RequestedAxis::Both,
                known_dimensions: Size { width: Some(cell_width), height: None },
                parent_size: Size { width: Some(table_width), height: parent_size.height },
                available_space: Size { width: AvailableSpace::Definite(cell_width), height: AvailableSpace::MaxContent },
                vertical_margins_are_collapsible: Line::FALSE,
            },
        );

        if cell.row_index < num_rows && measured.size.height > row_heights[cell.row_index] {
            row_heights[cell.row_index] = measured.size.height;
        }

        // The table's baseline is the baseline of its first row (max cell baseline)
        if cell.row_index == 0 {
            if let Some(baseline) = measured.first_baselines.y {
                first_row_baseline =
                    Some(match first_row_baseline {
                        Some(current) => f32_max(current, baseline),
                        None => baseline,
                    });
            }
        }

        #[cfg(feature = "content_size")]
        {
            measured_cell_content_sizes.push(measured.content_size);
            let cell_style = tree.get_core_container_style(cell.node_id);
            cell_overflows.push(cell_style.overflow());
        }
    }

    let total_row_height: f32 = row_heights.iter().sum();
    let total_v_spacing = v_spacing * (num_rows as f32 + 1.0);
    let table_content_height = total_row_height + total_v_spacing;
    let table_height = styled_known_dimensions
        .height
        .unwrap_or((table_content_height + padding_border_size.height).maybe_clamp(min_size.height, max_size.height));

    let final_size = Size { width: table_width, height: table_height };

    // Compute column x-offsets and row y-offsets (in table coordinates)
    let mut col_x_offsets: Vec<f32> = Vec::with_capacity(max_columns);
    let mut x = padding_border.left + h_spacing;
    for col in &columns {
        col_x_offsets.push(x);
        x += col.resolved_width + h_spacing;
    }

    let mut row_y_offsets: Vec<f32> = Vec::with_capacity(num_rows);
    let mut y = padding_border.top + v_spacing;
    for &rh in &row_heights {
        row_y_offsets.push(y);
        y += rh + v_spacing;
    }

    let first_baselines = Point { x: None, y: first_row_baseline.map(|b| row_y_offsets[0] + b) };

    // Accumulate the table's content size from the cells' measured content sizes
    #[cfg(feature = "content_size")]
    let table_content_size: Size<f32> = {
        let mut content_size = Size { width: table_content_width, height: table_content_height };
        for (cell_idx, cell) in cells.iter().enumerate() {
            let location =
                Point { x: col_x_offsets[cell.col_start], y: row_y_offsets.get(cell.row_index).copied().unwrap_or(0.0) };
            let size = Size {
                width: cell_widths[cell_idx],
                height: row_heights.get(cell.row_index).copied().unwrap_or(0.0),
            };
            content_size = content_size.f32_max(compute_content_size_contribution(
                location,
                size,
                measured_cell_content_sizes[cell_idx],
                cell_overflows[cell_idx],
            ));
        }
        content_size
    };
    #[cfg(not(feature = "content_size"))]
    let table_content_size = Size::ZERO;

    if run_mode == RunMode::ComputeSize {
        return LayoutOutput::from_sizes_and_baselines(final_size, table_content_size, first_baselines);
    }

    // Phase 4: Perform final layout and position cells
    #[cfg(feature = "content_size")]
    let mut row_content_sizes: Vec<Size<f32>> = vec![Size::ZERO; num_rows];

    for (cell_idx, cell) in cells.iter().enumerate() {
        if cell.row_index >= num_rows || cell.col_start >= max_columns {
            continue;
        }

        let cell_width = cell_widths[cell_idx];
        let cell_height = row_heights[cell.row_index];

        // Re-layout the cell with final dimensions
        let cell_output = tree.perform_child_layout(
            cell.node_id,
            Size { width: Some(cell_width), height: Some(cell_height) },
            Size { width: Some(table_width), height: Some(table_height) },
            Size { width: AvailableSpace::Definite(cell_width), height: AvailableSpace::Definite(cell_height) },
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
        #[cfg(feature = "content_size")]
        let cell_overflow = cell_style.overflow();
        drop(cell_style);

        // Cells inside real rows are positioned relative to their row; cells in
        // anonymous rows are children of the table and use table coordinates.
        let location = if cell.parent_is_table {
            Point { x: col_x_offsets[cell.col_start], y: row_y_offsets[cell.row_index] }
        } else {
            Point { x: col_x_offsets[cell.col_start] - padding_border.left, y: 0.0 }
        };

        #[cfg(feature = "content_size")]
        if !cell.parent_is_table {
            row_content_sizes[cell.row_index] =
                row_content_sizes[cell.row_index].f32_max(compute_content_size_contribution(
                    location,
                    Size { width: cell_width, height: cell_height },
                    cell_output.content_size,
                    cell_overflow,
                ));
        }

        tree.set_unrounded_layout(
            cell.node_id,
            &Layout {
                order: cell.order,
                location,
                size: Size { width: cell_width, height: cell_height },
                #[cfg(feature = "content_size")]
                content_size: cell_output.content_size,
                scrollbar_size,
                padding: cell_padding,
                border: cell_border_val,
                margin: cell_margin,
            },
        );
    }

    let row_width = table_width - padding_border_size.width;

    // Build a mapping from row index to parent row group's start_y offset.
    // Rows inside a row group need their location relative to the group, not the table.
    let mut row_parent_offset_y: Vec<f32> = vec![0.0; num_rows];
    let mut row_parent_offset_x: Vec<f32> = vec![0.0; num_rows];

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
            #[cfg(feature = "content_size")]
            let mut group_content_size = Size::ZERO;
            for gi in 0..group_child_count {
                let row_in_group = tree.get_child_id(child_id, gi);
                if let Some(ri) = rows.iter().position(|r| r.node == Some(row_in_group)) {
                    let ry = row_y_offsets.get(ri).copied().unwrap_or(0.0);
                    let rh = row_heights.get(ri).copied().unwrap_or(0.0);
                    if gi == 0 {
                        start_y = ry;
                    }
                    end_y = ry + rh;
                    // Record parent offset so row position can be made relative
                    row_parent_offset_y[ri] = start_y;
                    row_parent_offset_x[ri] = padding_border.left;

                    #[cfg(feature = "content_size")]
                    {
                        group_content_size = group_content_size.f32_max(compute_content_size_contribution(
                            Point { x: 0.0, y: ry - start_y },
                            Size { width: row_width, height: rh },
                            row_content_sizes[ri],
                            Point { x: Overflow::Visible, y: Overflow::Visible },
                        ));
                    }
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
                    size: Size { width: row_width, height: end_y - start_y },
                    #[cfg(feature = "content_size")]
                    content_size: group_content_size,
                    scrollbar_size: Size::ZERO,
                    padding: group_padding,
                    border: group_border,
                    margin: group_margin,
                },
            );
        }
    }

    // Set layouts for row nodes (anonymous rows have no node to lay out).
    // Row locations are relative to their parent (row group or table).
    for (row_idx, entry) in rows.iter().enumerate() {
        if let Some(row_id) = entry.node {
            let row_y = row_y_offsets.get(row_idx).copied().unwrap_or(0.0);
            let row_h = row_heights.get(row_idx).copied().unwrap_or(0.0);

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
                    order: entry.order,
                    location: Point { x: relative_x, y: relative_y },
                    size: Size { width: row_width, height: row_h },
                    #[cfg(feature = "content_size")]
                    content_size: row_content_sizes[row_idx],
                    scrollbar_size: Size::ZERO,
                    padding: row_padding,
                    border: row_border,
                    margin: row_margin,
                },
            );
        }
    }

    LayoutOutput::from_sizes_and_baselines(final_size, table_content_size, first_baselines)
}

/// Collect cells from a row node's children. All children of a row are treated as
/// cells: non-cell children are wrapped in anonymous cells per CSS 2.1 §17.2.1
/// (i.e. treated as the cell itself).
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
            order: cell_idx as u32,
            parent_is_table: false,
        });

        col += colspan;
    }

    if col > *max_columns {
        *max_columns = col;
    }
}

/// Raise the given columns' widths (selected by `field`) so that they can jointly
/// accommodate `target`. Extra width goes to auto columns when the span contains
/// any, otherwise to all spanned columns equally.
fn raise_columns_to_fit(columns: &mut [ColumnInfo], target: f32, field: impl Fn(&mut ColumnInfo) -> &mut f32) {
    if columns.is_empty() {
        return;
    }
    let current: f32 = columns.iter_mut().map(|c| *field(c)).sum();
    if target <= current {
        return;
    }
    let extra = target - current;

    let auto_count = columns.iter().filter(|c| c.is_auto()).count();
    if auto_count > 0 {
        let per_col = extra / auto_count as f32;
        for col in columns.iter_mut().filter(|c| c.is_auto()) {
            *field(col) += per_col;
        }
    } else {
        let per_col = extra / columns.len() as f32;
        for col in columns.iter_mut() {
            *field(col) += per_col;
        }
    }
}

/// Distribute `target` width (the table's content width minus border-spacing) to
/// columns (CSS 2.1 §17.5.2.2):
/// - Percentage columns resolve against the target width (floored at min-content
///   under automatic layout; exact under `table-layout: fixed`)
/// - Fixed columns use their specified width (floored at min-content under
///   automatic layout; exact under `table-layout: fixed`)
/// - Auto columns share the remaining space: below their combined min-content they
///   overflow at min-content; between min and max they grow proportionally to
///   (max - min); above max the excess is distributed proportionally to max
/// - If there are no auto columns, leftover space is distributed equally to all
///   columns
fn distribute_column_widths(columns: &mut [ColumnInfo], target: f32, is_fixed_layout: bool) {
    if columns.is_empty() {
        return;
    }

    let mut remaining = target;
    for col in columns.iter_mut() {
        if let Some(pct) = col.percent {
            let w = pct * target;
            col.resolved_width = if is_fixed_layout { f32_max(w, 0.0) } else { f32_max(w, col.min_content_width) };
            remaining -= col.resolved_width;
        } else if col.fixed.is_some() {
            col.resolved_width = col_min_w(col, is_fixed_layout);
            remaining -= col.resolved_width;
        }
    }

    let auto_count = columns.iter().filter(|c| c.is_auto()).count();
    if auto_count > 0 {
        let sum_min: f32 = columns.iter().filter(|c| c.is_auto()).map(|c| col_min_w(c, is_fixed_layout)).sum();
        let sum_max: f32 = columns.iter().filter(|c| c.is_auto()).map(|c| col_max_w(c, is_fixed_layout)).sum();

        if remaining >= sum_max {
            // Every auto column gets its max-content width; excess is distributed
            // proportionally to max-content width
            let excess = remaining - sum_max;
            for col in columns.iter_mut().filter(|c| c.is_auto()) {
                let max_w = col_max_w(col, is_fixed_layout);
                let share = if sum_max > 0.0 { max_w / sum_max } else { 1.0 / auto_count as f32 };
                col.resolved_width = max_w + excess * share;
            }
        } else if remaining > sum_min {
            // Between min and max: grow columns proportionally to (max - min)
            let pool = remaining - sum_min;
            let sum_diff = sum_max - sum_min;
            for col in columns.iter_mut().filter(|c| c.is_auto()) {
                let min_w = col_min_w(col, is_fixed_layout);
                let share =
                    if sum_diff > 0.0 { (col_max_w(col, is_fixed_layout) - min_w) / sum_diff } else { 1.0 / auto_count as f32 };
                col.resolved_width = min_w + pool * share;
            }
        } else {
            // Not enough space: columns get their min-content width (content overflows)
            for col in columns.iter_mut().filter(|c| c.is_auto()) {
                col.resolved_width = col_min_w(col, is_fixed_layout);
            }
        }
    } else if remaining > 0.0 {
        // No auto columns: distribute leftover space equally to all columns
        let per_col = remaining / columns.len() as f32;
        for col in columns.iter_mut() {
            col.resolved_width += per_col;
        }
    }
}
