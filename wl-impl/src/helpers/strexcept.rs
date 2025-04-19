use lilium_sys::uuid::{Uuid, parse_uuid};

pub struct StrExcept(StrExceptInner);

impl core::fmt::Display for StrExcept {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.0 {
            StrExceptInner::Named(n) => f.write_str(n),
            StrExceptInner::Unknown(n) => n.fmt(f),
        }
    }
}

enum StrExceptInner {
    Named(&'static str),
    Unknown(Uuid),
}

pub fn strexcept(uuid: Uuid) -> StrExcept {
    match uuid {
        const { parse_uuid("05e3080f-ded6-54a7-acfd-afec3d7e93cb") } => StrExcept(
            StrExceptInner::Named("Emulation Error (winter-lily Interal Error)"),
        ),
        const { parse_uuid("4c0c6658-59ae-5675-90c3-ffcc0a7219ad") } => {
            StrExcept(StrExceptInner::Named("Uncaught Runtime Exception"))
        }
        const { parse_uuid("466fbae6-be8b-5525-bd04-ee7153b74f55") } => {
            StrExcept(StrExceptInner::Named("Aborted"))
        }
        const { parse_uuid("fcf8d451-89e6-50b5-b2e6-396aec58a74a") } => {
            StrExcept(StrExceptInner::Named("Memory Access Violation"))
        }
        const { parse_uuid("ef1d81bc-58d9-5779-a4c7-540b9163cdf1") } => {
            StrExcept(StrExceptInner::Named("Memory Access Error"))
        }
        const { parse_uuid("f2520097-7a84-54f6-baf6-380242841fe9") } => {
            StrExcept(StrExceptInner::Named("Killed (Kernel)"))
        }
        const { parse_uuid("79a90b8e-8f4b-5134-8aa2-ff68877017db") } => {
            StrExcept(StrExceptInner::Named("Killed (Remote Process)"))
        }
        const { parse_uuid("255f142a-31da-53d6-8667-a69cd7c2ab12") } => {
            StrExcept(StrExceptInner::Named("User Interrupt"))
        }
        const { parse_uuid("df1ddb62-49c5-560f-86ab-1910471570b1") } => {
            StrExcept(StrExceptInner::Named("Breakpoint Trap"))
        }
        const { parse_uuid("9dc46cba-85a4-5b94-be24-03717a40c72b") } => {
            StrExcept(StrExceptInner::Named("Illegal Instruction"))
        }
        const { parse_uuid("5c91c672-f971-5b6b-a806-d6a6d2c8eb8a") } => {
            StrExcept(StrExceptInner::Named("Floating-point Exception"))
        }
        uuid => StrExcept(StrExceptInner::Unknown(uuid)),
    }
}
