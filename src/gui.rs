#![allow(non_snake_case)]

use crate::{imgui_ex, squad_tracker::SquadTracker};
use arcdps::imgui::{im_str, ImString, TableFlags, Ui, Window};
use std::cmp::Ordering;

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

    if let Some(sort_specs) = imgui_ex::table_sort_specs_mut(pUi) {
        users.sort_by(|lhs, rhs| {
            for spec in sort_specs.specs().iter() {
                let sort_column = spec.column_idx();
                debug_assert!(sort_column <= 2);

                let sort_direction = spec
                    .sort_direction()
                    .unwrap_or(imgui_ex::TableSortDirection::Ascending);

                let mut result = match sort_column {
                    0 => lhs.0.cmp(&rhs.0),
                    // 1 => lhs
                    //     .1
                    //     .current_ready_check_time
                    //     .cmp(&rhs.1.current_ready_check_time),
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

                if sort_direction == imgui_ex::TableSortDirection::Ascending {
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
        // if let Some(current_ready_check_time) = member_state.current_ready_check_time {
        //     imgui_ex::centered_text(pUi, &im_str!("{:#?}", current_ready_check_time));
        // }
        pUi.table_next_column();
        imgui_ex::centered_text(pUi, &im_str!("{:#?}", member_state.total_ready_check_time));
    }

    pUi.end_table();
}

pub fn draw_options(pUi: &Ui, pState: &mut GuiState) {
    pUi.checkbox(
        &ImString::new("Squad Manager"),
        &mut pState.main_window_open,
    );
}
