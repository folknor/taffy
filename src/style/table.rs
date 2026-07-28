//! Style types for Table layout
use crate::geometry::Size;
use crate::style::LengthPercentage;
use crate::CoreStyle;

/// CSS `table-layout` property (CSS 2.1 §17.5.2)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum TableLayout {
    /// Column widths are determined by content (automatic table layout algorithm)
    #[default]
    Auto,
    /// Column widths are determined by the first row's specified widths, not by content
    Fixed,
}

/// CSS `border-collapse` property (CSS 2.1 §17.6)
///
/// Note: `Collapse` is approximated: border-spacing is suppressed, but adjacent cell
/// borders are not merged/overlapped (taffy does not model border styles, which
/// border conflict resolution requires).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum BorderCollapse {
    /// Cells have their own borders, separated by border-spacing
    #[default]
    Separate,
    /// Adjacent cell borders are collapsed; border-spacing does not apply
    Collapse,
}

/// CSS `caption-side` property (CSS 2.1 §17.4.1)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum CaptionSide {
    /// The caption box is positioned above the table grid
    #[default]
    Top,
    /// The caption box is positioned below the table grid
    Bottom,
}

/// The set of styles required for a Table layout container
pub trait TableContainerStyle: CoreStyle {
    /// The spacing between table cells (maps to CSS border-spacing / HTML cellspacing)
    #[inline(always)]
    fn border_spacing(&self) -> Size<LengthPercentage> {
        Size::zero()
    }

    /// The table layout algorithm to use (CSS `table-layout` property)
    #[inline(always)]
    fn table_layout(&self) -> TableLayout {
        TableLayout::Auto
    }

    /// The border model to use (CSS `border-collapse` property)
    #[inline(always)]
    fn border_collapse(&self) -> BorderCollapse {
        BorderCollapse::Separate
    }
}

/// The set of styles required for a Table layout item (child of a Table container:
/// rows, row groups, cells, captions, columns, column groups)
pub trait TableItemStyle: CoreStyle {
    /// The number of columns this cell spans (colspan attribute).
    /// Also used as the `span` attribute for table columns.
    #[inline(always)]
    fn colspan(&self) -> u16 {
        1
    }

    /// The number of rows this cell spans (rowspan attribute)
    #[inline(always)]
    fn rowspan(&self) -> u16 {
        1
    }

    /// Whether this item is a table row
    #[inline(always)]
    fn is_table_row(&self) -> bool {
        false
    }

    /// Whether this item is a table row group (thead, tbody, tfoot)
    #[inline(always)]
    fn is_table_row_group(&self) -> bool {
        false
    }

    /// Whether this item is a table cell
    #[inline(always)]
    fn is_table_cell(&self) -> bool {
        false
    }

    /// Whether this item is a table caption
    #[inline(always)]
    fn is_table_caption(&self) -> bool {
        false
    }

    /// Whether this item is a table column (col element)
    #[inline(always)]
    fn is_table_column(&self) -> bool {
        false
    }

    /// Whether this item is a table column group (colgroup element)
    #[inline(always)]
    fn is_table_column_group(&self) -> bool {
        false
    }

    /// Which side of the table this item's caption box goes on
    /// (CSS `caption-side` property, read from the caption item)
    #[inline(always)]
    fn caption_side(&self) -> CaptionSide {
        CaptionSide::Top
    }
}
