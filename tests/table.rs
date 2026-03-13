#[cfg(feature = "table_layout")]
mod table_tests {
    use taffy::prelude::*;
    use taffy::style::{Display, LengthPercentage};

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
}
