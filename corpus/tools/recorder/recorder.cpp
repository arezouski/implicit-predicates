// Recorder of geometric predicate calls: counting, sampling and writing the
// corpus file. See recorder.hpp for the interface and corpus/FORMAT.md for
// the file format.

#include "recorder.hpp"

#include <algorithm>
#include <bit>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <unordered_map>
#include <vector>

namespace ipcorpus::detail {
namespace {

constexpr int kPredicates = 2;
constexpr int kMaxArity = 5;
constexpr int kStages = 5;
constexpr std::uint64_t kDefaultCap = 100000;
constexpr std::uint32_t kNone = UINT32_MAX;

int arity_of(Predicate p) { return p == Predicate::Orient3d ? 4 : 5; }

int parent_count(Kind kind) {
    switch (kind) {
        case Kind::Explicit: return 0;
        case Kind::Lnc: return 2;
        case Kind::Lpi: return 5;
        case Kind::Tpi: return 9;
    }
    return 0;
}

[[noreturn]] void fail(const std::string& message) {
    std::fprintf(stderr, "ipcorpus: %s\n", message.c_str());
    std::abort();
}

// A point of the point table: coordinate bits for an explicit point; parent
// indices (and the bits of t for LNC) for an implicit one.
struct Key {
    Kind kind;
    std::uint64_t words[9];

    bool operator==(const Key& o) const {
        return kind == o.kind && std::memcmp(words, o.words, sizeof words) == 0;
    }
};

struct KeyHash {
    std::size_t operator()(const Key& k) const {
        std::uint64_t h = 0xcbf29ce484222325u ^ static_cast<std::uint64_t>(k.kind);
        for (std::uint64_t w : k.words) h = (h ^ w) * 0x100000001b3u + (h >> 29);
        return static_cast<std::size_t>(h);
    }
};

// SplitMix64, seeded from the model name.
class Random {
public:
    explicit Random(const std::string& seed) {
        state_ = 0xcbf29ce484222325u;
        for (unsigned char c : seed) state_ = (state_ ^ c) * 0x100000001b3u;
    }

    std::uint64_t next() {
        std::uint64_t z = (state_ += 0x9e3779b97f4a7c15u);
        z = (z ^ (z >> 30)) * 0xbf58476d1ce4e5b9u;
        z = (z ^ (z >> 27)) * 0x94d049bb133111ebu;
        return z ^ (z >> 31);
    }

    // Uniform in [0, n), n > 0, without bias: draws below 2^64 mod n are
    // rejected so that every residue is equally likely.
    std::uint64_t below(std::uint64_t n) {
        const std::uint64_t threshold = (0 - n) % n;
        std::uint64_t r = next();
        while (r < threshold) r = next();
        return r % n;
    }

private:
    std::uint64_t state_;
};

struct Sample {
    std::uint64_t sequence;
    Predicate predicate;
    std::int8_t sign;
    Stage stage;
    std::uint32_t points[kMaxArity];
};

struct StratumState {
    std::uint64_t total = 0;
    std::vector<Sample> kept;
};

struct Session {
    std::string out;
    std::string model;
    std::uint64_t cap = kDefaultCap;
    Random random{""};
    std::uint64_t calls = 0;
    std::uint64_t counters[kPredicates][kMaxArity + 1][3][kStages] = {};
    StratumState strata[kPredicates][kMaxArity + 1][2];
    // The call accepted by the last `count`, waiting for `store`.
    StratumState* pending_stratum = nullptr;
    std::size_t pending_slot = 0;
    Sample pending{};
    std::vector<Key> table;
    std::unordered_map<Key, std::uint32_t, KeyHash> index;

    std::uint32_t intern(const Key& key) {
        const auto [it, inserted] = index.try_emplace(key, static_cast<std::uint32_t>(table.size()));
        if (inserted) {
            if (table.size() >= kNone) fail("too many points");
            table.push_back(key);
        }
        return it->second;
    }

    std::uint32_t intern(const Coordinates& c) {
        Key key{Kind::Explicit, {}};
        key.words[0] = std::bit_cast<std::uint64_t>(c.x);
        key.words[1] = std::bit_cast<std::uint64_t>(c.y);
        key.words[2] = std::bit_cast<std::uint64_t>(c.z);
        return intern(key);
    }

