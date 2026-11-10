// Recorder of geometric predicate calls, written as a corpus file.
//
// An instrumented program routes each orient3d and insphere call through
// `ipcorpus::record`, and the stage functions of its evaluation cascades call
// `ipcorpus::mark_stage`. The recorder is configured by environment variables
// read at the first call:
//
//   IPCORPUS_OUT          output path; nothing is recorded without it
//   IPCORPUS_MODEL        model name stored in the file and used as the
//                         sampling seed (default: empty)
//   IPCORPUS_STRATUM_CAP  calls stored per stratum (default: 100000)
//
// Counters cover every call. Stored calls are a reservoir sample of each
// stratum (predicate, number of implicit points, zero or non-zero sign),
// seeded by the model name, so a run is deterministic. The file is written
// to IPCORPUS_OUT plus ".partial" when the process exits normally (return
// from main or exit()), then renamed to IPCORPUS_OUT; a killed or aborted run
// leaves no final file.
//
// The recorder keeps global state and is not thread-safe: the recorded
// program must evaluate predicates from a single thread.
//
// The file format is described in corpus/FORMAT.md.

#ifndef IPCORPUS_RECORDER_HPP
#define IPCORPUS_RECORDER_HPP

#include <cstdint>

namespace ipcorpus {

// Codes of the corpus format.
enum class Predicate : std::uint8_t { Orient3d = 0, InSphere = 1 };
enum class Stage : std::uint8_t { Unknown = 0, Filtered = 1, Interval = 2, Bigfloat = 3, Exact = 4 };
enum class Kind : std::uint8_t { Explicit = 0, Lnc = 1, Lpi = 2, Tpi = 3 };

struct Coordinates {
    double x, y, z;
};

// A point of a call, by value. An explicit point has its coordinates in
// `own`; an implicit point has its explicit parents in `parents` (2 for LNC,
// line then plane for LPI, three planes for TPI) and, for LNC, `t`.
struct PointRecord {
    Kind kind;
    Coordinates own;
    Coordinates parents[9];
    double t;
};

namespace detail {

// The stage marked last by the evaluation in progress.
inline Stage stage = Stage::Unknown;
// Nesting depth of recorded predicates.
inline int depth = 0;
// Entries into the cascade entry functions, independent of `record`.
inline std::uint64_t low_level_entries = 0;

// True when recording is enabled (reads the environment once).
bool enabled();

// Counts an outermost call. Returns true when the call must be stored, in
// which case `store` must be called next with its points.
bool count(Predicate predicate, int arity, int implicit, int sign, Stage stage);
void store(const PointRecord* points);
// Stops the program: a call has a point that is not 3D.
[[noreturn]] void unsupported_point();

inline Coordinates coordinates_of(const auto& explicit_point) {
    return {explicit_point.X(), explicit_point.Y(), explicit_point.Z()};
}

template <class Point>
PointRecord describe(const Point& p) {
    PointRecord r{};
    if (p.isExplicit3D()) {
        r.kind = Kind::Explicit;
        r.own = coordinates_of(p.toExplicit3D());
    } else if (p.isLNC()) {
        const auto& lnc = p.toLNC();
        r.kind = Kind::Lnc;
        r.parents[0] = coordinates_of(lnc.P());
        r.parents[1] = coordinates_of(lnc.Q());
        r.t = lnc.T();
    } else if (p.isLPI()) {
        const auto& lpi = p.toLPI();
        r.kind = Kind::Lpi;
        const Coordinates parents[] = {coordinates_of(lpi.P()), coordinates_of(lpi.Q()),
                                       coordinates_of(lpi.R()), coordinates_of(lpi.S()),
                                       coordinates_of(lpi.T())};
        for (int i = 0; i < 5; i++) r.parents[i] = parents[i];
    } else if (p.isTPI()) {
        const auto& tpi = p.toTPI();
        r.kind = Kind::Tpi;
        const Coordinates parents[] = {
            coordinates_of(tpi.V1()), coordinates_of(tpi.V2()), coordinates_of(tpi.V3()),
            coordinates_of(tpi.W1()), coordinates_of(tpi.W2()), coordinates_of(tpi.W3()),
            coordinates_of(tpi.U1()), coordinates_of(tpi.U2()), coordinates_of(tpi.U3())};
        for (int i = 0; i < 9; i++) r.parents[i] = parents[i];
    } else {
        unsupported_point();
    }
    return r;
}

}  // namespace detail

// Called on entry to a stage function: the last stage marked before a
// predicate returns is the one that decided it.
inline void mark_stage(Stage stage) { detail::stage = stage; }

// Called on entry to a cascade entry function, at any nesting depth.
inline void count_low_level() { ++detail::low_level_entries; }

// Evaluates `evaluate(points...)` and records the call with the sign it
// returns. A predicate evaluated inside another recorded one is not
// recorded, and does not change the stage of the outer call.
template <class Evaluate, class... Points>
int record(Predicate predicate, Evaluate evaluate, const Points&... points) {
    if (!detail::enabled()) return evaluate(points...);
    const Stage outer = detail::stage;
    detail::stage = Stage::Unknown;
    ++detail::depth;
    const int sign = evaluate(points...);
    --detail::depth;
    const Stage decided = detail::stage;
    if (detail::depth > 0) {
        detail::stage = outer;
        return sign;
    }
    const int implicit = (0 + ... + (points.isExplicit3D() ? 0 : 1));
    if (detail::count(predicate, sizeof...(Points), implicit, sign, decided)) {
        const PointRecord records[] = {detail::describe(points)...};
        detail::store(records);
    }
    return sign;
}

}  // namespace ipcorpus

#endif  // IPCORPUS_RECORDER_HPP
