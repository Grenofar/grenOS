#![no_std]
#![no_main]

use core::panic::PanicInfo;
use limine::{BaseRevision, request::{BootloaderInfoRequest, StackSizeRequest}};

// Limine requests
pub static BASE_REVISION: BaseRevision = BaseRevision::new();
pub static STACK_SIZE_REQUEST: StackSizeRequest = StackSizeRequest::new().with_size(0x100000);
pub static BOOTLOADER_INFO_REQUEST: BootloaderInfoRequest = BootloaderInfoRequest::new();

#[no_mangle]
pub extern "C" fn kmain() -> ! {
    // Initialize serial
    serial::init();
    // Write message
    serial::write_str("grenOS\n");
    // Halt
    loop {
        unsafe {
            core::arch::asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        unsafe {
            core::arch::asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}
