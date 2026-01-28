// FMI 3.0 Co-Simulation Stubs
//
// Reusable stub implementations for FMI 3.0 functions that are not typically
// needed for basic Co-Simulation FMUs. Include this header and link against
// fmi3_cs_stubs.cpp to provide default implementations.
//
// These stubs handle:
// - Clock and interval functions (return fmi3OK, no-op)
// - FMU state serialization (return fmi3Error, not supported)
// - Directional derivatives (return fmi3Error, not supported)
// - Model Exchange functions (return nullptr/fmi3OK, not supported)
// - Scheduled Execution functions (return fmi3Error, not supported)
//
// Your FMU implementation must provide:
// - fmi3GetVersion()
// - fmi3InstantiateCoSimulation()
// - fmi3FreeInstance()
// - fmi3EnterInitializationMode()
// - fmi3ExitInitializationMode()
// - fmi3DoStep()
// - fmi3Terminate()
// - fmi3Reset()
// - Variable getters/setters for your implemented types

#ifndef FMI3_CS_STUBS_H_
#define FMI3_CS_STUBS_H_

#include "fmi3Functions.h"

// This header declares stubs; implementations are in fmi3_cs_stubs.cpp

#endif  // FMI3_CS_STUBS_H_
