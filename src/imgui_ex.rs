use arcdps::imgui::{self, ImStr, Ui};
use std::marker::PhantomData;

// START COPIED FROM LATEST IMGUI-RS - https://github.com/imgui-rs/imgui-rs/blob/main/imgui/src/tables.rs#L737

/// A wrapper around table sort specs.
///
/// To use this simply, use [conditional_sort] and provide a closure --
/// if you should sort your data, then the closure will be ran and imgui
/// will be informed that your data is sorted.
///
/// For manual control (such as if sorting can fail), use [should_sort] to
/// check if you should sort your data, sort your data using [specs] for information
/// on how to sort it, and then [set_sorted] to indicate that the data is sorted.
///
/// [conditional_sort]: Self::conditional_sort
/// [should_sort]: Self::should_sort
/// [specs]: Self::specs
/// [set_sorted]: Self::set_sorted
pub struct TableSortSpecsMut<'ui>(*mut imgui::sys::ImGuiTableSortSpecs, PhantomData<Ui<'ui>>);

impl TableSortSpecsMut<'_> {
    /// Gets the specs for a given sort. In most scenarios, this will be a slice of 1 entry.
    pub fn specs(&self) -> Specs<'_> {
        let value =
            unsafe { std::slice::from_raw_parts((*self.0).Specs, (*self.0).SpecsCount as usize) };

        Specs(value)
    }

    // /// Returns true if the data should be sorted.
    // pub fn should_sort(&self) -> bool {
    //     unsafe { (*self.0).SpecsDirty }
    // }

    // /// Sets the internal flag that the data has been sorted.
    // pub fn set_sorted(&mut self) {
    //     unsafe {
    //         (*self.0).SpecsDirty = false;
    //     }
    // }

    // /// Provide a closure, which will receive the Specs for a sort.
    // ///
    // /// If you should sort the data, the closure will run, and ImGui will be
    // /// told that the data has been sorted.
    // ///
    // /// If you need manual control over sorting, consider using [should_sort], [specs],
    // /// and [set_sorted] youself.
    // ///
    // /// [should_sort]: Self::should_sort
    // /// [specs]: Self::specs
    // /// [set_sorted]: Self::set_sorted
    // pub fn conditional_sort(mut self, mut f: impl FnMut(Specs<'_>)) {
    //     let is_dirty = self.should_sort();

    //     if is_dirty {
    //         f(self.specs());
    //     }

    //     self.set_sorted();
    // }
}

/// A wrapper around a slice of [TableColumnSortSpecs].
///
/// This slice may be 0 if [TableFlags::SORT_TRISTATE] is true, may be > 1 is [TableFlags::SORT_MULTI] is true,
/// but is generally == 1.
///
/// Consume this struct as an iterator.
pub struct Specs<'a>(&'a [imgui::sys::ImGuiTableColumnSortSpecs]);

impl<'a> Specs<'a> {
    pub fn iter(self) -> impl Iterator<Item = TableColumnSortSpecs<'a>> {
        self.0.iter().map(|v| TableColumnSortSpecs(v))
    }
}

pub struct TableColumnSortSpecs<'a>(&'a imgui::sys::ImGuiTableColumnSortSpecs);
impl<'a> TableColumnSortSpecs<'a> {
    // /// User id of the column (if specified by a TableSetupColumn() call)
    // pub fn column_user_id(&self) -> imgui::sys::ImGuiID {
    //     self.0.ColumnUserID
    // }

    /// Index of the column
    pub fn column_idx(&self) -> usize {
        self.0.ColumnIndex as usize
    }

    // /// Index within parent [Specs] slice where this was found -- always stored in order starting
    // /// from 0, tables sorted on a single criteria will always have a 0 here.
    // ///
    // /// Generally, you don't need to access this, as it's the same as calling `specs.iter().enumerate()`.
    // pub fn sort_order(&self) -> usize {
    //     self.0.SortOrder as usize
    // }

    /// Gets the sort direction for the given column. This will nearly always be `Some` if you
    /// can access it.
    pub fn sort_direction(&self) -> Option<TableSortDirection> {
        match self.0.SortDirection() {
            0 => None,
            1 => Some(TableSortDirection::Ascending),
            2 => Some(TableSortDirection::Descending),
            _ => unimplemented!(),
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum TableSortDirection {
    Ascending,
    Descending,
}

/// Gets the sorting data for a table. This will be `None` when not sorting.
///
/// See the examples folder for how to use the sorting API.
pub fn table_sort_specs_mut<'a>(_pUi: &'a Ui) -> Option<TableSortSpecsMut<'a>> {
    unsafe {
        let value = imgui::sys::igTableGetSortSpecs();
        if value.is_null() {
            None
        } else {
            Some(TableSortSpecsMut(value, PhantomData))
        }
    }
}

// END COPIED FROM LATEST IMGUI-RS

// Shamelessly copied from https://github.com/gw2scratch/arcdps-clears
pub fn centered_text(pUi: &Ui, pText: &ImStr) {
    let current_x = pUi.cursor_pos()[0];
    let text_width = pUi.calc_text_size(&pText, false, -1.0)[0];
    let column_width = pUi.current_column_width();
    let new_x = (current_x + column_width / 2. - text_width / 2.).max(current_x);
    pUi.set_cursor_pos([new_x, pUi.cursor_pos()[1]]);
    pUi.text(pText);
}

pub fn centered_text_colored(pUi: &Ui, pColor: [f32; 4], pText: &ImStr) {
    let current_x = pUi.cursor_pos()[0];
    let text_width = pUi.calc_text_size(&pText, false, -1.0)[0];
    let column_width = pUi.current_column_width();
    let new_x = (current_x + column_width / 2. - text_width / 2.).max(current_x);
    pUi.set_cursor_pos([new_x, pUi.cursor_pos()[1]]);
    pUi.text_colored(pColor, pText);
}
