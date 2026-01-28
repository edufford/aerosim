// Template FMU - A starting point for creating C++ FMUs
//
// Uses the FMI 3.0 C API directly with headers from:
// https://github.com/modelica/fmi-standard
//
// This template demonstrates the minimal structure needed for a C++ FMU.
// The example multiplies an input value by a gain parameter to produce an output.
//
// To create a new FMU from this template:
// 1. Copy this directory and rename it
// 2. Update FMU_MODEL_NAME in CMakeLists.txt
// 3. Update modelIdentifier and modelName in modelDescription.xml
// 4. Add your variables to modelDescription.xml with unique valueReferences
// 5. Update the kVr* constants to match your valueReferences
// 6. Implement your logic in fmi3DoStep()
//
// This file contains the FMU-specific implementation. Generic FMI 3.0 stubs
// for clock handling, state serialization, derivatives, and Model Exchange
// are provided by fmi3_cs_stubs.cpp.

#include "fmi3Functions.h"

#include <string>

// =============================================================================
// Value References
// =============================================================================
// These must match the valueReference attributes in modelDescription.xml.
// Convention: time at 0, inputs 1-99, parameters 100-199, outputs 200-299.

// Independent (time)
constexpr fmi3ValueReference kVrTime = 0;

// Inputs
constexpr fmi3ValueReference kVrInput = 1;

// Parameters (tunable)
constexpr fmi3ValueReference kVrGain = 100;

// Outputs
constexpr fmi3ValueReference kVrOutput = 200;

// =============================================================================
// FMU Instance Data
// =============================================================================

struct FmuInstance {
  std::string instance_name;
  fmi3LogMessageCallback log_message;
  fmi3InstanceEnvironment instance_environment;

  // Model variables
  fmi3Float64 time = 0.0;
  fmi3Float64 input = 0.0;
  fmi3Float64 gain = 1.0;
  fmi3Float64 output = 0.0;

  // Helper to log messages
  void Log(fmi3Status status, const char* category, const char* message) {
    if (log_message) {
      log_message(instance_environment, status, category, message);
    }
  }
};

// =============================================================================
// FMI 3.0 Core Functions
// =============================================================================

