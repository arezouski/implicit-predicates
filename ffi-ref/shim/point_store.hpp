// Point storage and C interface definitions shared by the two library
// versions. Each version includes its own library headers first and then
// expands IPREF_DEFINE_INTERFACE with its types.

#ifndef IPREF_POINT_STORE_HPP
#define IPREF_POINT_STORE_HPP

#include "guard.hpp"

#include <cfenv>
#include <cstdint>
#include <deque>
#include <vector>

namespace {

// Owns the points of one handle.
//
// LNC points hold references to their parents, so a point must never move
// once created and must outlive the points built on it. Both hold here:
// std::deque::emplace_back never moves the elements already stored, the
// store itself lives behind a handle that is never moved, and every point
// is destroyed only when the whole store is.
template <class Generic, class Explicit, class Lnc>
class PointStore {
public:
    uint32_t add_explicit(double x, double y, double z)
    {
        explicit_points.emplace_back(x, y, z);
        return add(explicit_points.back());
    }

    uint32_t add_lnc(uint32_t p, uint32_t q, double t)
    {
        lnc_points.emplace_back(as_explicit(p), as_explicit(q), t);
        return add(lnc_points.back());
    }

    const Generic &at(uint32_t index) const { return *all[index]; }

private:
    uint32_t add(const Generic &point)
    {
        all.push_back(&point);
        return static_cast<uint32_t>(all.size() - 1);
    }

    const Explicit &as_explicit(uint32_t index) const
    {
        return static_cast<const Explicit &>(*all[index]);
    }

    std::deque<Explicit> explicit_points;
    std::deque<Lnc> lnc_points;
    std::vector<const Generic *> all;
};

} // namespace

// Defines the C interface of one version. `VERSION` is the prefix word,
// `STORE` the PointStore type, `INIT` a statement run when a handle is
// created, `ORIENT3D`, `INSPHERE` the library's predicates on generic
// points and `ERROR_ROUTINE` the library's error routine (used only by a
// test hook). Besides the six functions of the bindings it defines the
// benchmark hook and the three test hooks of `ipref.h`.
#define IPREF_DEFINE_INTERFACE(VERSION, STORE, INIT, ORIENT3D, INSPHERE, ERROR_ROUTINE)            \
    struct ipref_##VERSION##_points {                                                              \
        STORE store;                                                                               \
    };                                                                                             \
    extern "C" ipref_##VERSION##_points *ipref_##VERSION##_points_new(void)                        \
    {                                                                                              \
        return guard([] {                                                                          \
            INIT;                                                                                  \
            return new ipref_##VERSION##_points();                                                 \
        });                                                                                        \
    }                                                                                              \
    extern "C" void ipref_##VERSION##_points_free(ipref_##VERSION##_points *points)                \
    {                                                                                              \
        guard([&] { delete points; });                                                             \
    }                                                                                              \
    extern "C" uint32_t ipref_##VERSION##_explicit(ipref_##VERSION##_points *points, double x,     \
                                                   double y, double z)                             \
    {                                                                                              \
        return guard([&] { return points->store.add_explicit(x, y, z); });                         \
    }                                                                                              \
    extern "C" uint32_t ipref_##VERSION##_lnc(ipref_##VERSION##_points *points, uint32_t p,        \
                                              uint32_t q, double t)                                \
    {                                                                                              \
        return guard([&] { return points->store.add_lnc(p, q, t); });                              \
    }                                                                                              \
    extern "C" int ipref_##VERSION##_orient3d(const ipref_##VERSION##_points *points, uint32_t a,  \
                                              uint32_t b, uint32_t c, uint32_t d)                  \
    {                                                                                              \
        return guard([&] {                                                                         \
            const STORE &s = points->store;                                                        \
            return ORIENT3D(s.at(a), s.at(b), s.at(c), s.at(d));                                   \
        });                                                                                        \
    }                                                                                              \
    extern "C" int ipref_##VERSION##_insphere(const ipref_##VERSION##_points *points, uint32_t a,  \
                                              uint32_t b, uint32_t c, uint32_t d, uint32_t e)      \
    {                                                                                              \
        return guard([&] {                                                                         \
            const STORE &s = points->store;                                                        \
            return INSPHERE(s.at(a), s.at(b), s.at(c), s.at(d), s.at(e));                          \
        });                                                                                        \
    }                                                                                              \
    extern "C" int ipref_##VERSION##_bench_overhead(const ipref_##VERSION##_points *points,        \
                                                    uint32_t a, uint32_t b, uint32_t c,            \
                                                    uint32_t d)                                    \
    {                                                                                              \
        return guard([&] {                                                                         \
            const STORE &s = points->store;                                                        \
            /* Keep the four lookups and the guard's stores, which the */                          \
            /* compiler could otherwise drop. */                                                   \
            asm volatile(""                                                                        \
                         :                                                                         \
                         : "r"(&s.at(a)), "r"(&s.at(b)), "r"(&s.at(c)), "r"(&s.at(d))              \
                         : "memory");                                                              \
            return 0;                                                                              \
        });                                                                                        \
    }                                                                                              \
    extern "C" void ipref_##VERSION##_test_error_routine(void)                                     \
    {                                                                                              \
        guard([] { ERROR_ROUTINE("ipref test hook: error routine\n"); });                          \
    }                                                                                              \
    extern "C" void ipref_##VERSION##_test_return_rounding_upward(void)                            \
    {                                                                                              \
        guard([] { std::fesetround(FE_UPWARD); });                                                 \
    }                                                                                              \
    extern "C" void ipref_##VERSION##_test_enter_rounding_upward(void)                             \
    {                                                                                              \
        std::fesetround(FE_UPWARD);                                                                \
        ipref_##VERSION##_points_free(ipref_##VERSION##_points_new());                             \
        std::fesetround(FE_TONEAREST);                                                             \
    }

#endif
