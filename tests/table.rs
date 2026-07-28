#[cfg(feature = "table_layout")]
mod table_tests {
    use taffy::prelude::*;
    use taffy::style::{BorderCollapse, CaptionSide, Display, LengthPercentage, TableLayout};
    use taffy::BoxSizing;

    #[test]
    fn basic_2x2_auto_widths() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();

        // Create 4 cells (all leaf nodes with fixed sizes)
        let cell_style = Style {
            display: Display::TableCell,
            size: Size::from_lengths(100.0, 30.0),
            ..Default::default()
        };

        let cell00 = taffy.new_leaf(cell_style.clone()).unwrap();
        let cell01 = taffy.new_leaf(cell_style.clone()).unwrap();
        let cell10 = taffy.new_leaf(cell_style.clone()).unwrap();
        let cell11 = taffy.new_leaf(cell_style.clone()).unwrap();

        // Create rows
        let row0 = taffy
            .new_with_children(Style { display: Display::TableRow, ..Default::default() }, &[cell00, cell01])
            .unwrap();
        let row1 = taffy
            .new_with_children(Style { display: Display::TableRow, ..Default::default() }, &[cell10, cell11])
            .unwrap();

        // Create table
        let table = taffy
            .new_with_children(Style { display: Display::Table, ..Default::default() }, &[row0, row1])
            .unwrap();

        taffy.compute_layout(table, Size::MAX_CONTENT).unwrap();

        let table_layout = taffy.layout(table).unwrap();
        // Table should be at least 200px wide (2 columns * 100px)
        assert!(table_layout.size.width >= 200.0, "Table width {} should be >= 200", table_layout.size.width);
        // Table should be at least 60px tall (2 rows * 30px)
        assert!(table_layout.size.height >= 60.0, "Table height {} should be >= 60", table_layout.size.height);

        // Cells in the same column should have the same x position
        let cell00_layout = taffy.layout(cell00).unwrap();
        let cell10_layout = taffy.layout(cell10).unwrap();
        assert_eq!(
            cell00_layout.location.x, cell10_layout.location.x,
            "Column 0 cells should be aligned"
        );

        let cell01_layout = taffy.layout(cell01).unwrap();
        let cell11_layout = taffy.layout(cell11).unwrap();
        assert_eq!(
            cell01_layout.location.x, cell11_layout.location.x,
            "Column 1 cells should be aligned"
        );