extern "C" {

// -----------------------------------------------------------------------------
// Version
// -----------------------------------------------------------------------------

const char* fmi3GetVersion() {
  return "3.0";
}

// -----------------------------------------------------------------------------
// Instance Creation and Destruction
// -----------------------------------------------------------------------------

fmi3Instance fmi3InstantiateCoSimulation(
    fmi3String instance_name,
    fmi3String /*instantiation_token*/,
    fmi3String /*resource_path*/,
    fmi3Boolean /*visible*/,
    fmi3Boolean /*logging_on*/,
    fmi3Boolean /*event_mode_used*/,
    fmi3Boolean /*early_return_allowed*/,
    const fmi3ValueReference /*required_intermediate_variables*/[],
    size_t /*n_required_intermediate_variables*/,
    fmi3InstanceEnvironment instance_environment,
    fmi3LogMessageCallback log_message,
    fmi3IntermediateUpdateCallback /*intermediate_update*/) {
  auto* instance = new FmuInstance();
  instance->instance_name = instance_name ? instance_name : "";
  instance->log_message = log_message;
  instance->instance_environment = instance_environment;
  return instance;
}

void fmi3FreeInstance(fmi3Instance instance) {
  delete static_cast<FmuInstance*>(instance);
}

// -----------------------------------------------------------------------------
// Initialization
// -----------------------------------------------------------------------------

fmi3Status fmi3EnterInitializationMode(
    fmi3Instance instance,
    fmi3Boolean /*tolerance_defined*/,
    fmi3Float64 /*tolerance*/,
    fmi3Float64 start_time,
    fmi3Boolean /*stop_time_defined*/,
    fmi3Float64 /*stop_time*/) {
  if (!instance) return fmi3Error;
  auto* fmu = static_cast<FmuInstance*>(instance);
  fmu->time = start_time;
  return fmi3OK;
}

fmi3Status fmi3ExitInitializationMode(fmi3Instance instance) {
  if (!instance) return fmi3Error;
  auto* fmu = static_cast<FmuInstance*>(instance);

  // Compute initial output from initial input and parameter values
  fmu->output = fmu->input * fmu->gain;

  return fmi3OK;
}

// -----------------------------------------------------------------------------
// Simulation Step
// -----------------------------------------------------------------------------

fmi3Status fmi3DoStep(
    fmi3Instance instance,
    fmi3Float64 current_communication_point,
    fmi3Float64 communication_step_size,
    fmi3Boolean /*no_set_fmu_state_prior_to_current_point*/,
    fmi3Boolean* event_handling_needed,
    fmi3Boolean* terminate_simulation,
    fmi3Boolean* early_return,
    fmi3Float64* last_successful_time) {
  if (!instance) return fmi3Error;

  auto* fmu = static_cast<FmuInstance*>(instance);

  // Update time
  fmu->time = current_communication_point + communication_step_size;

  // ===========================================================================
  // YOUR LOGIC HERE
  // ===========================================================================
  // This example simply multiplies the input by the gain.
  // Replace this with your actual computation.

  fmu->output = fmu->input * fmu->gain;

  // ===========================================================================
  // END OF YOUR LOGIC
  // ===========================================================================

  // Set output flags
  if (event_handling_needed) *event_handling_needed = fmi3False;
  if (terminate_simulation) *terminate_simulation = fmi3False;
  if (early_return) *early_return = fmi3False;
  if (last_successful_time) *last_successful_time = fmu->time;

  return fmi3OK;
}

// -----------------------------------------------------------------------------
// Termination and Reset
// -----------------------------------------------------------------------------

fmi3Status fmi3Terminate(fmi3Instance instance) {
  if (!instance) return fmi3Error;
  return fmi3OK;
}

fmi3Status fmi3Reset(fmi3Instance instance) {
  if (!instance) return fmi3Error;
  auto* fmu = static_cast<FmuInstance*>(instance);
  fmu->time = 0.0;
  fmu->input = 0.0;
  fmu->gain = 1.0;
  fmu->output = 0.0;
  return fmi3OK;
}

// =============================================================================
// Variable Getters and Setters
// =============================================================================
// Implement getters/setters for the variable types used by your FMU.
// The scaffolding below shows the pattern for each type.
// Add cases to the switch statements for your value references.

// -----------------------------------------------------------------------------
// Float64 (implemented for this template)
// -----------------------------------------------------------------------------

fmi3Status fmi3GetFloat64(
    fmi3Instance instance,
    const fmi3ValueReference value_references[],
    size_t n_value_references,
    fmi3Float64 values[],
    size_t n_values) {
  if (!instance) return fmi3Error;
  if (n_values < n_value_references) return fmi3Error;

  auto* fmu = static_cast<FmuInstance*>(instance);

  for (size_t i = 0; i < n_value_references; ++i) {
    switch (value_references[i]) {
      case kVrTime:
        values[i] = fmu->time;
        break;
      case kVrInput:
        values[i] = fmu->input;
        break;
      case kVrGain:
        values[i] = fmu->gain;
        break;
      case kVrOutput:
        values[i] = fmu->output;
        break;
      // Add cases for additional Float64 variables here
      default:
        values[i] = 0.0;
        break;
    }
  }
  return fmi3OK;
}

fmi3Status fmi3SetFloat64(
    fmi3Instance instance,
    const fmi3ValueReference value_references[],
    size_t n_value_references,
    const fmi3Float64 values[],
    size_t n_values) {
  if (!instance) return fmi3Error;
  if (n_values < n_value_references) return fmi3Error;

  auto* fmu = static_cast<FmuInstance*>(instance);

  for (size_t i = 0; i < n_value_references; ++i) {
    switch (value_references[i]) {
      // Note: kVrTime is not settable - time advances via fmi3DoStep
      case kVrInput:
        fmu->input = values[i];
        break;
      case kVrGain:
        fmu->gain = values[i];
        break;
      // Add cases for additional Float64 variables here
      default:
        break;
    }
  }
  return fmi3OK;
}

// -----------------------------------------------------------------------------
// Other Variable Types
// -----------------------------------------------------------------------------
// To implement additional variable types (Int32, Boolean, String, etc.):
// 1. Remove the stub from fmi3_cs_stubs.cpp for that type
// 2. Add your implementation here following the Float64 pattern above

}  // extern "C"
