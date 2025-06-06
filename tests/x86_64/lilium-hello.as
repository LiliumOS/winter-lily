.intel_syntax noprefix
.globl _start
.type _start, function
.size _start, _start._end-_start
_start:
    pop rdi
    lea rdi, [rdi+1]
    mov rsi, rbx # 
    _start._find_init_hdls:
    mov eax, dword ptr [rsi]
    test eax, eax
    je _start._auxv_end_err
    cmp eax, 64 # AT_LILIUM_INIT_HANDLES
    je _start._init_found
    lea rsi, [rsi+16]
    jmp _start._find_init_hdls
    _start._init_found:
    mov rsi, qword ptr [rsi+8]
    mov rdi, qword ptr [rsi+8] # stdout handle
    mov rax, 0x2001 # IOWrite
    lea rsi, [.hello + rip]
    mov rdx, 13
    syscall
    mov rax, 0x3000 # ExitProcess
    mov rdi, 0
    syscall
    _start._auxv_end_err:
    lea rdi, [.err + rip]
    mov rax, 0x0040 # UnmanagedException
    syscall
    ud2
    _start._end:

.hello:
    .ascii "Hello World!\n"

.align 16

.err:
    # 466fbae6-be8b-5525-bd04-ee7153b74f55
    .quad 0xbd04ee7153b74f55
    .quad 0x466fbae6be8b5525
    .quad 0
    .quad 0
