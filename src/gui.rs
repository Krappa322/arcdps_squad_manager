#![allow(non_snake_case)]

use crate::squad_tracker::SquadTracker;
use arcdps::imgui::{self, im_str, ImStr, ImString, TableFlags, Ui, Window};
use std::{cmp::Ordering, marker::PhantomData};

// Shamelessly stolen from https://github.com/gw2scratch/arcdps-clears
fn centered_text(pUi: &Ui, pText: &ImStr) {
    let current_x = pUi.cursor_pos()[0];
    let text_width = pUi.calc_text_size(&pText, false, -1.0)[0];
    let column_width = pUi.current_column_width();
    let new_x = (current_x + column_width / 2. - text_width / 2.).max(current_x);
    pUi.set_cursor_pos([new_x, pUi.cursor_pos()[1]]);
    pUi.text(pText);
}

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

    /// Returns true if the data should be sorted.
    pub fn should_sort(&self) -> bool {
        unsafe { (*self.0).SpecsDirty }
    }

    /// Sets the internal flag that the data has been sorted.
    pub fn set_sorted(&mut self) {
        unsafe {
            (*self.0).SpecsDirty = false;
        }
    }

    /// Provide a closure, which will receive the Specs for a sort.
    ///
    /// If you should sort the data, the closure will run, and ImGui will be
    /// told that the data has been sorted.
    ///
    /// If you need manual control over sorting, consider using [should_sort], [specs],
    /// and [set_sorted] youself.
    ///
    /// [should_sort]: Self::should_sort
    /// [specs]: Self::specs
    /// [set_sorted]: Self::set_sorted
    pub fn conditional_sort(mut self, mut f: impl FnMut(Specs<'_>)) {
        let is_dirty = self.should_sort();

        if is_dirty {
            f(self.specs());
        }

        self.set_sorted();
    }
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
    /// User id of the column (if specified by a TableSetupColumn() call)
    pub fn column_user_id(&self) -> imgui::sys::ImGuiID {
        self.0.ColumnUserID
    }

    /// Index of the column
    pub fn column_idx(&self) -> usize {
        self.0.ColumnIndex as usize
    }

    /// Index within parent [Specs] slice where this was found -- always stored in order starting
    /// from 0, tables sorted on a single criteria will always have a 0 here.
    ///
    /// Generally, you don't need to access this, as it's the same as calling `specs.iter().enumerate()`.
    pub fn sort_order(&self) -> usize {
        self.0.SortOrder as usize
    }

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
fn table_sort_specs_mut<'a>(pUi: &'a Ui) -> Option<TableSortSpecsMut<'a>> {
    unsafe {
        let value = imgui::sys::igTableGetSortSpecs();
        if value.is_null() {
            None
        } else {
            Some(TableSortSpecsMut(value, PhantomData))
        }
    }
}

pub struct GuiState {
    main_window_open: bool,
}

impl GuiState {
    pub fn new() -> Self {
        Self {
            main_window_open: true,
        }
    }
}

pub fn draw(pUi: &Ui, pState: &mut GuiState, pSquadTracker: &SquadTracker) {
    if pState.main_window_open == false {
        return;
    }

    Window::new(&ImString::new("Squad Manager###SQUAD_MANAGER_MAIN"))
        .always_auto_resize(true)
        .focus_on_appearing(false)
        .no_nav()
        .collapsible(false)
        .opened(&mut pState.main_window_open)
        .build(&pUi, || {
            draw_ready_check_tab(pUi, pSquadTracker);
        });
}

pub fn draw_ready_check_tab(pUi: &Ui, pSquadTracker: &SquadTracker) {
    pUi.begin_table_with_flags(
        &ImString::new("ready_check_table"),
        3,
        TableFlags::BORDERS
            | TableFlags::NO_HOST_EXTEND_X
            | TableFlags::SORTABLE
            | TableFlags::SORT_MULTI
            | TableFlags::SORT_TRISTATE,
    );

    pUi.table_setup_column(&ImString::new("account_name"));
    pUi.table_setup_column(&ImString::new("current_ready_check_time"));
    pUi.table_setup_column(&ImString::new("total_ready_check_time"));
    pUi.table_headers_row();

    let mut users: Vec<_> = pSquadTracker.get_squad_members().iter().collect();

    if let Some(sort_specs) = table_sort_specs_mut(pUi) {
        users.sort_by(|lhs, rhs| {
            for spec in sort_specs.specs().iter() {
                let sort_column = spec.column_idx();
                debug_assert!(sort_column <= 2);

                let sort_direction = spec
                    .sort_direction()
                    .unwrap_or(TableSortDirection::Ascending);

                let mut result = match sort_column {
                    0 => lhs.0.cmp(&rhs.0),
                    1 => lhs
                        .1
                        .current_ready_check_time
                        .cmp(&rhs.1.current_ready_check_time),
                    2 => lhs
                        .1
                        .total_ready_check_time
                        .cmp(&rhs.1.total_ready_check_time),
                    // Default to equal if column is invalid, which just lets the next sorter handle it instead
                    _ => Ordering::Equal,
                };
                if result == Ordering::Equal {
                    continue;
                }

                if sort_direction == TableSortDirection::Ascending {
                    result = result.reverse()
                };

                return result;
            }

            return Ordering::Equal;
        });
    }

    for (account_name, member_state) in users {
        pUi.table_next_column();
        pUi.text(&ImString::new(account_name));
        pUi.table_next_column();
        if let Some(current_ready_check_time) = member_state.current_ready_check_time {
            centered_text(pUi, &im_str!("{:#?}", current_ready_check_time));
        }
        pUi.table_next_column();
        centered_text(pUi, &im_str!("{:#?}", member_state.total_ready_check_time));
    }

    pUi.end_table();
}

pub fn draw_options(pUi: &Ui, pState: &mut GuiState) {
    pUi.checkbox(
        &ImString::new("Squad Manager"),
        &mut pState.main_window_open,
    );
}
