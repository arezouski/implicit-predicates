// C interface to the predicate library bundled with the CDT mesher.

#include "ipref.h"

#include "implicit_point.h"

#include "point_store.hpp"

namespace {
using LegacyStore = PointStore<genericPoint, explicitPoint3D, implicitPoint3D_LNC>;
}

// The mesher calls initFPU() once before using the predicates; a handle
// does the same when it is created.
IPREF_DEFINE_INTERFACE(legacy, LegacyStore, initFPU(), genericPoint::orient3D,
                       genericPoint::inSphere)
