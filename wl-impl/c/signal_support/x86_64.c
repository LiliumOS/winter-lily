#define _POSIX_C_SOURCE 200112L

#include <stddef.h>
#include <signal.h>
#include <stdatomic.h>

#include <setjmp.h>

extern void __memcpy_explicit(void* restrict _dest, const void* restrict _src, size_t _len);

_Thread_local static _Atomic(jmp_buf *) CHECKED_ACCESS_RETBUF;

_Thread_local static _Atomic(struct CheckedAccessError *) CHECKED_ACCESS_ERROR;

struct CheckedAccessError
{
    const void *addr;
    size_t mem_ty;
};

long __checked_memcpy_impl(void *restrict _dest, const void *restrict _src, size_t _len, struct CheckedAccessError *_acc)
{
    jmp_buf _buf;
    if (setjmp(_buf))
    {
        atomic_load_explicit(&CHECKED_ACCESS_ERROR, memory_order_acquire); // We know what store this locks to.
        atomic_store_explicit(&CHECKED_ACCESS_RETBUF, NULL, memory_order_relaxed);
        return -1;
    }
    atomic_store_explicit(&CHECKED_ACCESS_ERROR, _acc, memory_order_relaxed);
    atomic_store_explicit(&CHECKED_ACCESS_RETBUF, NULL, memory_order_release);
    __memcpy_explicit(_dest, _src, _len);
    atomic_store_explicit(&CHECKED_ACCESS_RETBUF, NULL, memory_order_relaxed);
    return 0;
}

extern void __sa_handler_seh_impl(int sig, siginfo_t *inf, void *uctx);

static void sa_handler_impl(int sig, siginfo_t *inf, void *uctx)
{
    if ((sig == SIGSEGV || sig == SIGBUS))
    {
        jmp_buf *_ptr = atomic_load_explicit(&CHECKED_ACCESS_RETBUF, memory_order_acquire);

        if (_ptr)
        {
            struct CheckedAccessError *_acc = atomic_load_explicit(&CHECKED_ACCESS_ERROR, memory_order_relaxed);

            _acc->addr = inf->si_addr;
            atomic_store_explicit(&CHECKED_ACCESS_ERROR, _acc + 1, memory_order_release);
            longjmp(*_ptr, 1); // This is a checked memory access trapping.
        }
    }

    __sa_handler_seh_impl(sig, inf, uctx);
}

void __install_sa_handler()
{
    sigaction_t sa = {
        .sa_sigaction = sa_handler_impl,
        .sa_flags = SA_SIGINFO};
    for (int i = 0; i < 32; i++)
        sigaction(i, &sa, NULL);
}