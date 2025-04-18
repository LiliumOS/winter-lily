use core::panic::PanicInfo;

use lilium_sys::uuid::parse_uuid;

use crate::helpers::exit_unrecoverably;

#[panic_handler]
fn at_panic(info: &PanicInfo) -> ! {
    // TODO: Print panic
    exit_unrecoverably(Some(parse_uuid("01964aac-9df4-7745-8585-6b9e2fc929d8")))
}
