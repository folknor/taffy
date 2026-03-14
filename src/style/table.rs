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
}

/// The set of styles required for a Table layout item (child of a Table container: rows, row groups, cells)
pub trait TableItemStyle: CoreStyle {
    /// The number of columns this cell spans (colspan attribute)
    #[inline(always)]
    fn colspan(&self) -> u16 {
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
}
