// The boundary between the C interface and the reference libraries. Every
// function of the interface runs its work through `guard`, which:
// - aborts if it is called with a rounding mode other than to nearest, or
//   if the library returns with one (Rust code requires round-to-nearest);
// - aborts if a C++ exception reaches it;
// - marks the calling thread as inside a library call, so that if the
//   library's error routine ends the process with `exit`, which may use
//   status 0, an exit handler turns that into a failure status.

#ifndef IPREF_GUARD_HPP
#define IPREF_GUARD_HPP

#include <cfenv>
#include <cstdio>
#include <cstdlib>
#include <exception>
#include <unistd.h>

namespace {

// Whether this thread is inside a call to the library. `exit` runs the exit
// handlers on the thread that called it, so the handler reads the flag of
// the thread that ended the process.
__attribute__((tls_model("initial-exec"))) thread_local bool inside_library = false;

void on_exit()
{
    if (inside_library) {
        static const char message[] =
            "ipref: the reference library ended the process from inside a call\n";
        (void)!write(STDERR_FILENO, message, sizeof message - 1);
        _exit(EXIT_FAILURE);
    }
}

[[noreturn]] void fail(const char *what)
{
    std::fprintf(stderr, "ipref: %s\n", what);
    std::abort();
}

bool register_exit_handler()
{
    if (std::atexit(on_exit) != 0) {
        fail("could not register the exit handler");
    }
    return true;
}

// Registered once per process, when the library is loaded.
[[maybe_unused]] const bool exit_handler_registered = register_exit_handler();

void require_nearest(const char *what)
{
    if (std::fegetround() != FE_TONEAREST) {
        fail(what);
    }
}

// Clears the flag and checks the rounding mode when a call leaves the
// library, after its result has been computed.
struct Leave {
    ~Leave()
    {
        inside_library = false;
        require_nearest("the reference library returned with the rounding mode not to nearest");
    }
};

// Runs `body` with the checks described at the top of this file.
template <class Body>
auto guard(Body body) noexcept -> decltype(body())
{
    require_nearest("called with the rounding mode not to nearest");
    inside_library = true;
    Leave leave;
    try {
        return body();
    } catch (const std::exception &e) {
        std::fprintf(stderr, "ipref: C++ exception reached the C interface: %s\n", e.what());
    } catch (...) {
        std::fprintf(stderr, "ipref: unknown C++ exception reached the C interface\n");
    }
    std::abort();
}

} // namespace

#endif
