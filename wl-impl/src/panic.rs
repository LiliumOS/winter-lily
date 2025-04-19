use core::panic::{Location, PanicInfo};

use lilium_sys::uuid::parse_uuid;

use crate::{eprintln, helpers::exit_unrecoverably};

struct PrintLoc<'a>(Option<&'a Location<'a>>);

impl<'a> core::fmt::Display for PrintLoc<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if let Some(loc) = self.0 {
            f.write_fmt(format_args!(
                "{} {}:{}",
                loc.file(),
                loc.line(),
                loc.column()
            ))
        } else {
            f.write_str("unknown location")
        }
    }
}

#[panic_handler]
fn at_panic(info: &PanicInfo) -> ! {
    eprintln!(
        "Program Panicked at: [{}] {}",
        PrintLoc(info.location()),
        info.message()
    );
    exit_unrecoverably(Some(
        const { parse_uuid("05e3080f-ded6-54a7-acfd-afec3d7e93cb") },
    ))
}
