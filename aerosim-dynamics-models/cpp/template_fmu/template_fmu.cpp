// Template FMU - A starting point for creating C++ FMUs
//
// Uses the FMI 3.0 version of CPPFMU from pythonfmu3:
// https://github.com/stephensmith25/PythonFMU3
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
// 6. Implement your logic in DoStep()

#include "cppfmu/cppfmu_cs.hpp"

#include <cmath>

namespace {

// =============================================================================
// Value References
// =============================================================================
// These must match the valueReference attributes in modelDescription.xml.
// Convention: time at 0, inputs 1-99, parameters 100-199, outputs 200-299.

// Independent (time)
constexpr cppfmu::FMIValueReference kVrTime = 0;

// Inputs
constexpr cppfmu::FMIValueReference kVrInput = 1;

// Parameters (tunable)
constexpr cppfmu::FMIValueReference kVrGain = 100;

// Outputs
constexpr cppfmu::FMIValueReference kVrOutput = 200;

// =============================================================================
// FMU Implementation
// =============================================================================

class TemplateFmu : public cppfmu::SlaveInstance {
 public:
  TemplateFmu()
      : time_(0.0),
        input_(0.0),
        gain_(1.0),
        output_(0.0) {}

  // ---------------------------------------------------------------------------
  // Variable Setters - Called by simulation environment to set FMU variables
  // ---------------------------------------------------------------------------

  void SetFloat64(const cppfmu::FMIValueReference vr[], std::size_t nvr,
                  const cppfmu::FMIFloat64 value[],
                  std::size_t /*nValues*/) override {
    for (std::size_t i = 0; i < nvr; ++i) {
      switch (vr[i]) {
        // Note: kVrTime is not settable here - it's set via SetTime()
        case kVrInput:
          input_ = value[i];
          break;
        case kVrGain:
          gain_ = value[i];
          break;
        // Add cases for additional variables here
        default:
          break;
      }
    }
  }

  // ---------------------------------------------------------------------------
  // Variable Getters - Called by simulation environment to read FMU variables
  // ---------------------------------------------------------------------------

  void GetFloat64(const cppfmu::FMIValueReference vr[], std::size_t nvr,
                  cppfmu::FMIFloat64 value[],
                  std::size_t /*nValues*/) const override {
    for (std::size_t i = 0; i < nvr; ++i) {
      switch (vr[i]) {
        case kVrTime:
          value[i] = time_;
          break;
        case kVrInput:
          value[i] = input_;
          break;
        case kVrGain:
          value[i] = gain_;
          break;
        case kVrOutput:
          value[i] = output_;
          break;
        // Add cases for additional variables here
        default:
          value[i] = 0.0;
          break;
      }
    }
  }

  // ---------------------------------------------------------------------------
  // DoStep - Main simulation step function
  // ---------------------------------------------------------------------------
  // This is called each simulation step. Implement your logic here.
  //
  // Parameters:
  //   current_communication_point: Current simulation time
  //   communication_step_size: Time step size
  //   new_step: True if this is a new step (not a rollback)
  //   event_handling_needed: Set to true if events need handling
  //   terminate_simulation: Set to true to request simulation termination
  //   early_return: Set to true if returning before step_size elapsed
  //   end_of_step: Actual end time of the step

  cppfmu::FMIStatus DoStep(cppfmu::FMIFloat64 current_communication_point,
                           cppfmu::FMIFloat64 communication_step_size,
                           cppfmu::FMIBoolean /*new_step*/,
                           cppfmu::FMIBoolean* event_handling_needed,
                           cppfmu::FMIBoolean* terminate_simulation,
                           cppfmu::FMIBoolean* early_return,
                           cppfmu::FMIFloat64& end_of_step) override {
    // Update time (for Co-Simulation, time advances here, not via SetTime)
    time_ = current_communication_point + communication_step_size;

    // =========================================================================
    // YOUR LOGIC HERE
    // =========================================================================
    // This example simply multiplies the input by the gain.
    // Replace this with your actual computation.

    output_ = input_ * gain_;

    // =========================================================================
    // END OF YOUR LOGIC
    // =========================================================================

    // Set output flags (typically leave these as-is)
    *event_handling_needed = cppfmu::FMIFalse;
    *terminate_simulation = cppfmu::FMIFalse;
    *early_return = cppfmu::FMIFalse;
    end_of_step = time_;

    return cppfmu::FMIOK;
  }

