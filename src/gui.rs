#![allow(non_snake_case)]

use crate::squad_tracker::SquadTracker;
use arcdps::imgui::{im_str, ImStr, ImString, TableFlags, Ui, Window};

// Shamelessly stolen from https://github.com/gw2scratch/arcdps-clears
pub fn centered_text(pUi: &Ui, pText: &ImStr) {
    let current_x = pUi.cursor_pos()[0];
    let text_width = pUi.calc_text_size(&pText, false, -1.0)[0];
    let column_width = pUi.current_column_width();
    let new_x = (current_x + column_width / 2. - text_width / 2.).max(current_x);
    pUi.set_cursor_pos([new_x, pUi.cursor_pos()[1]]);
    pUi.text(pText);
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
        TableFlags::BORDERS | TableFlags::NO_HOST_EXTEND_X | TableFlags::SORTABLE,
    );

    pUi.table_setup_column(&ImString::new("account_name"));
    pUi.table_setup_column(&ImString::new("current_ready_check_time"));
    pUi.table_setup_column(&ImString::new("total_ready_check_time"));
    pUi.table_headers_row();

    for (account_name, member_state) in pSquadTracker.get_squad_members() {
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
