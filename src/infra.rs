#![allow(non_snake_case)]

use backtrace::Backtrace;
use flexi_logger::filter::{LogLineFilter, LogLineWriter};
use flexi_logger::DeferredNow;
use static_init::dynamic;
use std::ffi::CString;
use std::mem::size_of;
use std::sync::atomic::{AtomicU64, Ordering};
use winapi::shared::ntdef::TRUE;
use winapi::um::dbghelp;
use winapi::um::errhandlingapi::GetLastError;
use winapi::um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE};
use winapi::um::memoryapi::{MapViewOfFile, UnmapViewOfFile, FILE_MAP_ALL_ACCESS};
use winapi::um::processthreadsapi::{GetCurrentProcess, GetCurrentThreadId};
use winapi::um::winbase::CreateFileMappingA;
use winapi::um::winnt::PAGE_READWRITE;

#[macro_export]
macro_rules! function_name_no_crate {
    () => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let raw_name = type_name_of(f);
        let crate_name_end_index = raw_name.find(':').unwrap();
        // 2 is the length of "::" delimiting the crate name and the function name start
        // 3 is the length of "::f"
        &raw_name[crate_name_end_index + 2..raw_name.len() - 3]
    }};
}

#[macro_export]
macro_rules! assert_in_range {
    ($value:expr, $min:expr, $max:expr) => {
        more_asserts::assert_ge!($value, $min);
        more_asserts::assert_le!($value, $max);
    };
}

#[macro_export]
macro_rules! trace {
    ($fmtstring:tt, $($arg:tt)*) => (
        log::trace!(std::concat!("{}|", $fmtstring), function_name_no_crate!(), $($arg)*)
    );
    ($fmtstring:tt) => (
        log::trace!(std::concat!("{}|", $fmtstring), function_name_no_crate!())
    )
}

#[macro_export]
macro_rules! debug {
    ($fmtstring:tt, $($arg:tt)*) => (
        log::debug!(std::concat!("{}|", $fmtstring), function_name_no_crate!(), $($arg)*)
    );
    ($fmtstring:tt) => (
        log::debug!(std::concat!("{}|", $fmtstring), function_name_no_crate!())
    )
}

#[macro_export]
macro_rules! info {
    ($fmtstring:tt, $($arg:tt)*) => (
        log::info!(std::concat!("{}|", $fmtstring), function_name_no_crate!(), $($arg)*)
    );
    ($fmtstring:tt) => (
        log::info!(std::concat!("{}|", $fmtstring), function_name_no_crate!())
    )
}

#[macro_export]
macro_rules! warn {
    ($fmtstring:tt, $($arg:tt)*) => (
        log::warn!(std::concat!("{}|", $fmtstring), function_name_no_crate!(), $($arg)*)
    );
    ($fmtstring:tt) => (
        log::warn!(std::concat!("{}|", $fmtstring), function_name_no_crate!())
    )
}

#[macro_export]
macro_rules! error {
    ($fmtstring:tt, $($arg:tt)*) => (
        log::error!(std::concat!("{}|", $fmtstring), function_name_no_crate!(), $($arg)*)
    );
    ($fmtstring:tt) => (
        log::error!(std::concat!("{}|", $fmtstring), function_name_no_crate!())
    )
}

fn get_current_thread_id() -> u32 {
    unsafe {
        return GetCurrentThreadId();
    }
}

fn get_global_sequence() -> Option<u64> {
    let shared_file_name = CString::new(r"Local\arcdps_squad_manager_ProcessSeq").unwrap();

    unsafe {
        let file_handle = CreateFileMappingA(
            INVALID_HANDLE_VALUE,    // use paging file
            std::ptr::null_mut(),    // default security
            PAGE_READWRITE,          // read/write access
            0,                       // maximum object size (high-order DWORD)
            size_of::<u64>() as u32, // maximum object size (low-order DWORD)
            shared_file_name.as_ptr(),
        ); // name of mapping object
        if file_handle.is_null() {
            error!("CreateFileMappingA failed with {}", GetLastError());
            return None;
        }

        let buf_void = MapViewOfFile(
            file_handle,         // handle to map object
            FILE_MAP_ALL_ACCESS, // read/write permission
            0,
            0,
            size_of::<u64>(),
        );
        let seq = match (buf_void as *mut u64).as_mut() {
            Some(x) => AtomicU64::from_mut(x),
            None => {
                error!("MapViewOfFile failed with {}", GetLastError());
                CloseHandle(file_handle);
                return None;
            }
        };

        let result_seq = seq.fetch_add(1, Ordering::AcqRel);

        UnmapViewOfFile(buf_void);
        // Leak file_handle, which ensures that it stays alive for the duration of the module lifetime. If we wanted to
        // make it even more robust, it should be captured in a global variable and CloseHandle()'d in mod_release

        Some(result_seq)
    }
}

#[dynamic]
static mut LOGGER: Option<flexi_logger::LoggerHandle> = None;

pub struct LogFilter;
impl LogLineFilter for LogFilter {
    fn write(
        &self,
        now: &mut DeferredNow,
        record: &log::Record,
        log_line_writer: &dyn LogLineWriter,
    ) -> std::io::Result<()> {
        if let Some(module_path) = record.module_path() {
            if module_path.starts_with("arcdps_squad_manager") {
                log_line_writer.write(now, record)?;
            }
        }
        Ok(())
    }
}

pub fn install_log_handler() -> Result<(), flexi_logger::FlexiLoggerError> {
    use flexi_logger::*;

    let mut logger = LOGGER.write();
    if logger.is_some() {
        return Ok(()); // Return OK value in case logger is already initialized
    }

    *logger = Some(
        Logger::try_with_str("debug")?
            .log_to_file(
                FileSpec::default()
                    .directory("addons/logs/arcdps_squad_manager")
                    .basename("arcdps_squad_manager")
                    .discriminant(get_global_sequence().unwrap_or(u64::MAX).to_string()),
            )
            .rotate(
                Criterion::AgeOrSize(Age::Day, 128 * 1024 * 1024),
                Naming::Numbers,
                Cleanup::KeepCompressedFiles(16),
            )
            .format(|write, now, record| {
                let format = time::macros::format_description!(
                    "[month repr:short] [day] [hour repr:24]:[minute]:[second].[subsecond digits:6]"
                );
                write.write_fmt(format_args!(
                    "{time} {thread_id} {level:.1} {message}",
                    time = now
                        .now()
                        .format(&format)
                        .unwrap_or("Unknown time".to_string()),
                    thread_id = get_current_thread_id(),
                    level = record.level(),
                    message = &record.args()
                ))
            })
            .filter(Box::new(LogFilter))
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
