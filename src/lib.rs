#[macro_use]
mod infra;

use arcdps::arcdps_export;
use infra::*;
use winapi::um::consoleapi;

arcdps_export! {
    name: "Squad Manager",
    sig: 0x88ef8f68u32,
    init: init,
    release: release
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

    let x: Result<u32, &str> = Err("emergency failure");
    x.expect("Testing expect"); // panics with `Testing expect: emergency failure`

    info!("{}", "how did this logline happen?");
}

fn release() {
    // info!("release")
}
