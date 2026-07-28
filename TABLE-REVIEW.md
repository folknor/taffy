# Table layout review findings — 2026-07-28

Review of the `table_layout` feature (`src/compute/table.rs` + integration) done after
merging upstream/main (v0.10 → v0.12.2). **This file is intentionally not committed.**

Context: primary consumer is HTML-email rendering (litehtml-rs pipeline), which is why
the explicit-width/cellspacing happy path works and these gaps went unnoticed.

Status: `[ ]` open · `[x]` fixed · `[~]` in progress

## Bugs

### B1. `content_size` discarded everywhere — `[x]`
`src/compute/table.rs`: Phase 4 ignores `_cell_output.content_size` (~line 448) and
hardcodes `content_size: Size::ZERO` in the manually-built `Layout` for cells (~480),
row groups (~542), and rows (~576). The table itself returns
`LayoutOutput::from_outer_size` (zero content size). `content_size` is a default-on
feature used for scroll ranges → any scroll container in/around a table sees scroll
area 0.
**Fix:** plumb `cell_output.content_size` into cell layouts; compute row/group/table
content size from child extents (mirror how `block.rs` accumulates content size).
Return `LayoutOutput::from_sizes(...)`.

### B2. `item_is_table` vs `Display::Table` disconnect — `[x]`
`BlockItemStyle::is_table()` for `Style` reads only the pre-existing `item_is_table`
bool (`src/style/mod.rs:892`). Block layout uses it to skip stretch-sizing
(`block.rs:934`) and BFC/margin-collapse participation (`block.rs:654-659`). A
`display: Table` node in a block parent without `item_is_table: true` gets
stretch-sized → auto table silently fills container.
**Fix (two parts):**
- `BlockItemStyle::is_table()` → `self.item_is_table || matches!(self.display, Display::Table)` (cfg table_layout).
- `CoreStyle::is_block()` should NOT match `Display::Table` (keep Row/RowGroup/Cell as
  block fallbacks). Consumers checked: `block.rs:426` + `leaf.rs:76`
  (collapse-through: tables must never be collapsed through → `!is_block()` true is
  correct), `block.rs:654` (same-BFC: tables establish their own → false is correct),
  `compute/mod.rs:80` (root stretch: root tables must shrink-to-fit, not stretch to
  viewport → skipping the block branch is correct).

### B3. Full `PerformLayout` of cells during measure passes — `[x]`
Phase 3 (`table.rs:390`) calls `perform_child_layout` (RunMode::PerformLayout, writes
unrounded layouts into cell subtrees) even when the table is only being measured
(`run_mode == ComputeSize`). Upstream fixed this exact pattern for block in #971/#972.
**Fix:** Phase 3 should only *measure* (RunMode::ComputeSize via
`tree.compute_child_layout` with a hand-built LayoutInput — we need the LayoutOutput
for height AND baselines, see B9, so `measure_child_size_both` is not enough). Phase 4
remains the only place that performs layout.

### B4. Direct-cell-as-row: double `set_unrounded_layout`, wrong grouping — `[x]`
Direct `TableCell` children of a table are each registered as their *own* single-cell
row ("the cell node doubles as the row"). Two problems:
- Per CSS 2.1 §17.2.1 *consecutive* orphan cells share ONE anonymous row (side by
  side); current code stacks them vertically (test only passes because it doesn't
  assert positions).
- The node's layout is written twice: cell loop positions it at its column offset,
  then row loop overwrites with row layout (x = padding_border.left, full row width).
  Wrong x/width whenever border-spacing ≠ 0 or columns differ.
**Fix:** rows become `Option<NodeId>` (None = anonymous row with no real node); track
per-cell whether parent is the table (position such cells in table coords:
`x = col_x_offsets[col]`, `y = row_y_offsets[row]`); consecutive orphan cells
accumulate into the current anonymous row; row layout loop skips anonymous rows.

### B5. Double margin subtraction for tables in block parents — `[x]`
`table.rs:293` subtracts `margin.horizontal_axis_sum()` from Definite available width,
but block layout already subtracts the item's margins from `stretch_width`
(`block.rs:922`) before passing it as available space; flexbox likewise pre-subtracts
child margins at measure sites. Taffy convention: *parent* subtracts child margins.
**Fix:** drop the margin subtraction in table.rs. (Trade-off: root tables with margins
under Definite available space get margins un-subtracted — root passes raw available —
but table-in-block is by far the common case, incl. email body tables.)

