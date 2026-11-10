// C interface to the current Indirect_Predicates library.

#include "ipref.h"

#include "implicit_point.h"

#include "point_store.hpp"

namespace {
using CurrentStore =
    PointStore<IPs::genericPoint, IPs::explicitPoint3D, IPs::implicitPoint3D_LNC>;
}

IPREF_DEFINE_INTERFACE(current, CurrentStore, NFG::initFPU(), IPs::genericPoint::orient3D,
                       IPs::genericPoint::inSphere)
