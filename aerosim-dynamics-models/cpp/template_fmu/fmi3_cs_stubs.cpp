// FMI 3.0 Co-Simulation Stubs - Implementation
//
// Default implementations for FMI 3.0 functions not typically used in basic
// Co-Simulation FMUs. These can be linked into any FMU that doesn't need
// clock handling, state serialization, derivatives, or Model Exchange.

#include "fmi3Functions.h"

extern "C" {

// =============================================================================
// Debug Logging
// =============================================================================

fmi3Status fmi3SetDebugLogging(
    fmi3Instance /*instance*/,
    fmi3Boolean /*logging_on*/,
    size_t /*n_categories*/,
    const fmi3String /*categories*/[]) {
  return fmi3OK;
}

// =============================================================================
// Variable Dependencies
// =============================================================================

fmi3Status fmi3GetNumberOfVariableDependencies(
    fmi3Instance /*instance*/,
    fmi3ValueReference /*value_reference*/,
    size_t* n_dependencies) {
  if (n_dependencies) *n_dependencies = 0;
  return fmi3OK;
}

fmi3Status fmi3GetVariableDependencies(
    fmi3Instance /*instance*/,
    fmi3ValueReference /*dependent*/,
    size_t /*element_indices_of_dependent*/[],
    fmi3ValueReference /*independents*/[],
    size_t /*element_indices_of_independents*/[],
    fmi3DependencyKind /*dependency_kinds*/[],
    size_t /*n_dependencies*/) {
  return fmi3OK;
}

// =============================================================================
// Clock Functions (not used in basic Co-Simulation)
// =============================================================================

fmi3Status fmi3GetClock(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    fmi3Clock /*values*/[]) {
  return fmi3OK;
}

fmi3Status fmi3SetClock(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    const fmi3Clock /*values*/[]) {
  return fmi3OK;
}

fmi3Status fmi3GetIntervalDecimal(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    fmi3Float64 /*intervals*/[],
    fmi3IntervalQualifier /*qualifiers*/[]) {
  return fmi3OK;
}

fmi3Status fmi3GetIntervalFraction(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    fmi3UInt64 /*interval_counters*/[],
    fmi3UInt64 /*resolutions*/[],
    fmi3IntervalQualifier /*qualifiers*/[]) {
  return fmi3OK;
}

fmi3Status fmi3SetIntervalDecimal(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    const fmi3Float64 /*intervals*/[]) {
  return fmi3OK;
}

fmi3Status fmi3SetIntervalFraction(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    const fmi3UInt64 /*interval_counters*/[],
    const fmi3UInt64 /*resolutions*/[]) {
  return fmi3OK;
}

fmi3Status fmi3GetShiftDecimal(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    fmi3Float64 /*shifts*/[]) {
  return fmi3OK;
}

fmi3Status fmi3GetShiftFraction(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    fmi3UInt64 /*shift_counters*/[],
    fmi3UInt64 /*resolutions*/[]) {
  return fmi3OK;
}

fmi3Status fmi3SetShiftDecimal(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    const fmi3Float64 /*shifts*/[]) {
  return fmi3OK;
}

fmi3Status fmi3SetShiftFraction(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    const fmi3UInt64 /*shift_counters*/[],
    const fmi3UInt64 /*resolutions*/[]) {
  return fmi3OK;
}

// =============================================================================
// Discrete States
// =============================================================================

fmi3Status fmi3EvaluateDiscreteStates(fmi3Instance /*instance*/) {
  return fmi3OK;
}

fmi3Status fmi3UpdateDiscreteStates(
    fmi3Instance /*instance*/,
    fmi3Boolean* discrete_states_need_update,
    fmi3Boolean* terminate_simulation,
    fmi3Boolean* nominals_of_continuous_states_changed,
    fmi3Boolean* values_of_continuous_states_changed,
    fmi3Boolean* next_event_time_defined,
    fmi3Float64* next_event_time) {
  if (discrete_states_need_update) *discrete_states_need_update = fmi3False;
  if (terminate_simulation) *terminate_simulation = fmi3False;
  if (nominals_of_continuous_states_changed)
    *nominals_of_continuous_states_changed = fmi3False;
  if (values_of_continuous_states_changed)
    *values_of_continuous_states_changed = fmi3False;
  if (next_event_time_defined) *next_event_time_defined = fmi3False;
  if (next_event_time) *next_event_time = 0.0;
  return fmi3OK;
}

// =============================================================================
// FMU State Serialization (not supported)
// =============================================================================

fmi3Status fmi3GetFMUState(
    fmi3Instance /*instance*/,
    fmi3FMUState* /*fmu_state*/) {
  return fmi3Error;
}

fmi3Status fmi3SetFMUState(
    fmi3Instance /*instance*/,
    fmi3FMUState /*fmu_state*/) {
  return fmi3Error;
}

fmi3Status fmi3FreeFMUState(
    fmi3Instance /*instance*/,
    fmi3FMUState* /*fmu_state*/) {
  return fmi3Error;
}

fmi3Status fmi3SerializedFMUStateSize(
    fmi3Instance /*instance*/,
    fmi3FMUState /*fmu_state*/,
    size_t* /*size*/) {
  return fmi3Error;
}

fmi3Status fmi3SerializeFMUState(
    fmi3Instance /*instance*/,
    fmi3FMUState /*fmu_state*/,
    fmi3Byte /*serialized_state*/[],
    size_t /*size*/) {
  return fmi3Error;
}

fmi3Status fmi3DeserializeFMUState(
    fmi3Instance /*instance*/,
    const fmi3Byte /*serialized_state*/[],
    size_t /*size*/,
    fmi3FMUState* /*fmu_state*/) {
  return fmi3Error;
}

// =============================================================================
// Directional Derivatives (not supported)
// =============================================================================

fmi3Status fmi3GetDirectionalDerivative(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*unknowns*/[],
    size_t /*n_unknowns*/,
    const fmi3ValueReference /*knowns*/[],
    size_t /*n_knowns*/,
    const fmi3Float64 /*seed*/[],
    size_t /*n_seed*/,
    fmi3Float64 /*sensitivity*/[],
    size_t /*n_sensitivity*/) {
  return fmi3Error;
}

fmi3Status fmi3GetAdjointDerivative(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*unknowns*/[],
    size_t /*n_unknowns*/,
    const fmi3ValueReference /*knowns*/[],
    size_t /*n_knowns*/,
    const fmi3Float64 /*seed*/[],
    size_t /*n_seed*/,
    fmi3Float64 /*sensitivity*/[],
    size_t /*n_sensitivity*/) {
  return fmi3Error;
}

fmi3Status fmi3GetOutputDerivatives(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    const fmi3Int32 /*orders*/[],
    fmi3Float64 /*values*/[],
    size_t /*n_values*/) {
  return fmi3Error;
}

// =============================================================================
// Co-Simulation State Transitions
// =============================================================================

fmi3Status fmi3EnterStepMode(fmi3Instance /*instance*/) {
  return fmi3OK;
}

// =============================================================================
// Model Exchange Functions (not supported for Co-Simulation FMUs)
// =============================================================================

fmi3Instance fmi3InstantiateModelExchange(
    fmi3String /*instance_name*/,
    fmi3String /*instantiation_token*/,
    fmi3String /*resource_path*/,
    fmi3Boolean /*visible*/,
    fmi3Boolean /*logging_on*/,
    fmi3InstanceEnvironment /*instance_environment*/,
    fmi3LogMessageCallback /*log_message*/) {
  return nullptr;
}

fmi3Status fmi3EnterConfigurationMode(fmi3Instance /*instance*/) {
  return fmi3OK;
}

fmi3Status fmi3ExitConfigurationMode(fmi3Instance /*instance*/) {
  return fmi3OK;
}

fmi3Status fmi3SetTime(fmi3Instance /*instance*/, fmi3Float64 /*time*/) {
  return fmi3OK;
}

fmi3Status fmi3SetContinuousStates(
    fmi3Instance /*instance*/,
    const fmi3Float64 /*continuous_states*/[],
    size_t /*n_continuous_states*/) {
  return fmi3OK;
}

fmi3Status fmi3GetContinuousStateDerivatives(
    fmi3Instance /*instance*/,
    fmi3Float64 /*derivatives*/[],
    size_t /*n_continuous_states*/) {
  return fmi3OK;
}

fmi3Status fmi3GetEventIndicators(
    fmi3Instance /*instance*/,
    fmi3Float64 /*event_indicators*/[],
    size_t /*n_event_indicators*/) {
  return fmi3OK;
}

fmi3Status fmi3GetContinuousStates(
    fmi3Instance /*instance*/,
    fmi3Float64 /*continuous_states*/[],
    size_t /*n_continuous_states*/) {
  return fmi3OK;
}

fmi3Status fmi3GetNominalsOfContinuousStates(
    fmi3Instance /*instance*/,
    fmi3Float64 /*nominals*/[],
    size_t /*n_continuous_states*/) {
  return fmi3OK;
}

fmi3Status fmi3GetNumberOfEventIndicators(
    fmi3Instance /*instance*/,
    size_t* n_event_indicators) {
  if (n_event_indicators) *n_event_indicators = 0;
  return fmi3OK;
}

fmi3Status fmi3GetNumberOfContinuousStates(
    fmi3Instance /*instance*/,
    size_t* n_continuous_states) {
  if (n_continuous_states) *n_continuous_states = 0;
  return fmi3OK;
}

fmi3Status fmi3EnterContinuousTimeMode(fmi3Instance /*instance*/) {
  return fmi3OK;
}

fmi3Status fmi3EnterEventMode(fmi3Instance /*instance*/) {
  return fmi3OK;
}

fmi3Status fmi3CompletedIntegratorStep(
    fmi3Instance /*instance*/,
    fmi3Boolean /*no_set_fmu_state_prior_to_current_point*/,
    fmi3Boolean* enter_event_mode,
    fmi3Boolean* terminate_simulation) {
  if (enter_event_mode) *enter_event_mode = fmi3False;
  if (terminate_simulation) *terminate_simulation = fmi3False;
  return fmi3OK;
}

// =============================================================================
// Scheduled Execution Functions (not supported)
// =============================================================================

fmi3Instance fmi3InstantiateScheduledExecution(
    fmi3String /*instance_name*/,
    fmi3String /*instantiation_token*/,
    fmi3String /*resource_path*/,
    fmi3Boolean /*visible*/,
    fmi3Boolean /*logging_on*/,
    fmi3InstanceEnvironment /*instance_environment*/,
    fmi3LogMessageCallback /*log_message*/,
    fmi3ClockUpdateCallback /*clock_update*/,
    fmi3LockPreemptionCallback /*lock_preemption*/,
    fmi3UnlockPreemptionCallback /*unlock_preemption*/) {
  return nullptr;
}

fmi3Status fmi3ActivateModelPartition(
    fmi3Instance /*instance*/,
    fmi3ValueReference /*clock_reference*/,
    fmi3Float64 /*activation_time*/) {
  return fmi3Error;
}

// =============================================================================
// Variable Type Getter/Setter Stubs
// =============================================================================
// Default implementations for variable types not used by the FMU.
// These return fmi3Error to indicate the type is not implemented.
// To implement a type: remove it from here and add your implementation
// to your FMU's .cpp file using the same pattern as fmi3GetFloat64.

fmi3Status fmi3GetFloat32(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    fmi3Float32 /*values*/[],
    size_t /*n_values*/) {
  return fmi3Error;  // Float32 type not implemented
}

fmi3Status fmi3SetFloat32(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    const fmi3Float32 /*values*/[],
    size_t /*n_values*/) {
  return fmi3Error;  // Float32 type not implemented
}

fmi3Status fmi3GetInt8(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    fmi3Int8 /*values*/[],
    size_t /*n_values*/) {
  return fmi3Error;  // Int8 type not implemented
}

fmi3Status fmi3SetInt8(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    const fmi3Int8 /*values*/[],
    size_t /*n_values*/) {
  return fmi3Error;  // Int8 type not implemented
}

fmi3Status fmi3GetUInt8(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    fmi3UInt8 /*values*/[],
    size_t /*n_values*/) {
  return fmi3Error;  // UInt8 type not implemented
}

fmi3Status fmi3SetUInt8(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    const fmi3UInt8 /*values*/[],
    size_t /*n_values*/) {
  return fmi3Error;  // UInt8 type not implemented
}

fmi3Status fmi3GetInt16(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    fmi3Int16 /*values*/[],
    size_t /*n_values*/) {
  return fmi3Error;  // Int16 type not implemented
}

fmi3Status fmi3SetInt16(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    const fmi3Int16 /*values*/[],
    size_t /*n_values*/) {
  return fmi3Error;  // Int16 type not implemented
}

fmi3Status fmi3GetUInt16(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    fmi3UInt16 /*values*/[],
    size_t /*n_values*/) {
  return fmi3Error;  // UInt16 type not implemented
}

fmi3Status fmi3SetUInt16(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    const fmi3UInt16 /*values*/[],
    size_t /*n_values*/) {
  return fmi3Error;  // UInt16 type not implemented
}

fmi3Status fmi3GetInt32(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    fmi3Int32 /*values*/[],
    size_t /*n_values*/) {
  return fmi3Error;  // Int32 type not implemented
}

fmi3Status fmi3SetInt32(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    const fmi3Int32 /*values*/[],
    size_t /*n_values*/) {
  return fmi3Error;  // Int32 type not implemented
}

fmi3Status fmi3GetUInt32(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    fmi3UInt32 /*values*/[],
    size_t /*n_values*/) {
  return fmi3Error;  // UInt32 type not implemented
}

fmi3Status fmi3SetUInt32(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    const fmi3UInt32 /*values*/[],
    size_t /*n_values*/) {
  return fmi3Error;  // UInt32 type not implemented
}

fmi3Status fmi3GetInt64(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    fmi3Int64 /*values*/[],
    size_t /*n_values*/) {
  return fmi3Error;  // Int64 type not implemented
}

fmi3Status fmi3SetInt64(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    const fmi3Int64 /*values*/[],
    size_t /*n_values*/) {
  return fmi3Error;  // Int64 type not implemented
}

fmi3Status fmi3GetUInt64(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    fmi3UInt64 /*values*/[],
    size_t /*n_values*/) {
  return fmi3Error;  // UInt64 type not implemented
}

fmi3Status fmi3SetUInt64(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    const fmi3UInt64 /*values*/[],
    size_t /*n_values*/) {
  return fmi3Error;  // UInt64 type not implemented
}

fmi3Status fmi3GetBoolean(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    fmi3Boolean /*values*/[],
    size_t /*n_values*/) {
  return fmi3Error;  // Boolean type not implemented
}

fmi3Status fmi3SetBoolean(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    const fmi3Boolean /*values*/[],
    size_t /*n_values*/) {
  return fmi3Error;  // Boolean type not implemented
}

fmi3Status fmi3GetString(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    fmi3String /*values*/[],
    size_t /*n_values*/) {
  return fmi3Error;  // String type not implemented
}

fmi3Status fmi3SetString(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    const fmi3String /*values*/[],
    size_t /*n_values*/) {
  return fmi3Error;  // String type not implemented
}

fmi3Status fmi3GetBinary(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    size_t /*sizes*/[],
    fmi3Binary /*values*/[],
    size_t /*n_values*/) {
  return fmi3Error;  // Binary type not implemented
}

fmi3Status fmi3SetBinary(
    fmi3Instance /*instance*/,
    const fmi3ValueReference /*value_references*/[],
    size_t /*n_value_references*/,
    const size_t /*sizes*/[],
    const fmi3Binary /*values*/[],
    size_t /*n_values*/) {
  return fmi3Error;  // Binary type not implemented
}

}  // extern "C"