        // Cells in the same row should have the same y position
        assert_eq!(
            cell00_layout.location.y, cell01_layout.location.y,
            "Row 0 cells should be aligned"
        );
        assert_eq!(
            cell10_layout.location.y, cell11_layout.location.y,
            "Row 1 cells should be aligned"
        );
    }

    #[test]
    fn explicit_px_column_widths() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();

        let cell0 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size { width: Dimension::from_length(150.0), height: Dimension::from_length(30.0) },
                ..Default::default()
            })
            .unwrap();
        let cell1 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size { width: Dimension::from_length(250.0), height: Dimension::from_length(30.0) },
                ..Default::default()
            })
            .unwrap();

        let row = taffy
            .new_with_children(Style { display: Display::TableRow, ..Default::default() }, &[cell0, cell1])
            .unwrap();

        let table = taffy
            .new_with_children(Style { display: Display::Table, ..Default::default() }, &[row])
            .unwrap();

        taffy.compute_layout(table, Size::MAX_CONTENT).unwrap();

        let cell0_layout = taffy.layout(cell0).unwrap();
        let cell1_layout = taffy.layout(cell1).unwrap();

        assert_eq!(cell0_layout.size.width, 150.0, "Cell 0 width should be 150px");
        assert_eq!(cell1_layout.size.width, 250.0, "Cell 1 width should be 250px");
    }

    #[test]
    fn percentage_column_widths() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();

        let cell0 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size { width: Dimension::from_percent(0.3), height: Dimension::from_length(30.0) },
                ..Default::default()
            })
            .unwrap();
        let cell1 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size { width: Dimension::from_percent(0.7), height: Dimension::from_length(30.0) },
                ..Default::default()
            })
            .unwrap();

        let row = taffy
            .new_with_children(Style { display: Display::TableRow, ..Default::default() }, &[cell0, cell1])
            .unwrap();

        let table = taffy
            .new_with_children(
                Style {
                    display: Display::Table,
                    size: Size { width: Dimension::from_length(500.0), height: Dimension::AUTO },
                    ..Default::default()
                },
                &[row],
            )
            .unwrap();

        taffy.compute_layout(table, Size::MAX_CONTENT).unwrap();

        let cell0_layout = taffy.layout(cell0).unwrap();
        let cell1_layout = taffy.layout(cell1).unwrap();

        // With a 500px table, spacing=0, percentages resolve against available_for_columns
        // The exact values depend on spacing, but the ratio should be approximately 30/70
        let total = cell0_layout.size.width + cell1_layout.size.width;
        let ratio = cell0_layout.size.width / total;
        assert!(
            (ratio - 0.3).abs() < 0.05,
            "Cell 0 should be ~30% of total, got {}%",
            ratio * 100.0
        );
    }

    #[test]
    fn colspan_spanning_columns() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();

        // Row 0: 3 cells, each 100px wide
        let cell00 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size::from_lengths(100.0, 30.0),
                ..Default::default()
            })
            .unwrap();
        let cell01 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size::from_lengths(100.0, 30.0),
                ..Default::default()
            })
            .unwrap();
        let cell02 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size::from_lengths(100.0, 30.0),
                ..Default::default()
            })
            .unwrap();

        // Row 1: 1 cell spanning 2 columns + 1 normal cell
        let cell10 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size::from_lengths(100.0, 30.0),
                colspan: 2,
                ..Default::default()
            })
            .unwrap();
        let cell11 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size::from_lengths(100.0, 30.0),
                ..Default::default()
            })
            .unwrap();

        let row0 = taffy
            .new_with_children(
                Style { display: Display::TableRow, ..Default::default() },
                &[cell00, cell01, cell02],
            )
            .unwrap();
        let row1 = taffy
            .new_with_children(Style { display: Display::TableRow, ..Default::default() }, &[cell10, cell11])
            .unwrap();

        let table = taffy
            .new_with_children(Style { display: Display::Table, ..Default::default() }, &[row0, row1])
            .unwrap();

        taffy.compute_layout(table, Size::MAX_CONTENT).unwrap();

        let cell10_layout = taffy.layout(cell10).unwrap();
        let cell00_layout = taffy.layout(cell00).unwrap();
        let cell01_layout = taffy.layout(cell01).unwrap();

        // The colspan=2 cell should span the width of columns 0 and 1
        let col0_width = cell00_layout.size.width;
        let col1_width = cell01_layout.size.width;
        // cell10 should be at least as wide as col0 + col1
        assert!(
            cell10_layout.size.width >= col0_width + col1_width - 1.0,
            "Colspan cell width {} should span columns 0+1 ({}+{})",
            cell10_layout.size.width,
            col0_width,
            col1_width
        );
    }

    #[test]
    fn border_spacing() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();

        let cell0 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size::from_lengths(100.0, 30.0),
                ..Default::default()
            })
            .unwrap();
        let cell1 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size::from_lengths(100.0, 30.0),
                ..Default::default()
            })
            .unwrap();

        let row = taffy
            .new_with_children(Style { display: Display::TableRow, ..Default::default() }, &[cell0, cell1])
            .unwrap();

        let table = taffy
            .new_with_children(
                Style {
                    display: Display::Table,
                    border_spacing: Size {
                        width: LengthPercentage::length(10.0),
                        height: LengthPercentage::length(5.0),
                    },
                    ..Default::default()
                },
                &[row],
            )
            .unwrap();

        taffy.compute_layout(table, Size::MAX_CONTENT).unwrap();

        let table_layout = taffy.layout(table).unwrap();
        let cell0_layout = taffy.layout(cell0).unwrap();
        let cell1_layout = taffy.layout(cell1).unwrap();

        // With 10px horizontal spacing and 2 columns:
        // total_spacing = 10 * (2 + 1) = 30px
        // table width should be 100 + 100 + 30 = 230px
        assert_eq!(table_layout.size.width, 230.0, "Table width should include border-spacing");

        // First cell should start at x=10 (one spacing unit from left)
        assert_eq!(cell0_layout.location.x, 10.0, "Cell 0 x should be 10 (border-spacing)");

        // Second cell should start at 10 + 100 + 10 = 120
        assert_eq!(cell1_layout.location.x, 120.0, "Cell 1 x should be 120");

        // With 5px vertical spacing and 1 row:
        // total_v_spacing = 5 * (1 + 1) = 10px
        // table height should be 30 + 10 = 40px
        assert_eq!(table_layout.size.height, 40.0, "Table height should include border-spacing");
    }

    #[test]
    fn row_height_is_max_of_cells() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();

        let cell0 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size::from_lengths(100.0, 30.0),
                ..Default::default()
            })
            .unwrap();
        let cell1 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size::from_lengths(100.0, 60.0), // Taller cell
                ..Default::default()
            })
            .unwrap();

        let row = taffy
            .new_with_children(Style { display: Display::TableRow, ..Default::default() }, &[cell0, cell1])
            .unwrap();

        let table = taffy
            .new_with_children(Style { display: Display::Table, ..Default::default() }, &[row])
            .unwrap();

        taffy.compute_layout(table, Size::MAX_CONTENT).unwrap();

        // Both cells should be 60px tall (the height of the tallest cell)
        let cell0_layout = taffy.layout(cell0).unwrap();
        let cell1_layout = taffy.layout(cell1).unwrap();

        assert_eq!(cell0_layout.size.height, 60.0, "Cell 0 should stretch to row height");
        assert_eq!(cell1_layout.size.height, 60.0, "Cell 1 should be its own height");
    }

    #[test]
    fn nested_table() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();

        // Inner table
        let inner_cell0 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size::from_lengths(50.0, 20.0),
                ..Default::default()
            })
            .unwrap();
        let inner_cell1 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size::from_lengths(50.0, 20.0),
                ..Default::default()
            })
            .unwrap();
        let inner_row = taffy
            .new_with_children(Style { display: Display::TableRow, ..Default::default() }, &[inner_cell0, inner_cell1])
            .unwrap();
        let inner_table = taffy
            .new_with_children(Style { display: Display::Table, ..Default::default() }, &[inner_row])
            .unwrap();

        // Outer table with inner table as a cell's child
        let outer_cell0 = taffy
            .new_with_children(
                Style {
                    display: Display::TableCell,
                    ..Default::default()
                },
                &[inner_table],
            )
            .unwrap();
        let outer_cell1 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size::from_lengths(100.0, 40.0),
                ..Default::default()
            })
            .unwrap();
        let outer_row = taffy
            .new_with_children(Style { display: Display::TableRow, ..Default::default() }, &[outer_cell0, outer_cell1])
            .unwrap();
        let outer_table = taffy
            .new_with_children(Style { display: Display::Table, ..Default::default() }, &[outer_row])
            .unwrap();

        taffy.compute_layout(outer_table, Size::MAX_CONTENT).unwrap();

        let outer_layout = taffy.layout(outer_table).unwrap();
        assert!(outer_layout.size.width > 0.0, "Outer table should have positive width");
        assert!(outer_layout.size.height > 0.0, "Outer table should have positive height");
    }

    #[test]
    fn table_with_row_groups() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();

        let cell00 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size::from_lengths(100.0, 30.0),
                ..Default::default()
            })
            .unwrap();
        let cell01 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size::from_lengths(100.0, 30.0),
                ..Default::default()
            })
            .unwrap();
        let cell10 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size::from_lengths(100.0, 30.0),
                ..Default::default()
            })
            .unwrap();
        let cell11 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size::from_lengths(100.0, 30.0),
                ..Default::default()
            })
            .unwrap();

        let row0 = taffy
            .new_with_children(Style { display: Display::TableRow, ..Default::default() }, &[cell00, cell01])
            .unwrap();
        let row1 = taffy
            .new_with_children(Style { display: Display::TableRow, ..Default::default() }, &[cell10, cell11])
            .unwrap();

        // Wrap rows in a tbody (TableRowGroup)
        let tbody = taffy
            .new_with_children(Style { display: Display::TableRowGroup, ..Default::default() }, &[row0, row1])
            .unwrap();

        let table = taffy
            .new_with_children(Style { display: Display::Table, ..Default::default() }, &[tbody])
            .unwrap();

        taffy.compute_layout(table, Size::MAX_CONTENT).unwrap();

        let table_layout = taffy.layout(table).unwrap();
        assert!(table_layout.size.width >= 200.0, "Table should be at least 200px wide");
        assert!(table_layout.size.height >= 60.0, "Table should be at least 60px tall");

        // Cells should still be properly aligned
        let cell00_layout = taffy.layout(cell00).unwrap();
        let cell10_layout = taffy.layout(cell10).unwrap();
        assert_eq!(
            cell00_layout.location.x, cell10_layout.location.x,
            "Column 0 cells should be aligned across rows"
        );
    }

    #[test]
    fn empty_table() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();

        let table = taffy
            .new_with_children(Style { display: Display::Table, ..Default::default() }, &[])
            .unwrap();

        taffy.compute_layout(table, Size::MAX_CONTENT).unwrap();

        let table_layout = taffy.layout(table).unwrap();
        assert_eq!(table_layout.size.width, 0.0);
        assert_eq!(table_layout.size.height, 0.0);
    }

    #[test]
    fn table_with_fixed_width() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();

        let cell0 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size::from_lengths(50.0, 30.0),
                ..Default::default()
            })
            .unwrap();
        let cell1 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size::from_lengths(50.0, 30.0),
                ..Default::default()
            })
            .unwrap();

        let row = taffy
            .new_with_children(Style { display: Display::TableRow, ..Default::default() }, &[cell0, cell1])
            .unwrap();

        let table = taffy
            .new_with_children(
                Style {
                    display: Display::Table,
                    size: Size { width: Dimension::from_length(400.0), height: Dimension::AUTO },
                    ..Default::default()
                },
                &[row],
            )
            .unwrap();

        taffy.compute_layout(table, Size::MAX_CONTENT).unwrap();

        let table_layout = taffy.layout(table).unwrap();
        assert_eq!(table_layout.size.width, 400.0, "Table should respect fixed width");

        // Extra space should be distributed to columns
        let cell0_layout = taffy.layout(cell0).unwrap();
        let cell1_layout = taffy.layout(cell1).unwrap();
        let total_cell_width = cell0_layout.size.width + cell1_layout.size.width;
        assert!(
            total_cell_width > 100.0,
            "Cells should expand to fill table (got {})",
            total_cell_width
        );
    }

    #[test]
    fn mixed_auto_fixed_percentage_columns() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();

        // Fixed 100px column
        let cell0 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size { width: Dimension::from_length(100.0), height: Dimension::from_length(30.0) },
                ..Default::default()
            })
            .unwrap();
        // 50% column
        let cell1 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size { width: Dimension::from_percent(0.5), height: Dimension::from_length(30.0) },
                ..Default::default()
            })
            .unwrap();
        // Auto column
        let cell2 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size { width: Dimension::AUTO, height: Dimension::from_length(30.0) },
                ..Default::default()
            })
            .unwrap();

        let row = taffy
            .new_with_children(Style { display: Display::TableRow, ..Default::default() }, &[cell0, cell1, cell2])
            .unwrap();

        let table = taffy
            .new_with_children(
                Style {
                    display: Display::Table,
                    size: Size { width: Dimension::from_length(500.0), height: Dimension::AUTO },
                    ..Default::default()
                },
                &[row],
            )
            .unwrap();

        taffy.compute_layout(table, Size::MAX_CONTENT).unwrap();

        let cell0_layout = taffy.layout(cell0).unwrap();
        let cell1_layout = taffy.layout(cell1).unwrap();

        assert_eq!(cell0_layout.size.width, 100.0, "Fixed column should be 100px");
        // The percentage column should be roughly 50% of available_for_columns
        assert!(cell1_layout.size.width > 200.0, "50% column should be > 200px, got {}", cell1_layout.size.width);
    }

    #[test]
    fn fixed_width_columns_with_padding_and_auto_column() {
        // Regression test: fixed-width columns with cell padding should get exactly
        // their specified content width + padding, with the auto column absorbing
        // the remaining space. Previously, resolved_width for Fixed columns did not
        // include cell padding, causing a units mismatch with min/max_content_width
        // (which DO include padding), leading to incorrect proportional distribution.
        //
        // Setup: 520px table, 3 columns:
        //   col 0: auto width, 16px padding each side
        //   col 1: 85px fixed width, 16px padding each side
        //   col 2: 85px fixed width, 16px padding each side
        //
        // Expected (matching browser behavior):
        //   col 1 & 2: 85 + 32 = 117px each
        //   col 0: 520 - 117 - 117 = 286px
        let mut taffy: TaffyTree<()> = TaffyTree::new();

        let padding_16 = Rect {
            left: LengthPercentage::length(16.0),
            right: LengthPercentage::length(16.0),
            top: LengthPercentage::length(16.0),
            bottom: LengthPercentage::length(16.0),
        };

        // Auto-width cell with some content size (content-box to match browser default)
        let cell_auto = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                box_sizing: BoxSizing::ContentBox,
                size: Size { width: Dimension::AUTO, height: Dimension::from_length(30.0) },
                padding: padding_16.clone(),
                ..Default::default()
            })
            .unwrap();

        // Fixed 85px cells (content-box: 85px is content width, total = 85 + 32 padding = 117px)
        let cell_fixed1 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                box_sizing: BoxSizing::ContentBox,
                size: Size { width: Dimension::from_length(85.0), height: Dimension::from_length(30.0) },
                padding: padding_16.clone(),
                ..Default::default()
            })
            .unwrap();
        let cell_fixed2 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                box_sizing: BoxSizing::ContentBox,
                size: Size { width: Dimension::from_length(85.0), height: Dimension::from_length(30.0) },
                padding: padding_16.clone(),
                ..Default::default()
            })
            .unwrap();

        let row = taffy
            .new_with_children(
                Style { display: Display::TableRow, ..Default::default() },
                &[cell_auto, cell_fixed1, cell_fixed2],
            )
            .unwrap();

        let table = taffy
            .new_with_children(
                Style {
                    display: Display::Table,
                    size: Size { width: Dimension::from_length(520.0), height: Dimension::AUTO },
                    ..Default::default()
                },
                &[row],
            )
            .unwrap();

        taffy.compute_layout(table, Size::MAX_CONTENT).unwrap();

        let auto_layout = taffy.layout(cell_auto).unwrap();
        let fixed1_layout = taffy.layout(cell_fixed1).unwrap();
        let fixed2_layout = taffy.layout(cell_fixed2).unwrap();

        // Fixed columns should be exactly 85 (content) + 32 (padding) = 117px
        assert_eq!(
            fixed1_layout.size.width, 117.0,
            "Fixed col 1 should be 117px (85 content + 32 padding), got {}",
            fixed1_layout.size.width
        );
        assert_eq!(
            fixed2_layout.size.width, 117.0,
            "Fixed col 2 should be 117px (85 content + 32 padding), got {}",
            fixed2_layout.size.width
        );

        // Auto column should get the remaining space: 520 - 117 - 117 = 286px
        assert_eq!(
            auto_layout.size.width, 286.0,
            "Auto col should be 286px (520 - 117 - 117), got {}",
            auto_layout.size.width
        );
    }

    #[test]
    fn table_layout_fixed_ignores_min_content_width() {
        // With table-layout: fixed, fixed-width columns should use their specified
        // width exactly, even when min_content_width is larger. Content overflows
        // rather than expanding the column.
        //
        // Setup: 520px table with table-layout: fixed, 3 columns:
        //   col 0: auto width, 16px padding each side
        //   col 1: 85px content-box width, 16px padding each side (total = 117px)
        //   col 2: 85px content-box width, 16px padding each side (total = 117px)
        //
        // The fixed columns contain wide content (150px leaf) that exceeds 85px,
        // but with table-layout: fixed the column stays at 117px.
        let mut taffy: TaffyTree<()> = TaffyTree::new();

        let padding_16 = Rect {
            left: LengthPercentage::length(16.0),
            right: LengthPercentage::length(16.0),
            top: LengthPercentage::length(16.0),
            bottom: LengthPercentage::length(16.0),
        };

        // Auto-width cell
        let cell_auto = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                box_sizing: BoxSizing::ContentBox,
                size: Size { width: Dimension::AUTO, height: Dimension::from_length(30.0) },
                padding: padding_16.clone(),
                ..Default::default()
            })
            .unwrap();

        // Fixed 85px cells with wide content (150px leaf child)
        let wide_content1 = taffy
            .new_leaf(Style {
                size: Size::from_lengths(150.0, 14.0),
                ..Default::default()
            })
            .unwrap();
        let cell_fixed1 = taffy
            .new_with_children(
                Style {
                    display: Display::TableCell,
                    box_sizing: BoxSizing::ContentBox,
                    size: Size { width: Dimension::from_length(85.0), height: Dimension::from_length(30.0) },
                    padding: padding_16.clone(),
                    ..Default::default()
                },
                &[wide_content1],
            )
            .unwrap();

        let wide_content2 = taffy
            .new_leaf(Style {
                size: Size::from_lengths(150.0, 14.0),
                ..Default::default()
            })
            .unwrap();
        let cell_fixed2 = taffy
            .new_with_children(
                Style {
                    display: Display::TableCell,
                    box_sizing: BoxSizing::ContentBox,
                    size: Size { width: Dimension::from_length(85.0), height: Dimension::from_length(30.0) },
                    padding: padding_16.clone(),
                    ..Default::default()
                },
                &[wide_content2],
            )
            .unwrap();

        let row = taffy
            .new_with_children(
                Style { display: Display::TableRow, ..Default::default() },
                &[cell_auto, cell_fixed1, cell_fixed2],
            )
            .unwrap();

        let table = taffy
            .new_with_children(
                Style {
                    display: Display::Table,
                    table_layout: TableLayout::Fixed,
                    size: Size { width: Dimension::from_length(520.0), height: Dimension::AUTO },
                    ..Default::default()
                },
                &[row],
            )
            .unwrap();

        taffy.compute_layout(table, Size::MAX_CONTENT).unwrap();

        let auto_layout = taffy.layout(cell_auto).unwrap();
        let fixed1_layout = taffy.layout(cell_fixed1).unwrap();
        let fixed2_layout = taffy.layout(cell_fixed2).unwrap();

        // Fixed columns should be exactly 117px (85 + 32 padding), NOT expanded by content
        assert_eq!(
            fixed1_layout.size.width, 117.0,
            "Fixed col 1 should be 117px regardless of content width, got {}",
            fixed1_layout.size.width
        );
        assert_eq!(
            fixed2_layout.size.width, 117.0,
            "Fixed col 2 should be 117px regardless of content width, got {}",
            fixed2_layout.size.width
        );

        // Auto column gets remaining: 520 - 117 - 117 = 286px
        assert_eq!(
            auto_layout.size.width, 286.0,
            "Auto col should be 286px, got {}",
            auto_layout.size.width
        );
    }

    #[test]
    fn direct_table_cell_children_not_discarded() {
        // Regression test: direct TableCell children of a table (no wrapping TableRow)
        // should be treated as cells in an anonymous row, not discarded.
        let mut taffy: TaffyTree<()> = TaffyTree::new();

        let cell0 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size::from_lengths(100.0, 30.0),
                ..Default::default()
            })
            .unwrap();
        let cell1 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size::from_lengths(100.0, 30.0),
                ..Default::default()
            })
            .unwrap();

        // Cells are direct children of the table (no TableRow wrapper)
        let table = taffy
            .new_with_children(
                Style { display: Display::Table, ..Default::default() },
                &[cell0, cell1],
            )
            .unwrap();

        taffy.compute_layout(table, Size::MAX_CONTENT).unwrap();

        let table_layout = taffy.layout(table).unwrap();
        let cell0_layout = taffy.layout(cell0).unwrap();
        let cell1_layout = taffy.layout(cell1).unwrap();

        // Table should not be empty — cells should contribute to size
        assert!(
            table_layout.size.width > 0.0,
            "Table should not be empty, got width {}",
            table_layout.size.width
        );
        assert!(
            table_layout.size.height >= 30.0,
            "Table height should be >= 30px, got {}",
            table_layout.size.height
        );

        // Both cells should have been laid out with non-zero size
        assert_eq!(cell0_layout.size.width, 100.0, "Cell 0 width");
        assert_eq!(cell0_layout.size.height, 30.0, "Cell 0 height");
        assert_eq!(cell1_layout.size.width, 100.0, "Cell 1 width");
        assert_eq!(cell1_layout.size.height, 30.0, "Cell 1 height");
    }

    #[test]
    fn content_box_cell_padding_reflected_in_row_height() {
        // Regression test: cell padding should be included in the row height
        // when cells use content-box sizing.
        //
        // Setup: single-column table, 500px wide.
        //   Row 0: cell with padding-bottom: 20px, contains a 28px-tall leaf
        //   Expected row height: 28 + 20 = 48px
        let mut taffy: TaffyTree<()> = TaffyTree::new();

        let content = taffy
            .new_leaf(Style {
                size: Size::from_lengths(100.0, 28.0),
                ..Default::default()
            })
            .unwrap();

        let cell = taffy
            .new_with_children(
                Style {
                    display: Display::TableCell,
                    box_sizing: BoxSizing::ContentBox,
                    padding: Rect {
                        left: LengthPercentage::length(0.0),
                        right: LengthPercentage::length(0.0),
                        top: LengthPercentage::length(0.0),
                        bottom: LengthPercentage::length(20.0),
                    },
                    ..Default::default()
                },
                &[content],
            )
            .unwrap();

        let row = taffy
            .new_with_children(
                Style { display: Display::TableRow, ..Default::default() },
                &[cell],
            )
            .unwrap();

        let table = taffy
            .new_with_children(
                Style {
                    display: Display::Table,
                    size: Size { width: Dimension::from_length(500.0), height: Dimension::AUTO },
                    ..Default::default()
                },
                &[row],
            )
            .unwrap();

        taffy.compute_layout(table, Size::MAX_CONTENT).unwrap();

        let cell_layout = taffy.layout(cell).unwrap();
        let row_layout = taffy.layout(row).unwrap();

        // Cell height should include padding: 28 content + 20 padding-bottom = 48
        assert_eq!(
            cell_layout.size.height, 48.0,
            "Cell height should be 48px (28 content + 20 padding), got {}",
            cell_layout.size.height
        );
        assert_eq!(
            row_layout.size.height, 48.0,
            "Row height should be 48px, got {}",
            row_layout.size.height
        );
    }

    #[test]
    fn auto_width_table_no_padding_double_count() {
        // Regression test: auto-width column sizing was double-counting cell padding.
        // measure_child_size_both with SizingMode::ContentSize returns the outer size
        // (already including padding), but the table code was adding cell_pb on top.
        //
        // Setup: auto-width table, 4 cells each with padding: 0 6px and a 34px-wide child.
        // Expected: each cell = 34 + 12 = 46px, table = 184px
        // Bug produced: each cell = 34 + 12 + 12 = 58px, table = 232px
        let mut taffy: TaffyTree<()> = TaffyTree::new();

        let padding_6h = Rect {
            left: LengthPercentage::length(6.0),
            right: LengthPercentage::length(6.0),
            top: LengthPercentage::length(0.0),
            bottom: LengthPercentage::length(0.0),
        };

        let mut cells = Vec::new();
        for _ in 0..4 {
            let icon = taffy
                .new_leaf(Style {
                    size: Size::from_lengths(34.0, 34.0),
                    ..Default::default()
                })
                .unwrap();
            let cell = taffy
                .new_with_children(
                    Style {
                        display: Display::TableCell,
                        padding: padding_6h.clone(),
                        ..Default::default()
                    },
                    &[icon],
                )
                .unwrap();
            cells.push(cell);
        }

        let row = taffy
            .new_with_children(
                Style { display: Display::TableRow, ..Default::default() },
                &cells,
            )
            .unwrap();

        // Auto-width table (no explicit width)
        let table = taffy
            .new_with_children(
                Style { display: Display::Table, ..Default::default() },
                &[row],
            )
            .unwrap();

        taffy.compute_layout(table, Size::MAX_CONTENT).unwrap();

        let table_layout = taffy.layout(table).unwrap();
        let cell_layout = taffy.layout(cells[0]).unwrap();

        // Cell should be 34 + 6 + 6 = 46px (NOT 58px from double-counted padding)
        assert_eq!(
            cell_layout.size.width, 46.0,
            "Cell width should be 46px (34 content + 12 padding), got {}",
            cell_layout.size.width
        );
        // Table should be 4 * 46 = 184px
        assert_eq!(
            table_layout.size.width, 184.0,
            "Table width should be 184px (4 × 46), got {}",
            table_layout.size.width
        );
    }

    #[test]
    fn content_box_leaf_cell_padding_reflected_in_row_height() {
        // Same as above but cell is a LEAF node (no children, uses measure function).
        // This tests the compute_leaf_layout path instead of compute_block_layout.
        use taffy::tree::NodeId;

        let mut taffy: TaffyTree<&str> = TaffyTree::with_capacity(8);

        let cell = taffy
            .new_leaf_with_context(
                Style {
                    display: Display::TableCell,
                    box_sizing: BoxSizing::ContentBox,
                    padding: Rect {
                        left: LengthPercentage::length(0.0),
                        right: LengthPercentage::length(0.0),
                        top: LengthPercentage::length(0.0),
                        bottom: LengthPercentage::length(20.0),
                    },
                    ..Default::default()
                },
                "text_28px",
            )
            .unwrap();

        let row = taffy
            .new_with_children(
                Style { display: Display::TableRow, ..Default::default() },
                &[cell],
            )
            .unwrap();

        let table = taffy
            .new_with_children(
                Style {
                    display: Display::Table,
                    size: Size { width: Dimension::from_length(500.0), height: Dimension::AUTO },
                    ..Default::default()
                },
                &[row],
            )
            .unwrap();

        taffy
            .compute_layout_with_measure(
                table,
                Size::MAX_CONTENT,
                |known_dimensions: Size<Option<f32>>,
                 _available_space: Size<AvailableSpace>,
                 _node_id: NodeId,
                 _context: Option<&mut &str>,
                 _style: &Style| {
                    // Return content size of 100x28 (like one line of 20px text)
                    Size {
                        width: known_dimensions.width.unwrap_or(100.0),
                        height: known_dimensions.height.unwrap_or(28.0),
                    }
                },
            )
            .unwrap();

        let cell_layout = taffy.layout(cell).unwrap();
        let row_layout = taffy.layout(row).unwrap();

        // Cell height should include padding: 28 content + 20 padding-bottom = 48
        assert_eq!(
            cell_layout.size.height, 48.0,
            "Leaf cell height should be 48px (28 content + 20 padding), got {}",
            cell_layout.size.height
        );
        assert_eq!(
            row_layout.size.height, 48.0,
            "Row height should be 48px, got {}",
            row_layout.size.height
        );
    }

    /// Helper: a 2-cell single-row table where each cell holds a wrappable flex
    /// container (two 100x20 leaves, wrap) so min-content = 100 and max-content = 200.
    fn wrappable_table(taffy: &mut TaffyTree<()>, table_style: Style) -> (NodeId, NodeId, NodeId) {
        let make_cell = |taffy: &mut TaffyTree<()>| {
            let leaf_a = taffy.new_leaf(Style { size: Size::from_lengths(100.0, 20.0), ..Default::default() }).unwrap();
            let leaf_b = taffy.new_leaf(Style { size: Size::from_lengths(100.0, 20.0), ..Default::default() }).unwrap();
            let flex = taffy
                .new_with_children(
                    Style { display: Display::Flex, flex_wrap: FlexWrap::Wrap, ..Default::default() },
                    &[leaf_a, leaf_b],
                )
                .unwrap();
            taffy.new_with_children(Style { display: Display::TableCell, ..Default::default() }, &[flex]).unwrap()
        };
        let cell0 = make_cell(taffy);
        let cell1 = make_cell(taffy);
        let row = taffy
            .new_with_children(Style { display: Display::TableRow, ..Default::default() }, &[cell0, cell1])
            .unwrap();
        let table = taffy.new_with_children(table_style, &[row]).unwrap();
        (table, cell0, cell1)
    }

    #[test]
    fn auto_table_shrinks_in_narrow_container() {
        // A 2-column auto table (each column min 100 / max 200) inside a narrow
        // definite space must shrink below its max-content width (400) by wrapping
        // cell content, down to no less than its min-content width (200).
        let mut taffy: TaffyTree<()> = TaffyTree::new();
        let (table, cell0, cell1) = wrappable_table(&mut taffy, Style { display: Display::Table, ..Default::default() });

        taffy
            .compute_layout(table, Size { width: AvailableSpace::Definite(300.0), height: AvailableSpace::MaxContent })
            .unwrap();

        let table_layout = taffy.layout(table).unwrap();
        assert_eq!(table_layout.size.width, 300.0, "Table should shrink to fit 300px, got {}", table_layout.size.width);
        // Both columns are identical, so each gets half
        assert_eq!(taffy.layout(cell0).unwrap().size.width, 150.0);
        assert_eq!(taffy.layout(cell1).unwrap().size.width, 150.0);
        // Wrapped content makes rows taller
        assert_eq!(table_layout.size.height, 40.0, "Cells should wrap to two 20px lines");
    }

    #[test]
    fn auto_table_floors_at_min_content_width() {
        // Below min-content width the table stops shrinking and overflows instead
        let mut taffy: TaffyTree<()> = TaffyTree::new();
        let (table, cell0, _) = wrappable_table(&mut taffy, Style { display: Display::Table, ..Default::default() });

        taffy
            .compute_layout(table, Size { width: AvailableSpace::Definite(120.0), height: AvailableSpace::MaxContent })
            .unwrap();

        assert_eq!(taffy.layout(table).unwrap().size.width, 200.0, "Table must not shrink below min-content (200)");
        assert_eq!(taffy.layout(cell0).unwrap().size.width, 100.0);
    }

    #[test]
    fn auto_table_does_not_stretch_in_block_parent() {
        // A table child of a block container must shrink-to-fit, not stretch-fit
        // like a regular block child (no `item_is_table` flag needed).
        let mut taffy: TaffyTree<()> = TaffyTree::new();
        let cell = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size::from_lengths(100.0, 30.0),
                ..Default::default()
            })
            .unwrap();
        let row =
            taffy.new_with_children(Style { display: Display::TableRow, ..Default::default() }, &[cell]).unwrap();
        let table =
            taffy.new_with_children(Style { display: Display::Table, ..Default::default() }, &[row]).unwrap();
        let block = taffy
            .new_with_children(
                Style {
                    display: Display::Block,
                    size: Size { width: Dimension::from_length(500.0), height: Dimension::AUTO },
                    ..Default::default()
                },
                &[table],
            )
            .unwrap();

        taffy.compute_layout(block, Size::MAX_CONTENT).unwrap();

        let table_layout = taffy.layout(table).unwrap();
        assert_eq!(table_layout.size.width, 100.0, "Auto table should shrink-to-fit, got {}", table_layout.size.width);
    }

    #[test]
    fn table_margins_not_double_subtracted_in_block_parent() {
        // Block parents already subtract the child's margins from the available
        // space they pass down; the table must not subtract them again. With a
        // 500px block, 50px margins each side and wrappable content (min 200 /
        // max 400), the table gets 400px of space: exactly max-content.
        let mut taffy: TaffyTree<()> = TaffyTree::new();
        let (table, ..) = wrappable_table(
            &mut taffy,
            Style {
                display: Display::Table,
                margin: Rect {
                    left: LengthPercentageAuto::length(50.0),
                    right: LengthPercentageAuto::length(50.0),
                    top: LengthPercentageAuto::ZERO,
                    bottom: LengthPercentageAuto::ZERO,
                },
                ..Default::default()
            },
        );
        let block = taffy
            .new_with_children(
                Style {
                    display: Display::Block,
                    size: Size { width: Dimension::from_length(500.0), height: Dimension::AUTO },
                    ..Default::default()
                },
                &[table],
            )
            .unwrap();

        taffy.compute_layout(block, Size::MAX_CONTENT).unwrap();

        let table_layout = taffy.layout(table).unwrap();
        assert_eq!(
            table_layout.size.width, 400.0,
            "Table should get 500 - 2*50 = 400px (no double margin subtraction), got {}",
            table_layout.size.width
        );
        assert_eq!(table_layout.size.height, 20.0, "Content should not wrap at 400px");
    }

    #[test]
    fn consecutive_direct_cells_share_anonymous_row() {
        // CSS 2.1 §17.2.1: consecutive cells without a row wrapper share ONE
        // anonymous row — side by side, not stacked.
        let mut taffy: TaffyTree<()> = TaffyTree::new();
        let cell0 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size::from_lengths(100.0, 30.0),
                ..Default::default()
            })
            .unwrap();
        let cell1 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size::from_lengths(150.0, 30.0),
                ..Default::default()
            })
            .unwrap();
        let table = taffy
            .new_with_children(
                Style {
                    display: Display::Table,
                    border_spacing: Size { width: LengthPercentage::length(10.0), height: LengthPercentage::length(10.0) },
                    ..Default::default()
                },
                &[cell0, cell1],
            )
            .unwrap();

        taffy.compute_layout(table, Size::MAX_CONTENT).unwrap();

        let table_layout = taffy.layout(table).unwrap();
        let cell0_layout = taffy.layout(cell0).unwrap();
        let cell1_layout = taffy.layout(cell1).unwrap();

        // One row: 3 h-gaps (10) + 100 + 150 wide, 2 v-gaps + 30 tall
        assert_eq!(table_layout.size.width, 280.0, "Table width should be 280, got {}", table_layout.size.width);
        assert_eq!(table_layout.size.height, 50.0, "Table height should be 50, got {}", table_layout.size.height);

        // Cells are children of the table, positioned in table coordinates
        assert_eq!(cell0_layout.location.x, 10.0);
        assert_eq!(cell0_layout.location.y, 10.0);
        assert_eq!(cell0_layout.size.width, 100.0);
        assert_eq!(cell1_layout.location.x, 120.0);
        assert_eq!(cell1_layout.location.y, 10.0);
        assert_eq!(cell1_layout.size.width, 150.0);
    }

    #[test]
    fn non_table_child_wrapped_as_single_cell() {
        // A non-table child of a table is wrapped in ONE anonymous cell; its own
        // children must not be promoted to cells/columns.
        let mut taffy: TaffyTree<()> = TaffyTree::new();
        let inner_a = taffy.new_leaf(Style { size: Size::from_lengths(60.0, 10.0), ..Default::default() }).unwrap();
        let inner_b = taffy.new_leaf(Style { size: Size::from_lengths(60.0, 10.0), ..Default::default() }).unwrap();
        let div = taffy
            .new_with_children(Style { display: Display::Block, ..Default::default() }, &[inner_a, inner_b])
            .unwrap();
        let table =
            taffy.new_with_children(Style { display: Display::Table, ..Default::default() }, &[div]).unwrap();

        taffy.compute_layout(table, Size::MAX_CONTENT).unwrap();

        let table_layout = taffy.layout(table).unwrap();
        let div_layout = taffy.layout(div).unwrap();

        // One column (60 wide), one row with two stacked 10px leaves (20 tall)
        assert_eq!(table_layout.size.width, 60.0, "div should be one cell/column, got width {}", table_layout.size.width);
        assert_eq!(table_layout.size.height, 20.0);
        assert_eq!(div_layout.size.width, 60.0);
        assert_eq!(div_layout.size.height, 20.0);
    }

    #[cfg(feature = "content_size")]
    #[test]
    fn content_size_propagated_from_cells() {
        // A cell whose content overflows must report the real content size, and the
        // table's own layout must reflect it (used for scroll ranges).
        let mut taffy: TaffyTree<()> = TaffyTree::new();
        let big = taffy.new_leaf(Style { size: Size::from_lengths(300.0, 90.0), ..Default::default() }).unwrap();
        let cell = taffy
            .new_with_children(
                Style {
                    display: Display::TableCell,
                    size: Size { width: Dimension::from_length(100.0), height: Dimension::from_length(30.0) },
                    ..Default::default()
                },
                &[big],
            )
            .unwrap();
        let row =
            taffy.new_with_children(Style { display: Display::TableRow, ..Default::default() }, &[cell]).unwrap();
        let table =
            taffy.new_with_children(Style { display: Display::Table, ..Default::default() }, &[row]).unwrap();

        taffy.compute_layout(table, Size::MAX_CONTENT).unwrap();

        let cell_layout = taffy.layout(cell).unwrap();
        assert_eq!(cell_layout.content_size.width, 300.0, "Cell content_size.width should be 300");
        assert_eq!(cell_layout.content_size.height, 90.0, "Cell content_size.height should be 90");

        let row_layout = taffy.layout(row).unwrap();
        assert!(
            row_layout.content_size.width >= 300.0,
            "Row content_size.width should include overflowing cell content, got {}",
            row_layout.content_size.width
        );

        let table_layout = taffy.layout(table).unwrap();
        assert!(
            table_layout.content_size.width >= 300.0,
            "Table content_size.width should include overflowing cell content, got {}",
            table_layout.content_size.width
        );
    }

    #[test]
    fn column_width_is_max_of_cell_widths() {
        // The column's specified width is the max across its cells, not whichever
        // cell came first.
        let mut taffy: TaffyTree<()> = TaffyTree::new();
        let cell_narrow = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size { width: Dimension::from_length(80.0), height: Dimension::from_length(20.0) },
                ..Default::default()
            })
            .unwrap();
        let cell_wide = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size { width: Dimension::from_length(140.0), height: Dimension::from_length(20.0) },
                ..Default::default()
            })
            .unwrap();
        let row0 =
            taffy.new_with_children(Style { display: Display::TableRow, ..Default::default() }, &[cell_narrow]).unwrap();
        let row1 =
            taffy.new_with_children(Style { display: Display::TableRow, ..Default::default() }, &[cell_wide]).unwrap();
        let table = taffy
            .new_with_children(Style { display: Display::Table, ..Default::default() }, &[row0, row1])
            .unwrap();

        taffy.compute_layout(table, Size::MAX_CONTENT).unwrap();

        assert_eq!(taffy.layout(cell_narrow).unwrap().size.width, 140.0, "Column should use the max specified width");
        assert_eq!(taffy.layout(cell_wide).unwrap().size.width, 140.0);
    }

    #[test]
    fn rowspan_occupies_grid_and_distributes_height() {
        // Cell A spans 2 rows and is 80px tall; the second row's cell must be
        // placed in column 1 (col 0 is occupied), and A's extra height (80 vs
        // 30+30) is distributed to the spanned rows (40/40).
        let mut taffy: TaffyTree<()> = TaffyTree::new();
        let cell_a = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                rowspan: 2,
                size: Size::from_lengths(100.0, 80.0),
                ..Default::default()
            })
            .unwrap();
        let cell_b = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size::from_lengths(100.0, 30.0),
                ..Default::default()
            })
            .unwrap();
        let cell_c = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size::from_lengths(100.0, 30.0),
                ..Default::default()
            })
            .unwrap();
        let row0 = taffy
            .new_with_children(Style { display: Display::TableRow, ..Default::default() }, &[cell_a, cell_b])
            .unwrap();
        let row1 =
            taffy.new_with_children(Style { display: Display::TableRow, ..Default::default() }, &[cell_c]).unwrap();
        let table = taffy
            .new_with_children(Style { display: Display::Table, ..Default::default() }, &[row0, row1])
            .unwrap();

        taffy.compute_layout(table, Size::MAX_CONTENT).unwrap();

        let table_layout = taffy.layout(table).unwrap();
        assert_eq!(table_layout.size.width, 200.0, "Two 100px columns");
        assert_eq!(table_layout.size.height, 80.0, "Rows grow to fit the spanning cell (40 + 40)");

        // C lands in column 1, not column 0
        assert_eq!(taffy.layout(cell_c).unwrap().location.x, 100.0, "Row-1 cell must skip the occupied column 0");
        // A's box spans both rows
        assert_eq!(taffy.layout(cell_a).unwrap().size.height, 80.0);
        // Rows got the distributed height
        assert_eq!(taffy.layout(row0).unwrap().size.height, 40.0);
        assert_eq!(taffy.layout(row1).unwrap().size.height, 40.0);
        assert_eq!(taffy.layout(row1).unwrap().location.y, 40.0);
        // B stretches to its row's height, not the full span
        assert_eq!(taffy.layout(cell_b).unwrap().size.height, 40.0);
    }

    #[test]
    fn border_collapse_suppresses_spacing() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();
        let cell0 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size::from_lengths(100.0, 30.0),
                ..Default::default()
            })
            .unwrap();
        let cell1 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size::from_lengths(100.0, 30.0),
                ..Default::default()
            })
            .unwrap();
        let row = taffy
            .new_with_children(Style { display: Display::TableRow, ..Default::default() }, &[cell0, cell1])
            .unwrap();
        let table = taffy
            .new_with_children(
                Style {
                    display: Display::Table,
                    border_collapse: BorderCollapse::Collapse,
                    border_spacing: Size { width: LengthPercentage::length(10.0), height: LengthPercentage::length(10.0) },
                    ..Default::default()
                },
                &[row],
            )
            .unwrap();

        taffy.compute_layout(table, Size::MAX_CONTENT).unwrap();

        let table_layout = taffy.layout(table).unwrap();
        assert_eq!(table_layout.size.width, 200.0, "border-spacing must not apply when collapsed");
        assert_eq!(table_layout.size.height, 30.0);
        assert_eq!(taffy.layout(cell0).unwrap().location.x, 0.0);
        assert_eq!(taffy.layout(cell1).unwrap().location.x, 100.0);
    }

    #[test]
    fn cell_align_content_gives_vertical_align() {
        // Cells are block containers laid out at the full row height, so
        // align_content on a cell behaves like vertical-align (HTML valign).
        let mut taffy: TaffyTree<()> = TaffyTree::new();
        let content = taffy.new_leaf(Style { size: Size::from_lengths(50.0, 20.0), ..Default::default() }).unwrap();
        let valign_middle_cell = taffy
            .new_with_children(
                Style {
                    display: Display::TableCell,
                    align_content: Some(AlignContent::CENTER),
                    ..Default::default()
                },
                &[content],
            )
            .unwrap();
        let tall_cell = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size::from_lengths(50.0, 100.0),
                ..Default::default()
            })
            .unwrap();
        let row = taffy
            .new_with_children(Style { display: Display::TableRow, ..Default::default() }, &[valign_middle_cell, tall_cell])
            .unwrap();
        let table =
            taffy.new_with_children(Style { display: Display::Table, ..Default::default() }, &[row]).unwrap();

        taffy.compute_layout(table, Size::MAX_CONTENT).unwrap();

        assert_eq!(taffy.layout(valign_middle_cell).unwrap().size.height, 100.0, "Cell fills the row height");
        assert_eq!(
            taffy.layout(content).unwrap().location.y,
            40.0,
            "align_content: center should vertically center the 20px content in the 100px cell"
        );
    }

    #[test]
    fn captions_stack_above_and_below_grid() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();
        let top_caption = taffy
            .new_leaf(Style {
                display: Display::TableCaption,
                size: Size { width: Dimension::AUTO, height: Dimension::from_length(30.0) },
                ..Default::default()
            })
            .unwrap();
        let bottom_caption = taffy
            .new_leaf(Style {
                display: Display::TableCaption,
                caption_side: CaptionSide::Bottom,
                size: Size { width: Dimension::AUTO, height: Dimension::from_length(20.0) },
                ..Default::default()
            })
            .unwrap();
        let cell = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size::from_lengths(200.0, 50.0),
                ..Default::default()
            })
            .unwrap();
        let row =
            taffy.new_with_children(Style { display: Display::TableRow, ..Default::default() }, &[cell]).unwrap();
        let table = taffy
            .new_with_children(
                Style { display: Display::Table, ..Default::default() },
                &[top_caption, row, bottom_caption],
            )
            .unwrap();

        taffy.compute_layout(table, Size::MAX_CONTENT).unwrap();

        let table_layout = taffy.layout(table).unwrap();
        assert_eq!(table_layout.size.width, 200.0);
        assert_eq!(table_layout.size.height, 100.0, "grid 50 + top caption 30 + bottom caption 20");

        let top_layout = taffy.layout(top_caption).unwrap();
        assert_eq!(top_layout.location.y, 0.0);
        assert_eq!(top_layout.size.width, 200.0, "Caption spans the full table width");
        assert_eq!(top_layout.size.height, 30.0);

        assert_eq!(taffy.layout(row).unwrap().location.y, 30.0, "Grid is shifted below the top caption");

        let bottom_layout = taffy.layout(bottom_caption).unwrap();
        assert_eq!(bottom_layout.location.y, 80.0, "Bottom caption goes after the 30+50 grid");
        assert_eq!(bottom_layout.size.height, 20.0);
    }

    #[test]
    fn col_elements_provide_column_widths() {
        // <colgroup><col width=120><col></colgroup> — the first column takes its
        // width from the col element even though its cells are narrower.
        let mut taffy: TaffyTree<()> = TaffyTree::new();
        let col0 = taffy
            .new_leaf(Style {
                display: Display::TableColumn,
                size: Size { width: Dimension::from_length(120.0), height: Dimension::AUTO },
                ..Default::default()
            })
            .unwrap();
        let col1 = taffy.new_leaf(Style { display: Display::TableColumn, ..Default::default() }).unwrap();
        let colgroup = taffy
            .new_with_children(Style { display: Display::TableColumnGroup, ..Default::default() }, &[col0, col1])
            .unwrap();

        let cell0 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size::from_lengths(50.0, 30.0),
                ..Default::default()
            })
            .unwrap();
        let cell1 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size::from_lengths(50.0, 30.0),
                ..Default::default()
            })
            .unwrap();
        let row = taffy
            .new_with_children(Style { display: Display::TableRow, ..Default::default() }, &[cell0, cell1])
            .unwrap();
        let table = taffy
            .new_with_children(Style { display: Display::Table, ..Default::default() }, &[colgroup, row])
            .unwrap();

        taffy.compute_layout(table, Size::MAX_CONTENT).unwrap();

        assert_eq!(taffy.layout(cell0).unwrap().size.width, 120.0, "Column 0 takes the col element's 120px width");
        assert_eq!(taffy.layout(cell1).unwrap().size.width, 50.0, "Column 1 stays at its cell's width");
        assert_eq!(taffy.layout(table).unwrap().size.width, 170.0);
        // Column nodes generate no boxes
        assert_eq!(taffy.layout(colgroup).unwrap().size.width, 0.0);
        assert_eq!(taffy.layout(col0).unwrap().size.width, 0.0);
    }

    #[test]
    fn specified_cell_height_is_a_minimum_not_a_cap() {
        // <td style="width:30px;height:30px"><img width=60 height=60></td>
        // CSS 2.1 §17.5.3: height on a cell is a minimum for the row, so the 60px
        // image grows the row rather than overflowing a 30px cell.
        let mut taffy: TaffyTree<()> = TaffyTree::new();

        let img = taffy.new_leaf(Style { size: Size::from_lengths(60.0, 60.0), ..Default::default() }).unwrap();
        let cell = taffy
            .new_with_children(
                Style {
                    display: Display::TableCell,
                    size: Size::from_lengths(30.0, 30.0),
                    ..Default::default()
                },
                &[img],
            )
            .unwrap();
        let row = taffy
            .new_with_children(Style { display: Display::TableRow, ..Default::default() }, &[cell])
            .unwrap();
        let table = taffy
            .new_with_children(Style { display: Display::Table, ..Default::default() }, &[row])
            .unwrap();

        taffy.compute_layout(table, Size::MAX_CONTENT).unwrap();

        assert_eq!(taffy.layout(cell).unwrap().size, Size { width: 60.0, height: 60.0 }, "Cell grows to its content");
        assert_eq!(taffy.layout(row).unwrap().size.height, 60.0, "Row height is the cell's content height");
        assert_eq!(taffy.layout(table).unwrap().size, Size { width: 60.0, height: 60.0 });
        // The image no longer overflows the cell
        assert_eq!(taffy.layout(img).unwrap().location.y, 0.0);
    }

    #[test]
    fn cell_taller_than_specified_height_still_honours_it_as_a_floor() {
        // A cell whose content is shorter than its specified height keeps that height
        let mut taffy: TaffyTree<()> = TaffyTree::new();

        let content = taffy.new_leaf(Style { size: Size::from_lengths(20.0, 10.0), ..Default::default() }).unwrap();
        let cell = taffy
            .new_with_children(
                Style {
                    display: Display::TableCell,
                    size: Size::from_lengths(40.0, 50.0),
                    ..Default::default()
                },
                &[content],
            )
            .unwrap();
        let row = taffy
            .new_with_children(Style { display: Display::TableRow, ..Default::default() }, &[cell])
            .unwrap();
        let table = taffy
            .new_with_children(Style { display: Display::Table, ..Default::default() }, &[row])
            .unwrap();

        taffy.compute_layout(table, Size::MAX_CONTENT).unwrap();

        assert_eq!(taffy.layout(cell).unwrap().size, Size { width: 40.0, height: 50.0 });
    }

    #[test]
    fn nested_table_min_content_width_reaches_outer_cell() {
        // MJML social-icon footer:
        //   <td style="padding:4px 10px"><table style="width:30px"><tr>
        //     <td style="width:30px;height:30px"><img width=60 height=60></td>
        //   </tr></table></td>
        // The inner table's used width is max(30px, min-content 60px) = 60px, and it
        // is that used width — not the specified 30px — which the outer cell sizes
        // itself from (CSS 2.1 §17.5.2.2).
        let mut taffy: TaffyTree<()> = TaffyTree::new();

        let img = taffy.new_leaf(Style { size: Size::from_lengths(60.0, 60.0), ..Default::default() }).unwrap();
        let inner_cell = taffy
            .new_with_children(
                Style {
                    display: Display::TableCell,
                    size: Size::from_lengths(30.0, 30.0),
                    ..Default::default()
                },
                &[img],
            )
            .unwrap();
        let inner_row = taffy
            .new_with_children(Style { display: Display::TableRow, ..Default::default() }, &[inner_cell])
            .unwrap();
        let inner_table = taffy
            .new_with_children(
                Style {
                    display: Display::Table,
                    size: Size { width: Dimension::from_length(30.0), height: Dimension::AUTO },
                    ..Default::default()
                },
                &[inner_row],
            )
            .unwrap();

        let outer_cell = taffy
            .new_with_children(
                Style {
                    display: Display::TableCell,
                    padding: Rect {
                        left: LengthPercentage::from_length(10.0),
                        right: LengthPercentage::from_length(10.0),
                        top: LengthPercentage::from_length(4.0),
                        bottom: LengthPercentage::from_length(4.0),
                    },
                    ..Default::default()
                },
                &[inner_table],
            )
            .unwrap();
        let outer_row = taffy
            .new_with_children(Style { display: Display::TableRow, ..Default::default() }, &[outer_cell])
            .unwrap();
        let outer_table = taffy
            .new_with_children(Style { display: Display::Table, ..Default::default() }, &[outer_row])
            .unwrap();

        taffy.compute_layout(outer_table, Size::MAX_CONTENT).unwrap();

        assert_eq!(taffy.layout(inner_table).unwrap().size, Size { width: 60.0, height: 60.0 });
        assert_eq!(
            taffy.layout(outer_cell).unwrap().size,
            Size { width: 80.0, height: 68.0 },
            "Outer cell wraps the inner table's 60x60 used size plus its 10/4 padding"
        );
        assert_eq!(taffy.layout(outer_table).unwrap().size, Size { width: 80.0, height: 68.0 });
    }
}
