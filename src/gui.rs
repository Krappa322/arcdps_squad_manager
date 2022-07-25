#![allow(non_snake_case)]

use crate::{
    chat_log::ChatLog,
    imgui_ex,
    squad_tracker::{SquadMemberState, SquadTracker},
    updates::{install_update, tag_to_version_num, UpdateInfo, UpdateStatus},
    NEW_UPDATE,
};
use arcdps::{
    imgui::{ImString, TableFlags, Ui, Window, TableColumnSetup, Id, TableColumnFlags},
    ChannelType, UserRole,
};
use chrono::Local;
use std::{
    cmp::Ordering,
    time::{Duration, Instant},
};

pub struct GuiState {
    ready_check_window_open: bool,
    chat_log_window_open: bool,
    chat_log_wrap_width: f32,
}

impl GuiState {
    pub fn new() -> Self {
        Self {
            ready_check_window_open: false,
            chat_log_window_open: false,
            chat_log_wrap_width: 600.0,
        }
    }
}

pub fn draw(pUi: &Ui, pState: &mut GuiState, pSquadTracker: &SquadTracker, pChatLog: &ChatLog) {
    if pState.ready_check_window_open == true {
        Window::new(&ImString::new("Squad Manager###SQUAD_MANAGER_READY_CHECK"))
            .always_auto_resize(true)
            .focus_on_appearing(false)
            .no_nav()
            .collapsible(false)
            .opened(&mut pState.ready_check_window_open)
            .build(&pUi, || {
                draw_ready_check_tab(pUi, pSquadTracker);
            });
    }

    if pState.chat_log_window_open == true {
        Window::new(&ImString::new("Chat Log###SQUAD_MANAGER_CHAT_LOG"))
            .always_auto_resize(true)
            .focus_on_appearing(false)
            .no_nav()
            .collapsible(false)
            .opened(&mut pState.chat_log_window_open)
            .build(&pUi, || {
                draw_chat_log(pUi, pChatLog, pState.chat_log_wrap_width);
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
    let _table_ref = pUi.begin_table_with_flags(
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
                format!(
                    "{:2}.{}s",
                    last_unready_duration.as_secs(),
                    last_unready_duration.subsec_millis() / 100
                ),
            );
        }

        pUi.table_next_column();
        imgui_ex::centered_text(
            pUi,
            format!(
                "{:2}.{}s",
                member_state.total_ready_check_time.as_secs(),
                member_state.total_ready_check_time.subsec_millis() / 100
            ),
        );
    }
}

fn draw_chat_log(pUi: &Ui, pChatLog: &ChatLog, pChatLogWrapWidth: f32) {
    let _table_ref = pUi.begin_table_with_sizing(
        "chat_log",
        5,
        TableFlags::NO_BORDERS_IN_BODY
        | TableFlags::HIDEABLE // TODO: Use custom context menu instead
        | TableFlags::REORDERABLE // TODO: Use custom context menu instead
        | TableFlags::CONTEXT_MENU_IN_BODY
        | TableFlags::SIZING_FIXED_FIT
        | TableFlags::SCROLL_Y,
        [0.0, 400.0],
        0.0,
    );

    for name in ["To", "Time", "Account", "Character"] {
        pUi.table_setup_column_with(TableColumnSetup {
            name,
            flags: TableColumnFlags::empty(),
            init_width_or_weight: 0.0,
            user_id: Id::Int(0)});
    }
    pUi.table_setup_column_with(TableColumnSetup {
        name: "Message",
        flags: TableColumnFlags::empty(),
        init_width_or_weight: pChatLogWrapWidth,
        user_id: Id::Int(0)});
    pUi.table_headers_row();

    let mut messages = pChatLog.get_all_messages();
    messages.sort_unstable_by_key(|v| v.1.timestamp);

    for (channel, msg) in messages {
        pUi.table_next_column();
        let mut subgroup_str = match channel.channel_type {
            ChannelType::Party => "P".to_string(),
            ChannelType::Squad => {
                if channel.subgroup == u8::MAX {
                    "S".to_string()
                } else {
                    (channel.subgroup + 1).to_string()
                }
            }
            _ => "?".to_string(),
        };
        if msg.is_broadcast {
            subgroup_str += " (B)";
        }
        imgui_ex::centered_text(pUi, &subgroup_str);

        pUi.table_next_column();
        imgui_ex::centered_text(pUi, msg.timestamp.with_timezone(&Local).format("%X").to_string());

        pUi.table_next_column();
        imgui_ex::centered_text(pUi, &msg.account_name);

        pUi.table_next_column();
        imgui_ex::centered_text(pUi,&msg.character_name);

        pUi.table_next_column();
        pUi.text_wrapped(&msg.text);
    }
}

fn draw_update_window(pUi: &Ui, pUpdate: &mut UpdateInfo) {
    const RED: [f32; 4] = [0.85, 0.0, 0.0, 1.0];
    const GREEN: [f32; 4] = [0.0, 0.85, 0.0, 1.0];

    pUi.text_colored(RED, "A new update for the squad manager addon is available");
    pUi.text_colored(
        RED,
        format!("Current version: {}", env!("CARGO_PKG_VERSION")),
    );
    pUi.text_colored(
        GREEN,
        format!(
            "New version: {}",
            tag_to_version_num(&pUpdate.newer_release.tag_name)
        ),
    );

    match &pUpdate.status {
        UpdateStatus::UpdateAvailable(_) => {
            if pUi.button("Update automatically") == true {
                install_update(pUpdate);
            }
        }
        UpdateStatus::Downloading => pUi.text("Downloading update"),
        UpdateStatus::Updating => pUi.text("Installing update"),
        UpdateStatus::RestartPending => pUi.text_colored(
            GREEN,
            "Update finished, restart Guild Wars 2 for the update to take effect",
        ),
        UpdateStatus::UpdateError(e) => pUi.text_colored(RED, format!("Update failed - {}", e)),
    }
}

pub fn draw_options(pUi: &Ui, pState: &mut GuiState) {
    pUi.checkbox(
        &ImString::new("Squad Manager"),
        &mut pState.ready_check_window_open,
    );
    pUi.checkbox(&ImString::new("Chat Log"), &mut pState.chat_log_window_open);
}
