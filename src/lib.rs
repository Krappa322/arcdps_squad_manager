#![feature(atomic_from_mut)]
#![allow(non_snake_case)]

#[macro_use]
mod infra;
mod gui;
mod squad_tracker;

use arcdps::arcdps_export;
use arcdps::UserInfoIter;
use arcdps::imgui;
use infra::*;
use squad_tracker::SquadTracker;
use static_init::dynamic;
use winapi::um::consoleapi;

arcdps_export! {
    name: "Squad Manager",
    sig: 0x88ef8f68u32,
    imgui: imgui,
    init: init,
    release: release,
    unofficial_extras_init: unofficial_extras_init,
    unofficial_extras_squad_update: unofficial_extras_squad_update,
}

#[dynamic]
static mut SQUAD_TRACKER: Option<SquadTracker> = None;

fn unofficial_extras_init(
    pSelfAccountName: Option<&str>,
    pUnofficialExtrasVersion: Option<&'static str>,
) {
    if let Some(name) = pSelfAccountName {
        *SQUAD_TRACKER.write() = Some(SquadTracker::new(name));
    }

    info!(
        "Initialized - pSelfAccountName={:?} pUnofficialExtrasVersion={:?}",
        pSelfAccountName, pUnofficialExtrasVersion
    );
}

fn unofficial_extras_squad_update(pUsers: UserInfoIter) {
    if let Some(tracker) = &mut *SQUAD_TRACKER.write() {
        tracker.squad_update(pUsers);
    }
}

fn init() {
    unsafe {
        consoleapi::AllocConsole();
    }

    match install_log_handler() {
        Ok(_) => println!("Starting log succeeded"),
        Err(e) => println!("Starting log failed {}", e),
    }
    info!("{}", "started logger");

    install_panic_handler();
    info!("{}", "started panic handler");
}

fn release() {
    info!("release");
}

fn imgui(pUi: &imgui::Ui, pNotChararacterSelectOrLoading: bool) {
    if pNotChararacterSelectOrLoading == false {
        return;
    }

    if let Some(tracker) = SQUAD_TRACKER.read().as_ref() {
        gui::draw(pUi, tracker);
    }
}