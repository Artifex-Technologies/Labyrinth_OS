/* fastclock: for the parity cases of programs that wait (labyrinth-first-components waits 15 s, then up to 120 s): loaded with
 * LD_PRELOAD, a sleep returns at once and moves this process's clocks on by as long as it would have slept, so the script and
 * the program wait the same "time", in milliseconds, and ask the same number of times. Built by compare.py with cc. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <time.h>

static long long offset_ns;

static int (*real_gettime)(clockid_t, struct timespec *);

static int moved(clockid_t c) {
    return c == CLOCK_REALTIME || c == CLOCK_MONOTONIC || c == CLOCK_BOOTTIME || c == CLOCK_MONOTONIC_RAW ||
           c == CLOCK_REALTIME_COARSE || c == CLOCK_MONOTONIC_COARSE;
}

int clock_gettime(clockid_t c, struct timespec *ts) {
    if (!real_gettime)
        real_gettime = (int (*)(clockid_t, struct timespec *))dlsym(RTLD_NEXT, "clock_gettime");
    int r = real_gettime(c, ts);
    if (r == 0 && moved(c)) {
        long long t = (long long)ts->tv_sec * 1000000000LL + ts->tv_nsec + offset_ns;
        ts->tv_sec = t / 1000000000LL;
        ts->tv_nsec = t % 1000000000LL;
    }
    return r;
}

int nanosleep(const struct timespec *req, struct timespec *rem) {
    offset_ns += (long long)req->tv_sec * 1000000000LL + req->tv_nsec;
    if (rem) {
        rem->tv_sec = 0;
        rem->tv_nsec = 0;
    }
    return 0;
}

int clock_nanosleep(clockid_t c, int flags, const struct timespec *req, struct timespec *rem) {
    if (flags & TIMER_ABSTIME) {
        struct timespec now;
        clock_gettime(c, &now);
        long long d = ((long long)req->tv_sec - now.tv_sec) * 1000000000LL + (req->tv_nsec - now.tv_nsec);
        if (d > 0)
            offset_ns += d;
        return 0;
    }
    return nanosleep(req, rem);
}

time_t time(time_t *t) {
    struct timespec ts;
    clock_gettime(CLOCK_REALTIME, &ts);
    if (t)
        *t = ts.tv_sec;
    return ts.tv_sec;
}
