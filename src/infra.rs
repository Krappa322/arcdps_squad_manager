#![allow(non_snake_case)]

use backtrace::Backtrace;
use lazy_static::lazy_static;
use std::sync::Mutex;
use winapi::shared::ntdef::TRUE;
use winapi::um::dbghelp;
use winapi::um::processthreadsapi::{GetCurrentProcess, GetCurrentThreadId};

#[macro_export]
macro_rules! trace {
    ($fmtstring:tt, $($arg:tt)*) => (
        log::trace!(std::concat!("{}|", $fmtstring), stdext::function_name!(), $($arg)*);
    )
}

#[macro_export]
macro_rules! debug {
    ($fmtstring:tt, $($arg:tt)*) => (
        log::debug!(std::concat!("{}|", $fmtstring), stdext::function_name!(), $($arg)*);
    )
}

#[macro_export]
macro_rules! info {
    ($fmtstring:tt, $($arg:tt)*) => (
        log::info!(std::concat!("{}|", $fmtstring), stdext::function_name!(), $($arg)*);
    )
}

#[macro_export]
macro_rules! warn {
    ($fmtstring:tt, $($arg:tt)*) => (
        log::warn!(std::concat!("{}|", $fmtstring), stdext::function_name!(), $($arg)*);
    )
}

#[macro_export]
macro_rules! error {
    ($fmtstring:tt, $($arg:tt)*) => (
        log::error!(std::concat!("{}|", $fmtstring), stdext::function_name!(), $($arg)*);
    )
}

fn get_current_thread_id() -> u32 {
    unsafe {
        return GetCurrentThreadId();
    }
}

lazy_static! {
    static ref LOGGER: Mutex<Option<flexi_logger::LoggerHandle>> = Mutex::new(None);
}

pub fn install_log_handler() -> Result<(), flexi_logger::FlexiLoggerError> {
    use flexi_logger::*;

    *LOGGER.lock().unwrap() = Some(
        Logger::try_with_str("info")?
            .log_to_file(
                FileSpec::default()
                    .directory("addons/logs/arcdps_squad_manager")
                    .basename("arcdps_squad_manager"),
            )
            .rotate(
                Criterion::AgeOrSize(Age::Day, 128 * 1024 * 1024),
                Naming::Numbers,
                Cleanup::KeepCompressedFiles(16),
            )
            .format(|write, now, record| {
                write.write_fmt(format_args!(
                    "{time} {thread_id} {level:.1} {message}",
                    time = now.now().format("%b %d %H:%M:%S.%6f"),
                    thread_id = get_current_thread_id(),
                    level = record.level(),
                    message = &record.args()
                ))
            })
            .write_mode(WriteMode::Direct)
            .start()?,
    );

    Ok(())
}

pub fn install_panic_handler() {
    std::panic::set_hook(Box::new(panic_handler));
}

fn panic_handler(pPanicInfo: &std::panic::PanicInfo) {
    unsafe {
        let result = dbghelp::SymCleanup(GetCurrentProcess());
        info!("SymCleanup returned {}", result);
        let result = dbghelp::SymInitializeW(GetCurrentProcess(), std::ptr::null(), TRUE.into());
        info!("SymInitializeW returned {}", result);
    }

    let bt = Backtrace::new();
    error!("Caught panic \"{}\"", pPanicInfo);

    for (i, frame) in bt.frames().into_iter().enumerate() {
        // Resolve module name
        let mut mod_info: dbghelp::IMAGEHLP_MODULEW64;
        unsafe {
            mod_info = std::mem::zeroed::<dbghelp::IMAGEHLP_MODULEW64>();
            mod_info.SizeOfStruct = std::mem::size_of_val(&mod_info) as u32;
            dbghelp::SymGetModuleInfoW64(GetCurrentProcess(), frame.ip() as u64, &mut mod_info);
        }

        let symbol_name = frame
            .symbols()
            .first()
            .and_then(|x| x.name().map(|y| format!("{}", y)))
            .unwrap_or("<unknown function>".to_string());

        let symbol_offset = frame
            .symbols()
            .first()
            .and_then(|x| x.addr().map(|y| frame.ip() as u64 - y as u64))
            .unwrap_or(0x0);

        let file_name = frame
            .symbols()
            .first()
            .and_then(|x| x.filename().and_then(|y| y.to_str()))
            .unwrap_or("<unknown file>");

        let line = frame
            .symbols()
            .first()
            .and_then(|x| x.lineno())
            .unwrap_or(0);

        let module_path_buf = std::path::PathBuf::from(String::from_utf16_lossy(
            mod_info
                .ImageName
                .iter()
                .take_while(|c| **c != 0) // truncate string to null termination
                .map(|c| *c)
                .collect::<Vec<_>>()
                .as_slice(),
        ));
        let module_name = module_path_buf
            .file_name()
            .and_then(|y| y.to_str())
            .unwrap_or("<unknown module>");

        let module_offset = if mod_info.BaseOfImage > 0 {
            frame.ip() as u64 - mod_info.BaseOfImage
        } else {
            0x0
        };

        error!(
            "{i}: {module_name}+0x{module_offset:x}({symbol_name}+0x{symbol_offset:x}) [0x{addr:x}] {file}:{line}",
            i = i,
            module_name = module_name,
            module_offset = module_offset,
            symbol_name = symbol_name,
            symbol_offset = symbol_offset,
            addr = frame.ip() as u64,
            file = file_name,
            line = line,
        );
    }
}
