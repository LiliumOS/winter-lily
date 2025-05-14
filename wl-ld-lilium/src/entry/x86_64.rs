use core::arch::naked_asm;

use super::*;

core::arch::global_asm! {
    ".globl _start",
    ".hidden _start",
    ".type _start, STT_FUNC",
    ".size _start, _start._end - _start",
    "_start:",
    "pop r12",
    "xor rbp, rbp",
    "lea r13, [rsp]",
    "lea r14, [rsp+8*r12+8]",
    "mov r15, r14",
    "_start._find_auxv:",
    "mov rax, qword ptr [r15]",
    "lea r15, [r15+8]",
    "test rax, rax",
    "jnz _start._find_auxv",
    "_start._setup_stack:",
    "mov rdi, rsp",
    "add rdi, 0xFFFF",
    "and rdi, 0xFFFFFFFFFFFF0000",
    "add rdi, {STACK_DISPLACEMENT}",
    "mov rsi, {STACK_SIZE}",
    "mov rdx, {MMAP_PROT}",
    "mov r10, {MMAP_FLAGS}",
    "mov r8, -1",
    "mov r9, 0",
    "mov rax, 9",
    "syscall",
    "mov rsp, rax",
    "mov r8, rax", // stack_addr
    "add rsp, {STACK_SIZE}",
    "mov rdi, r12", // argc
    "mov rsi, r13", // argv
    "mov rdx, r14", // envp
    "mov rcx, r15", // auxv
    "call {rust_entry}",
    "mov edi, eax",
    "mov eax, 60",
    "syscall",
    "ud2",
    "_start._end:",
    rust_entry = sym __rust_entry,
    STACK_DISPLACEMENT = const STACK_DISPLACEMENT,
    STACK_SIZE = const STACK_SIZE,
    MMAP_PROT = const const {linux_raw_sys::general::PROT_READ | linux_raw_sys::general::PROT_WRITE },
    MMAP_FLAGS = const const { linux_raw_sys::general::MAP_PRIVATE | linux_raw_sys::general::MAP_ANONYMOUS | linux_raw_sys::general::MAP_GROWSDOWN | linux_raw_sys::general::MAP_STACK }
}

#[unsafe(naked)]
pub unsafe extern "sysv64" fn __call_entry_point(
    argc: usize,            /* rdi */
    argv: *mut *mut c_char, /* rsi */
    envp: *mut *mut c_char, /* rdx */
    numenv: usize,          /* rcx */
    auxv: *mut AuxEnt,      /* r8 */
    numaux: usize,          /* r9 */
    entry: *const c_void,   /* rsp[-8] */
) -> ! {
    unsafe {
        naked_asm! {
            "pop rax",
            "pop r10",
            "push rax",
            "shl rcx, 3",
            "shl r9, 4",
            "push 0", //
            "push 0", // AT_END
            "sub rsp, r9",
            "2:",
            "test r9, r9",
            "je 3f",
            "lea r9, [r9-16]",
            "movups xmm0, xmmword ptr [r8+r9]",
            "movaps xmmword ptr [rsp+r9], xmm0",
            "jmp 2b",
            "3:",
            "mov rbx, rsp",
            "push 0",
            "2:",
            "test rcx, rcx",
            "je 3f",
            "lea rcx, [rcx-8]",
            "push qword ptr [rdx+rcx]",
            "jmp 2b",
            "3:",
            "mov r12, rsp",
            "push 0",
            "2:",
            "test rdi, rdi",
            "je 2f",
            "lea rdi, [rdi-8]",
            "push qword ptr [rsi+rdi*8]",
            "2:",
            "push rdi",
            "mov rax, 1",
            "lea rdx, [{_local_fini}+rip]",
            "jmp r10",
            _local_fini = sym _local_fini
        }
    }
}

extern "C" fn _local_fini() {}
