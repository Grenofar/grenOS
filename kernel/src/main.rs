#![no_std]
#![no_main]

mod serial;

use serial::{init, write_str};

#[repr(C)]
struct LimineBootInfoRequest {
    id: [u64; 4],
    revision: u64,
    response: *mut LimineBootInfoResponse,
}

#[repr(C)]
struct LimineBootInfoResponse {
    revision: u64,
    name: *const u8,
    version: *const u8,
}

static mut BOOT_INFO_RESPONSE: LimineBootInfoResponse = LimineBootInfoResponse {
    revision: 0,
    name: core::ptr::null(),
    version: core::ptr::null(),
};

#[used]
[link_section = ".requests"]
static BOOT_INFO_REQUEST: LimineBootInfoRequest = LimineBootInfoRequest {
    id: [0xf4, 0x3b, 0xda, 0x5b, 0x00, 0x00, 0x00, 0x00],
    revision: 0,
    response: &mut BOOT_INFO_RESPONSE as *mut LimineBootInfoResponse,
};

#[no_mangle]
pub extern "C" fn _start() -> ! {
    init();
    write_str("grenOS\n");
    loop { unsafe { core::arch::x86_64::hlt(); } }
}
