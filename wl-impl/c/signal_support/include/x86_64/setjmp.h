#pragma once

typedef struct __jmp_buf
{
    _Alignas(64) void *__reg_nv[8];
} jmp_buf[1];

__attribute__((__visibility__("protected"))) int __setjmp(jmp_buf __buf) __attribute__((returns_twice));

_Noreturn __attribute__((__visibility__("protected"))) void longjmp(jmp_buf __buf, int status);

#define setjmp(buf) __setjmp(buf)