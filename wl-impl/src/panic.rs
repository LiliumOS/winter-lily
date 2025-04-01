use core::panic::PanicInfo;

use crate::helpers::exit_unrecoverably;

#[panic_handler]
fn at_panic(info: &PanicInfo) -> ! {
    // TODO: Print panic
    exit_unrecoverably()
}