### B6. Non-table children of a table treated as rows, their children promoted to cells — `[x]`
`table.rs:179` else-branch calls `collect_cells_from_row` on any non-cell/row/group
child, so a plain block div with 3 children becomes a 3-column row. CSS 2.1 §17.2.1:
the child itself gets wrapped in an anonymous *cell* (which joins the current
anonymous row run, same machinery as B4).
**Fix:** treat any non-row/non-row-group table child as a cell (colspan from style if
`is_table_cell`, else 1) in the current anonymous row run.
Note: `collect_cells_from_row` treating all *row* children as cells is CORRECT
(anonymous cell wrapping inside rows) — leave that.

### B7. Table used width doesn't reach ancestor intrinsic sizing — `[x]`
`block.rs` `determine_content_based_container_width` took a child's specified width
as its intrinsic contribution without measuring it. For a table that is wrong: the
used width is `max(specified, GRIDMIN)` (CSS 2.1 §17.5.2.2), so a `width:30px` table
whose column min-content is 60px laid out at 60 while its block parent sized itself
from 30 — the child overflowed the parent by 30px.
**Fix:** pass `Size::NONE` as `known_dimensions` for `is_table` items so they are
measured and resolve their own size styles, matching what the final layout pass at
`block.rs:934` already does.

### B8. Specified cell height capped the row instead of flooring it — `[x]`
Phase 3 measured cells with `SizingMode::InherentSize`, so a `height:30px` cell
measured 30 no matter how tall its content was, and `row_heights` never saw it.
CSS 2.1 §17.5.3: height on a cell is a *minimum* for the row.
**Fix:** measure with `SizingMode::ContentSize` (the cell then disregards its own
size styles) and apply the resolved specified height as a floor afterwards. Note the
floor must also cover `min_size`, which `ContentSize` drops for *leaf* cells (see
`leaf.rs:38-43`) though block cells still apply it.
**Trap:** measuring the cell both ways instead does not work — `Cache::get` for
`RunMode::ComputeSize` matches on the packed known-dimensions/available-space key and
the parent *width* only (`cache.rs:229-239`); `sizing_mode`, `axis` and the parent
height are all absent from the comparison, so both calls share one cache entry.
Still open: `height` on a *row* is ignored entirely (only cells raise row heights).

### B9. Specified column width raised the table's minimum — `[x]`
`col_min_w` returned `max(fixed, min_content)` under automatic layout, so a
`<td width="1456">` holding a 550px image made GRIDMIN 1456 and
`table_width = max(candidate, outer_min)` blew the table past its 550px containing
block (repro: Substack emails carry the image's original width on the `td` while the
`img` is author-sized down; litehtml-rs `gmail_gullinbursti_dividend`,
`gmail_steam_purchase` table[12]). Chrome: a specified column width feeds the table's
*preferred* width only; the minimum is content-driven, so used width =
`max(GRIDMIN_content, min(avail, GRIDMAX))` = 550.
**Fix:** `col_min_w` is `min_content_width` for every column under automatic layout
(fixed layout unchanged). `distribute_column_widths` then has to cope with a target
below the specified widths: specified-width columns float at min-content and share
whatever is left after the auto columns' minimums, proportional to how much they asked
for above min-content.
Note the asymmetry with B7: a width on the *table* still floors at GRIDMIN; a width on
a *column* must not.

## CSS fidelity gaps

### F1. Table min-content width == max-content width (no shrinking) — `[x]`
Under `AvailableSpace::MinContent`, `available_for_columns` is 0.0 and auto columns
still resolve to max-content (`resolve_column_widths` fallthrough). Tables never
shrink below max-content → overflow instead of wrapping in narrow containers, and
overstated min-content contribution to flex/grid parents. Biggest remaining algorithm
work. CSS 2.1 §17.5.2.2:
- col min = max cell min-content (have it); col max = max(cell max-content, fixed).
- table min = Σ col-min + spacing; table max = Σ col-max + spacing.
- used width: explicit → max(specified, table-min); Definite avail →
  max(table-min, min(avail, table-max)); MaxContent → table-max; MinContent → table-min.
- distribute: fixed ≥ min (unless fixed layout); percent = pct·W clamped ≥ min; auto
  cols get min + share of (W − Σmin) proportional to (max − min); excess beyond Σmax
  (explicit-width tables) distributed to auto cols.
**Fix:** restructure `resolve_column_widths` around the min/max sums; delete the
separate "redistribute extra space" block (~354-373) which the distribution subsumes.

