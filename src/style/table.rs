//! Style types for Table layout
use crate::geometry::Size;
use crate::style::LengthPercentage;
use crate::CoreStyle;

/// The set of styles required for a Table layout container
pub trait TableContainerStyle: CoreStyle {
    /// The spacing between table cells (maps to CSS border-spacing / HTML cellspacing)
    #[inline(always)]
    fn border_spacing(&self) -> Size<LengthPercentage> {
        Size::zero()
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
