#![feature(atomic_from_mut)]
#![allow(non_snake_case)]

#[macro_use]
mod infra;
mod gui;
mod imgui_ex;
mod squad_tracker;
mod chat_log;
mod updates;

use arcdps::ChatMessageInfo;
use arcdps::arcdps_export;
use arcdps::imgui;
use arcdps::UserInfoIter;
use chat_log::ChatLog;
use gui::GuiState;
use infra::*;
use squad_tracker::SquadTracker;
use static_init::dynamic;
use updates::{find_potential_update, UpdateInfo};

arcdps_export! {
    name: "Squad Manager",
    sig: 0x88ef8f68u32,
    options_windows: options_windows,
    imgui: imgui,
    init: init,
    release: release,
    unofficial_extras_init: unofficial_extras_init,
    unofficial_extras_squad_update: unofficial_extras_squad_update,
    unofficial_extras_chat_message: unofficial_extras_chat_message,
}

#[dynamic]
static mut SQUAD_TRACKER: Option<SquadTracker> = None;

#[dynamic]
static mut CHAT_LOG: Option<ChatLog> = None;

#[dynamic]
static mut GUI_STATE: Option<GuiState> = None;

#[dynamic]
static mut NEW_UPDATE: Option<UpdateInfo> = None;

fn unofficial_extras_init(
    pSelfAccountName: Option<&str>,
    pUnofficialExtrasVersion: Option<&'static str>,
) {
    if let Some(name) = pSelfAccountName {
        {
            let mut tracker = SQUAD_TRACKER.write();
            tracker.get_or_insert(SquadTracker::new(name));
        }
        {
            let mut chatlog = CHAT_LOG.write();
            chatlog.get_or_insert(ChatLog::new());
        }

        info!(
            "Initialized - pSelfAccountName={:?} pUnofficialExtrasVersion={:?}",
            pSelfAccountName, pUnofficialExtrasVersion
        );
    } else {
        error!(
            "Ignoring initialization - pSelfAccountName={:?} pUnofficialExtrasVersion={:?}",
            pSelfAccountName, pUnofficialExtrasVersion
        );
    }
}

fn unofficial_extras_chat_message(
    pChatMessage: &ChatMessageInfo,
) {
    if let Some(chatlog) = &mut *CHAT_LOG.write() {
        chatlog.add(pChatMessage);
    }
}

#[allow(dead_code)]
fn mock_unofficial_extras_init() {
    let mut tracker = SQUAD_TRACKER.write();
    let tracker = tracker.get_or_insert(SquadTracker::new("mock_self"));
    //tracker.setup_mock_data_active_ready_check();
    tracker.setup_mock_data_inactive_ready_check();

    info!("Initialized");
}

fn unofficial_extras_squad_update(pUsers: UserInfoIter) {
    if let Some(tracker) = &mut *SQUAD_TRACKER.write() {
        tracker.squad_update(pUsers);
    }
}

fn init() -> Result<(), Box<dyn std::error::Error>> {
    if let Err(e) = install_log_handler() {
        println!("Starting log failed {}", e);
    }
    info!("{}", "Started logger");

    install_panic_handler();
    info!("{}", "Started panic handler");

    find_potential_update();
    //mock_unofficial_extras_init();

    Ok(())
}

fn release() {
    info!("Release");
}

fn imgui(pUi: &imgui::Ui, pNotChararacterSelectOrLoading: bool) {
    if pNotChararacterSelectOrLoading == false {
        return;
    }

    let mut state = GUI_STATE.write();
    let state = state.get_or_insert(GuiState::new());

    let tracker = SQUAD_TRACKER.read();
    let chatlog = CHAT_LOG.read();
    if let Some((tracker, chatlog)) = tracker.as_ref().zip(chatlog.as_ref()) {
        gui::draw(pUi, state, tracker, chatlog);
    } else {
        debug!("Tried to render frame before initialization");
    }
}

fn options_windows(pUi: &imgui::Ui, pWindowName: Option<&str>) -> bool {
    if pWindowName.is_none() {
        let mut state = GUI_STATE.write();
        let state = state.get_or_insert(GuiState::new());

        gui::draw_options(pUi, state);
    }

    return false;
}
