use libsqlite3_sys as ffi;
use std::os::raw::{c_char, c_int, c_void};

// SQLITE_EXTENSION_INIT1
#[no_mangle]
#[allow(bad_style)]
static mut sqlite3_api: *mut c_void = 0 as *mut _;

#[no_mangle]
pub unsafe extern "C" fn sqlite3_placessqliteext_init(
    db: *mut ffi::sqlite3,
    err_str_ptr: *mut *mut c_char,
    p_api: *mut c_void,
) -> c_int {
    // SQLITE_EXTENSION_INIT2
    sqlite3_api = p_api;
    let mut err = ffi_support::ExternError::default();
    ffi_support::call_with_result(&mut err, || -> places::Result<()> {
        let conn = rusqlite::Connection::from_handle(db)?;
        places::db::db::define_functions(&conn).map(|_| ())
    });
    if !err.get_raw_message().is_null() && !err_str_ptr.is_null() {
        *err_str_ptr = err.get_raw_message() as *mut _; // XXX dodgy but shouldn't matter.
    }
    // mem::forget err here if we ever add a Drop for it!
    err.get_code().code() as c_int
}