  // ---------------------------------------------------------------------------
  // Initialization - Compute initial output values
  // ---------------------------------------------------------------------------
  // ExitInitializationMode is called after all initial values have been set
  // but before the first DoStep(). Use this to compute initial outputs.

  void ExitInitializationMode() override {
    // Compute initial output from initial input and parameter values
    output_ = input_ * gain_;
  }

  // ---------------------------------------------------------------------------
  // Optional: Other initialization methods
  // ---------------------------------------------------------------------------

  // void SetupExperiment(cppfmu::FMIBoolean tolerance_defined,
  //                      cppfmu::FMIFloat64 tolerance,
  //                      cppfmu::FMIFloat64 start_time,
  //                      cppfmu::FMIBoolean stop_time_defined,
  //                      cppfmu::FMIFloat64 stop_time) override {}

  // void EnterInitializationMode() override {}
  // void Terminate() override {}
  // void Reset() override {}

  // ---------------------------------------------------------------------------
  // Model Exchange stubs - Required by CPPFMU interface but not called for
  // Co-Simulation FMUs. For CS, time advances via DoStep(), not SetTime().
  // ---------------------------------------------------------------------------

  void SetTime(cppfmu::FMIFloat64 time) override { time_ = time; }  // ME only
  void GetContinuousStates(cppfmu::FMIFloat64*, std::size_t) const override {}
  void SetContinuousStates(const cppfmu::FMIFloat64*, std::size_t) override {}
  void GetContinuousStateDerivatives(cppfmu::FMIFloat64*,
                                     std::size_t) const override {}
  void GetNominalsOfContinuousStates(cppfmu::FMIFloat64*,
                                     std::size_t) const override {}
  void GetNumberOfContinuousStates(std::size_t& n) const override { n = 0; }
  void GetNumberOfEventIndicators(std::size_t& n) const override { n = 0; }
  void GetEventIndicators(cppfmu::FMIFloat64*, std::size_t) const override {}

  void UpdateDiscreteStates(cppfmu::FMIBoolean* discrete_states_need_update,
                            cppfmu::FMIBoolean* terminate_simulation,
                            cppfmu::FMIBoolean* nominal_continuous_states_changed,
                            cppfmu::FMIBoolean* values_of_continuous_states_changed,
                            cppfmu::FMIBoolean* next_event_time_defined,
                            cppfmu::FMIFloat64*) override {
    *discrete_states_need_update = cppfmu::FMIFalse;
    *terminate_simulation = cppfmu::FMIFalse;
    *nominal_continuous_states_changed = cppfmu::FMIFalse;
    *values_of_continuous_states_changed = cppfmu::FMIFalse;
    *next_event_time_defined = cppfmu::FMIFalse;
  }

  void GetFMUstate(fmi3FMUState&) override {}
  void SetFMUstate(const fmi3FMUState&) override {}
  void FreeFMUstate(fmi3FMUState&) override {}
  size_t SerializedFMUstateSize(const fmi3FMUState&) override { return 0; }
  void SerializeFMUstate(const fmi3FMUState&, fmi3Byte[], size_t) override {}
  void DeSerializeFMUstate(const fmi3Byte[], size_t, fmi3FMUState&) override {}

 private:
  // ---------------------------------------------------------------------------
  // Member Variables
  // ---------------------------------------------------------------------------

  double time_;     // Current simulation time
  double input_;    // Input variable
  double gain_;     // Parameter (tunable)
  double output_;   // Output variable
};

}  // namespace

// =============================================================================
// Factory Function
// =============================================================================
// This function is called by the FMI runtime to create an instance of the FMU.
//
// Note: The FMI 3.0 version of CPPFMU uses std::unique_ptr directly, unlike the
// FMI 2.0 version which had cppfmu::AllocateUnique() with a custom Memory
// allocator. The comment in cppfmu_cs.hpp mentioning AllocateUnique is outdated.

std::unique_ptr<cppfmu::SlaveInstance> CppfmuInstantiateSlave(
    cppfmu::FMIString /*instance_name*/,
    cppfmu::FMIString /*fmu_guid*/,
    cppfmu::FMIString /*fmu_resource_location*/,
    cppfmu::FMIString /*mime_type*/,
    cppfmu::FMIFloat64 /*timeout*/,
    cppfmu::FMIBoolean /*visible*/,
    cppfmu::FMIBoolean /*interactive*/,
    const cppfmu::Logger& /*logger*/) {
  return std::make_unique<TemplateFmu>();
}