    std::uint32_t intern(const PointRecord& p) {
        if (p.kind == Kind::Explicit) return intern(p.own);
        Key key{p.kind, {}};
        const int n = parent_count(p.kind);
        for (int i = 0; i < n; i++) key.words[i] = intern(p.parents[i]);
        if (p.kind == Kind::Lnc) key.words[2] = std::bit_cast<std::uint64_t>(p.t);
        return intern(key);
    }
};

std::uint64_t parse_cap(const char* text) {
    char* end = nullptr;
    const unsigned long long value = std::strtoull(text, &end, 10);
    if (*text < '0' || *text > '9' || *end != '\0') {
        fail(std::string("IPCORPUS_STRATUM_CAP is not a number: ") + text);
    }
    return value;
}

void write_at_exit();

Session* create_session() {
    const char* out = std::getenv("IPCORPUS_OUT");
    if (out == nullptr || *out == '\0') return nullptr;
    auto* s = new Session;  // Never freed: it must outlive static destructors.
    s->out = out;
    const char* model = std::getenv("IPCORPUS_MODEL");
    s->model = model != nullptr ? model : "";
    const char* cap = std::getenv("IPCORPUS_STRATUM_CAP");
    if (cap != nullptr) s->cap = parse_cap(cap);
    s->random = Random(s->model);
    if (std::atexit(write_at_exit) != 0) fail("cannot register the exit handler");
    return s;
}

Session* session() {
    static Session* const instance = create_session();
    return instance;
}

// Little-endian encoder of the file contents.
class Buffer {
public:
    void u8(std::uint8_t v) { bytes_.push_back(v); }
    void u32(std::uint32_t v) { put(v, 4); }
    void u64(std::uint64_t v) { put(v, 8); }
    void f64_bits(std::uint64_t bits) { put(bits, 8); }
    void text(const std::string& s) { bytes_.insert(bytes_.end(), s.begin(), s.end()); }
    const std::vector<std::uint8_t>& bytes() const { return bytes_; }

private:
    void put(std::uint64_t v, int n) {
        for (int i = 0; i < n; i++) bytes_.push_back(static_cast<std::uint8_t>(v >> (8 * i)));
    }
    std::vector<std::uint8_t> bytes_;
};

// The stored calls in the order they happened, and the points they cite
// (parents first), renumbered from 0.
struct Selection {
    std::vector<const Sample*> calls;
    std::vector<std::uint32_t> points;      // indices into the table
    std::vector<std::uint32_t> renumbered;  // table index -> file index
};

Selection select(const Session& s) {
    Selection sel;
    for (const auto& by_predicate : s.strata)
        for (const auto& by_implicit : by_predicate)
            for (const StratumState& stratum : by_implicit)
                for (const Sample& sample : stratum.kept) sel.calls.push_back(&sample);
    std::sort(sel.calls.begin(), sel.calls.end(),
              [](const Sample* a, const Sample* b) { return a->sequence < b->sequence; });
    sel.renumbered.assign(s.table.size(), kNone);
    auto cite = [&](auto& self, std::uint32_t i) -> void {
        if (sel.renumbered[i] != kNone) return;
        const Key& key = s.table[i];
        for (int j = 0; j < parent_count(key.kind); j++) {
            self(self, static_cast<std::uint32_t>(key.words[j]));
        }
        sel.renumbered[i] = static_cast<std::uint32_t>(sel.points.size());
        sel.points.push_back(i);
    };
    for (const Sample* call : sel.calls) {
        for (int j = 0; j < arity_of(call->predicate); j++) cite(cite, call->points[j]);
    }
    return sel;
}

void encode_point(Buffer& b, const Key& key, const Selection& sel) {
    b.u8(static_cast<std::uint8_t>(key.kind));
    if (key.kind == Kind::Explicit) {
        for (int i = 0; i < 3; i++) b.f64_bits(key.words[i]);
        return;
    }
    for (int i = 0; i < parent_count(key.kind); i++) {
        b.u32(sel.renumbered[static_cast<std::uint32_t>(key.words[i])]);
    }
    if (key.kind == Kind::Lnc) b.f64_bits(key.words[2]);
}

Buffer encode(const Session& s, const Selection& sel) {
    std::uint64_t counters = 0, strata = 0;
    for (int p = 0; p < kPredicates; p++)
        for (int i = 0; i <= kMaxArity; i++) {
            for (int z = 0; z < 2; z++) strata += s.strata[p][i][z].total > 0;
            for (const auto& by_sign : s.counters[p][i])
                for (std::uint64_t n : by_sign) counters += n > 0;
        }
    Buffer b;
    b.text("IPCORPUS");
    b.u32(1);
    b.u32(static_cast<std::uint32_t>(s.model.size()));
    b.text(s.model);
    b.u64(sel.points.size());
    b.u64(counters);
    b.u64(strata);
    b.u64(sel.calls.size());
    for (std::uint32_t i : sel.points) encode_point(b, s.table[i], sel);
    for (int p = 0; p < kPredicates; p++)
        for (int i = 0; i <= kMaxArity; i++)
            for (int sign = 0; sign < 3; sign++)
                for (int stage = 0; stage < kStages; stage++) {
                    const std::uint64_t n = s.counters[p][i][sign][stage];
                    if (n == 0) continue;
                    b.u8(static_cast<std::uint8_t>(p));
                    b.u8(static_cast<std::uint8_t>(i));
                    b.u8(static_cast<std::uint8_t>(sign - 1));
                    b.u8(static_cast<std::uint8_t>(stage));
                    b.u64(n);
                }
    for (int p = 0; p < kPredicates; p++)
        for (int i = 0; i <= kMaxArity; i++)
            for (int z = 0; z < 2; z++) {
                const StratumState& stratum = s.strata[p][i][z];
                if (stratum.total == 0) continue;
                b.u8(static_cast<std::uint8_t>(p));
                b.u8(static_cast<std::uint8_t>(i));
                b.u8(static_cast<std::uint8_t>(z));
                b.u64(stratum.total);
                b.u64(stratum.kept.size());
            }
    for (const Sample* call : sel.calls) {
        b.u8(static_cast<std::uint8_t>(call->predicate));
        b.u8(static_cast<std::uint8_t>(call->sign));
        b.u8(static_cast<std::uint8_t>(call->stage));
        for (int j = 0; j < arity_of(call->predicate); j++) b.u32(sel.renumbered[call->points[j]]);
    }
    return b;
}

[[noreturn]] void write_failed(const std::string& path) {
    std::perror(("ipcorpus: cannot write " + path).c_str());
    std::_Exit(EXIT_FAILURE);
}

void write_at_exit() {
    const Session& s = *session();
    const Selection sel = select(s);
    const Buffer b = encode(s, sel);
    const std::string partial = s.out + ".partial";
    std::FILE* f = std::fopen(partial.c_str(), "wb");
    if (f == nullptr) write_failed(partial);
    const bool written = std::fwrite(b.bytes().data(), 1, b.bytes().size(), f) == b.bytes().size();
    if (std::fclose(f) != 0 || !written) write_failed(partial);
    if (std::rename(partial.c_str(), s.out.c_str()) != 0) write_failed(s.out);
    std::fprintf(stderr,
                 "ipcorpus: %llu calls recorded, %zu stored, %zu points; "
                 "%llu cascade entries\n",
                 static_cast<unsigned long long>(s.calls), sel.calls.size(), sel.points.size(),
                 static_cast<unsigned long long>(low_level_entries));
}

}  // namespace

bool enabled() { return session() != nullptr; }

bool count(Predicate predicate, int arity, int implicit, int sign, Stage stage) {
    Session& s = *session();
    if (sign < -1 || sign > 1) {
        fail("call " + std::to_string(s.calls) + " of predicate " +
             std::to_string(static_cast<int>(predicate)) + " returned " + std::to_string(sign) +
             ", which is not a sign");
    }
    if (arity != arity_of(predicate)) fail("wrong number of points");
    const int p = static_cast<int>(predicate);
    s.counters[p][implicit][sign + 1][static_cast<int>(stage)]++;
    StratumState& stratum = s.strata[p][implicit][sign == 0];
    const std::uint64_t seen = ++stratum.total;
    const std::uint64_t sequence = s.calls++;
    std::size_t slot = stratum.kept.size();
    if (slot >= s.cap) {
        const std::uint64_t j = s.random.below(seen);
        if (j >= s.cap) return false;
        slot = static_cast<std::size_t>(j);
    }
    s.pending_stratum = &stratum;
    s.pending_slot = slot;
    s.pending = Sample{sequence, predicate, static_cast<std::int8_t>(sign), stage, {}};
    return true;
}

void store(const PointRecord* points) {
    Session& s = *session();
    Sample sample = s.pending;
    for (int i = 0; i < arity_of(sample.predicate); i++) sample.points[i] = s.intern(points[i]);
    std::vector<Sample>& kept = s.pending_stratum->kept;
    if (s.pending_slot == kept.size()) {
        kept.push_back(sample);
    } else {
        kept[s.pending_slot] = sample;
    }
}

void unsupported_point() { fail("a predicate was called with a point that is not 3D"); }

}  // namespace ipcorpus::detail
