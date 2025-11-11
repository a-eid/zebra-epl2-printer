use std::error::Error;

/// Send raw bytes to the named printer. On non-Windows this function returns an error.
pub fn send_raw_to_printer(printer_name: &str, data: &[u8]) -> Result<(), Box<dyn Error>> {
    #[cfg(target_os = "windows")]
    {
        use std::ffi::OsStr;
        use std::iter::once;
        use std::os::windows::ffi::OsStrExt;
        use winapi::um::winspool::*;
        use winapi::shared::minwindef::*;
        use winapi::shared::ntdef::LPWSTR;
        use std::ptr::null_mut;

        // convert printer name and helper strings to wide
        let wide_name: Vec<u16> = OsStr::new(printer_name).encode_wide().chain(once(0)).collect();
        let wide_doc: Vec<u16> = OsStr::new("EPL Job").encode_wide().chain(once(0)).collect();
        let wide_raw: Vec<u16> = OsStr::new("RAW").encode_wide().chain(once(0)).collect();

        unsafe {
            let mut handle: *mut winapi::ctypes::c_void = null_mut();
            if OpenPrinterW(wide_name.as_ptr() as LPWSTR, &mut handle as *mut _ as *mut _, null_mut()) == 0 {
                return Err(Box::<dyn Error>::from("OpenPrinterW failed"));
            }

            let doc_info = DOC_INFO_1W {
                pDocName: wide_doc.as_ptr() as LPWSTR,
                pOutputFile: null_mut(),
                pDatatype: wide_raw.as_ptr() as LPWSTR, // RAW data type
            };

            let job = StartDocPrinterW(handle as *mut _, 1, &doc_info as *const _ as *mut _);
            if job == 0 {
                ClosePrinter(handle as *mut _);
                return Err(Box::<dyn Error>::from("StartDocPrinterW failed"));
            }

            if StartPagePrinter(handle as *mut _) == 0 {
                EndDocPrinter(handle as *mut _);
                ClosePrinter(handle as *mut _);
                return Err(Box::<dyn Error>::from("StartPagePrinter failed"));
            }

            let mut written: DWORD = 0;
            let ok = WritePrinter(
                handle as *mut _,
                data.as_ptr() as *mut _,
                data.len() as DWORD,
                &mut written as *mut DWORD,
            );

            EndPagePrinter(handle as *mut _);
            EndDocPrinter(handle as *mut _);
            ClosePrinter(handle as *mut _);

            if ok == 0 {
                return Err(Box::<dyn Error>::from("WritePrinter failed"));
            }
            Ok(())
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err(Box::<dyn Error>::from("send_raw_to_printer is only supported on Windows (uses Win32 spooler)"))
    }
}
