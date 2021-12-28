#![allow(non_snake_case)]

use crate::{
    imgui_ex,
    squad_tracker::{SquadMemberState, SquadTracker},
    updates::{install_update, UpdateInfo, UpdateStatus, tag_to_version_num},
    NEW_UPDATE,
};
use arcdps::{
    imgui::{im_str, ImString, TableFlags, Ui, Window},
    UserRole,
};
use std::{
    cmp::Ordering,
    time::{Duration, Instant},
};

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
    if pState.main_window_open == true {
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

    let mut raw_update = NEW_UPDATE.write();
    if let Some(update) = raw_update.as_mut() {
        let mut open = true;
        Window::new(&ImString::new("Squad Manager###SQUAD_MANAGER_UPDATE"))
            .always_auto_resize(true)
            .focus_on_appearing(false)
            .no_nav()
            .collapsible(false)
            .opened(&mut open)
            .build(&pUi, || {
                draw_update_window(pUi, update);
            });

        if open == false {
            *raw_update = None;
        }
    }
}

fn draw_ready_check_tab(pUi: &Ui, pSquadTracker: &SquadTracker) {
    pUi.begin_table_with_flags(
        &ImString::new("ready_check_table"),
        3,
        TableFlags::BORDERS
            | TableFlags::NO_HOST_EXTEND_X
            | TableFlags::SORTABLE
            | TableFlags::SORT_MULTI
            | TableFlags::SORT_TRISTATE,
    );

    pUi.table_setup_column(&ImString::new("Account Name"));
    pUi.table_setup_column(&ImString::new("Current Ready Check"));
    pUi.table_setup_column(&ImString::new("Total Time Unready"));
    pUi.table_headers_row();

    let mut users: Vec<(&String, &SquadMemberState, Option<Duration>)> = Vec::new();
    let mut ready_check_start_time: Option<Instant> = None;
    for (account_name, user_state) in pSquadTracker.get_squad_members() {
        if user_state.role == UserRole::SquadLeader && user_state.is_ready == true {
            ready_check_start_time = user_state.last_ready_time;
        }
        users.push((account_name, user_state, user_state.last_unready_duration));
    }

    let now = Instant::now();
    if let Some(start_time) = ready_check_start_time {
        for (_account_name, user_state, last_unready_duration) in users.iter_mut() {
            let ready_time = if user_state.is_ready == true {
                user_state.last_ready_time.unwrap()
            } else {
                now
            };

            *last_unready_duration = Some(ready_time.max(start_time) - start_time);
        }
    }

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
                    1 => lhs.2.cmp(&rhs.2),
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

    for (account_name, member_state, last_unready_duration) in users {
        pUi.table_next_column();
        pUi.text(&ImString::new(account_name));
        pUi.table_next_column();

        const GREEN: [f32; 4] = [0.0, 0.75, 0.0, 1.0];
        const RED: [f32; 4] = [0.85, 0.0, 0.0, 1.0];
        const GRAY: [f32; 4] = [0.62, 0.62, 0.62, 1.0];

        if let Some(last_unready_duration) = last_unready_duration {
            let color = if ready_check_start_time.is_some() {
                if member_state.is_ready {
                    GREEN
                } else {
                    RED
                }
            } else {
                GRAY
            };
            imgui_ex::centered_text_colored(
                pUi,
                color,
                &im_str!(
                    "{:2}.{}s",
                    last_unready_duration.as_secs(),
                    last_unready_duration.subsec_millis() / 100
                ),
            );
        }

        pUi.table_next_column();
        imgui_ex::centered_text(
            pUi,
            &im_str!(
                "{:2}.{}s",
                member_state.total_ready_check_time.as_secs(),
                member_state.total_ready_check_time.subsec_millis() / 100
            ),
        );
    }

    pUi.end_table();
}

fn draw_update_window(pUi: &Ui, pUpdate: &mut UpdateInfo) {
    const RED: [f32; 4] = [0.85, 0.0, 0.0, 1.0];
    const GREEN: [f32; 4] = [0.0, 0.85, 0.0, 1.0];

    pUi.text_colored(
        RED,
        im_str!("A new update for the squad manager addon is available"),
    );
    pUi.text_colored(
        RED,
        im_str!("Current version: {}", env!("CARGO_PKG_VERSION")),
    );
    pUi.text_colored(
        GREEN,
        im_str!("New version: {}", tag_to_version_num(&pUpdate.newer_release.tag_name)),
    );

    match &pUpdate.status {
        UpdateStatus::UpdateAvailable(_) => {
            if pUi.button(im_str!("Update automatically"), [0.0, 0.0]) == true {
                install_update(pUpdate);
            }
        }
        UpdateStatus::Downloading => pUi.text("Downloading update"),
        UpdateStatus::Updating => pUi.text("Installing update"),
        UpdateStatus::RestartPending => pUi.text_colored(
            GREEN,
            im_str!("Update finished, restart Guild Wars 2 for the update to take effect"),
        ),
        UpdateStatus::UpdateError(e) => pUi.text_colored(RED, im_str!("Update failed - {}", e)),
    }
}

pub fn draw_options(pUi: &Ui, pState: &mut GuiState) {
    pUi.checkbox(
        &ImString::new("Squad Manager"),
        &mut pState.main_window_open,
    );
}