### F2. Column width type: first cell wins, should be max across cells — `[x]`
`table.rs:239-254` only assigns Fixed/Percent from the first cell encountered per
column (guarded on `ColumnWidthType::Auto`). Spec/browsers use the max of the
column's cells' specified widths; percent generally beats fixed.
**Fix:** take max fixed / max percent across cells; percent takes priority over fixed.

### F3. Spanning cells contribute only min-content — `[x]`
Colspan > 1 cells: fixed/percent widths and max-content are ignored; only a post-hoc
min-content expansion runs (~305-345). Acceptable simplification short-term; at least
distribute the spanning cell's *max*-content into columns during the intrinsic phase
so F1's sums see it.

### F4. No baselines — `[x]` (implemented; no direct test — needs measure-func content to produce baselines)
Table returns `first_baselines: NONE`. Upstream now propagates baselines through
block layout (#996), so cells produce them. CSS: table baseline = baseline of first
row (= max cell baseline in row 0).
**Fix:** capture `first_baselines` from cell measure outputs (see B3 — use
compute_child_layout so we get LayoutOutput), table baseline =
`row_y_offsets[0] + max(first-row cell baseline)`; return via
`from_sizes_and_baselines`. Needed for both ComputeSize and PerformLayout returns.

### F5. Unsupported features — `[x]` IMPLEMENTED 2026-07-28 (second commit)
- rowspan ✓ (grid placement w/ occupancy, deficit height distribution; rowspan=0 → 1)
- captions ✓ (`Display::TableCaption`, `caption_side` Top/Bottom, stack outside grid
  height; caption min-width does NOT influence table width; caption-only tables
  render empty)
- `<col>` / `<colgroup>` widths ✓ (`Display::TableColumn`/`TableColumnGroup`,
  BoxGenerationMode::None, `colspan` doubles as `span`; hints extend the grid)
- `vertical-align` ✓ via `align_content` on the cell (cells are block containers at
  full row height, upstream #959 gives top/middle/bottom); TRUE baseline
  vertical-align still unsupported — needs cross-cell content shifting
- `border-collapse` ✓ as approximation: `BorderCollapse::Collapse` suppresses
  border-spacing only; adjacent borders not merged (taffy has no border styles)
- percent columns don't influence auto table width; resolved against content width
  rather than used table width
- `table-layout: fixed` still scans all rows, not just the first
- non-row children inside row groups are assumed to be rows

## Minor / cleanups

### M1. `row_heights` manual push loop → `vec![0.0; num_rows]` (~377) — `[x]`
### M2. Row `order` uses global row index; should be index within parent (group) — `[x]`
Direct-cell layouts should use the table child index as `order`.
### M3. Style borrow dance (`child_style_2`, `cell_style_2` get/drop/re-get) — `[x]`
### M4. Empty-table branch zeroes only direct children, not grandchildren — `[ ]`
Stale layouts possible on cells of empty rows. Low priority.

## Testing gaps

### T1. No gentest fixtures (`test_fixtures/` has no table dir) — `[ ]`
The Chrome-comparison harness is the strongest validation in this repo. Requires
running the gentest tooling; separate session.
### T2. Missing hand-written coverage — `[x]` (all except baseline, see F4)
- content_size (scroll container in table / overflowing cell content) → B1
- table inside block parent: no stretch, no double margin subtraction → B2/B5
- two direct cells side by side + border-spacing ≠ 0 → B4
- non-table child wrapped as single anonymous cell (div w/ children ≠ columns) → B6
- min-content / narrow-container shrinking; MinContent available space → F1
- percent columns, mixed with fixed and auto → F1/F2
- baseline: table in baseline-aligned flex row → F4

## Suggested order

1. B2 (style/trait changes, self-contained) ✚ tests
2. B1 + B3 + F4 together (all reshape phases 3/4 output plumbing) ✚ tests
3. B5 (one-line) ✚ test
4. B4 + B6 together (same anonymous-row machinery) ✚ tests
5. F1 + F2 + F3 together (column algorithm rework) ✚ tests
6. M1–M4, F5 doc update
7. T1 in a separate session (needs Chrome gentest harness)

Parallelization notes (if delegating to agents): nearly everything touches
`src/compute/table.rs`, so don't run two agents on it concurrently. Safe splits:
one agent on style/trait layer (B2), one on tests (T2 scaffolding), main session on
table.rs. Per house rules: agents edit code only — building/testing happens in the
main session.
