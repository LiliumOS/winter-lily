#pragma once
#include <linux/signal.h>

typedef void (*sighandler_t)(int);
typedef void (*sigaction_handler_t)(int, siginfo_t *, void *);

typedef struct
{
    union
    {
        sighandler_t sa_handler;
        sigaction_handler_t sa_sigaction;
    };
    sigset_t sa_mask;
    int sa_flags;
} sigaction_t;

int sigaction(int signo, const sigaction_t *restrict __act, sigaction_t *restrict __old);