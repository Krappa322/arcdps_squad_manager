#![allow(non_snake_case)]

use crate::squad_tracker::SquadTracker;
use arcdps::imgui;

pub fn draw(pUi: &imgui::Ui, pSquadTracker: &SquadTracker) {
    imgui::Window::new(&imgui::ImString::new("Squad Manager###SQUAD_MANAGER_MAIN"))
        .always_auto_resize(true)
        .focus_on_appearing(false)
        .no_nav()
        .collapsible(false)
        .build(&pUi, || {
            for (account_name, member_state) in pSquadTracker.get_squad_members() {
                pUi.text(format!(
                    "{} - {:#?} - {:#?}",
                    account_name,
                    member_state
                        .current_ready_check_time
                        .unwrap_or(std::time::Duration::new(0, 0)),
                    member_state.total_ready_check_time
                ));
            }
        });
}
