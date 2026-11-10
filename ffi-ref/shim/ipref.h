/*
 * C interface to the reference predicate libraries.
 *
 * Each library version is built into its own shared library and has its
 * own prefix: `ipref_legacy_` and `ipref_current_`. Only plain doubles,
 * integers and opaque handles cross this interface; no C++ exception does.
 *
 * A points handle owns every point added to it and frees them all when it
 * is freed. Points are named by their index, in the order they were added.
 * Every function that takes indices requires them to name existing points;
 * the parents of an LNC point must be explicit points. These preconditions
 * are not checked here: the Rust wrapper checks them.
 *
 * The predicates return 1, 0 or -1 in the sign convention of the library.
 */

#ifndef IPREF_H
#define IPREF_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define IPREF_API __attribute__((visibility("default")))

typedef struct ipref_legacy_points ipref_legacy_points;
typedef struct ipref_current_points ipref_current_points;

IPREF_API ipref_legacy_points *ipref_legacy_points_new(void);
IPREF_API void ipref_legacy_points_free(ipref_legacy_points *points);
IPREF_API uint32_t ipref_legacy_explicit(ipref_legacy_points *points, double x, double y, double z);
IPREF_API uint32_t ipref_legacy_lnc(ipref_legacy_points *points, uint32_t p, uint32_t q, double t);
IPREF_API int ipref_legacy_orient3d(const ipref_legacy_points *points, uint32_t a, uint32_t b,
                                    uint32_t c, uint32_t d);
IPREF_API int ipref_legacy_insphere(const ipref_legacy_points *points, uint32_t a, uint32_t b,
                                    uint32_t c, uint32_t d, uint32_t e);

IPREF_API ipref_current_points *ipref_current_points_new(void);
IPREF_API void ipref_current_points_free(ipref_current_points *points);
IPREF_API uint32_t ipref_current_explicit(ipref_current_points *points, double x, double y,
                                          double z);
IPREF_API uint32_t ipref_current_lnc(ipref_current_points *points, uint32_t p, uint32_t q,
                                     double t);
IPREF_API int ipref_current_orient3d(const ipref_current_points *points, uint32_t a, uint32_t b,
                                     uint32_t c, uint32_t d);
IPREF_API int ipref_current_insphere(const ipref_current_points *points, uint32_t a, uint32_t b,
                                     uint32_t c, uint32_t d, uint32_t e);

#ifdef __cplusplus
}
#endif

#endif
