// eVTOL Effectors FMU Model - C++ Implementation
//
// Uses the FMI 3.0 version of CPPFMU from pythonfmu3:
// https://github.com/stephensmith25/PythonFMU3
//
// This is a C++ port of evtol_effectors_fmu_model.py

#include "cppfmu/cppfmu_cs.hpp"

#include <array>
#include <cmath>
#include <string>

namespace {

constexpr double kPi = 3.14159265358979323846;
constexpr double kTau = 2.0 * kPi;
constexpr double kDegToRad = kPi / 180.0;

// Effector types
enum class EffectorType {
  kTiltrotor,
  kProprotor,
  kLiftrotor
};

// Quaternion structure
struct Quaternion {
  double w = 1.0;
  double x = 0.0;
  double y = 0.0;
  double z = 0.0;
};

// Position structure
struct Position {
  double x = 0.0;
  double y = 0.0;
  double z = 0.0;
};

// Pose structure (position + orientation)
struct Pose {
  Position position;
  Quaternion orientation;
};

// EffectorState structure
struct EffectorState {
  Pose pose;
};

// Converts Euler angles to Quaternion matching scipy's from_euler("zyx", ...).
// In scipy's convention: "zyx" with [a, b, c] means rotate by 'a' around Z,
// then 'b' around Y, then 'c' around X.
// So: roll->Z rotation, pitch->Y rotation, yaw->X rotation.
// Input angles are in radians.
Quaternion EulerToQuaternion(double z_angle, double y_angle, double x_angle) {
  // ZYX intrinsic rotation: first Z, then Y, then X
  // Q = Qz * Qy * Qx
  double cz = std::cos(z_angle * 0.5);
  double sz = std::sin(z_angle * 0.5);
  double cy = std::cos(y_angle * 0.5);
  double sy = std::sin(y_angle * 0.5);
  double cx = std::cos(x_angle * 0.5);
  double sx = std::sin(x_angle * 0.5);

  Quaternion q;
  q.w = cx * cy * cz + sx * sy * sz;
  q.x = sx * cy * cz - cx * sy * sz;
  q.y = cx * sy * cz + sx * cy * sz;
  q.z = cx * cy * sz - sx * sy * cz;

  return q;
}

// Effector class - handles individual effector calculations
class Effector {
 public:
  Effector(const std::string& name, EffectorType type, double direction,
           double offset_deg)
      : name_(name),
        type_(type),
        offset_deg_(offset_deg),
        roll_rad_(0.0),
        pitch_rad_(0.0),
        yaw_rad_(0.0),
        direction_and_deg2rad_(direction * kDegToRad) {
    // Position of an effector on an eVTOL does not change
    state_.pose.position.x = 0.0;
    state_.pose.position.y = 0.0;
    state_.pose.position.z = 0.0;
  }

  void DoStep(double dt_s, double tilt_deg, double proprotor_rpm,
              double liftrotor_rpm) {
    // Convert RPM to degree of rotation in 'dt_s' time duration
    // (RPM / 60.0 * dt_s * 360)
    double rpm_to_deg = dt_s * 6.0;

    // Calculate orientation based on effector type
    switch (type_) {
      case EffectorType::kTiltrotor:
        pitch_rad_ = (tilt_deg + offset_deg_) * direction_and_deg2rad_;
        break;

      case EffectorType::kProprotor:
        yaw_rad_ += (proprotor_rpm * rpm_to_deg) * direction_and_deg2rad_;
        yaw_rad_ = std::fmod(yaw_rad_ + kTau, kTau);  // Wrap to 0-2pi range
        break;

      case EffectorType::kLiftrotor:
        yaw_rad_ += (liftrotor_rpm * rpm_to_deg) * direction_and_deg2rad_;
        yaw_rad_ = std::fmod(yaw_rad_ + kTau, kTau);  // Wrap to 0-2pi range
        break;
    }

    // Convert RPY to Quaternion (angles are in radians)
    state_.pose.orientation = EulerToQuaternion(roll_rad_, pitch_rad_, yaw_rad_);
  }

  const EffectorState& GetState() const { return state_; }

 private:
  std::string name_;
  EffectorType type_;
  double offset_deg_;
  double roll_rad_;
  double pitch_rad_;
  double yaw_rad_;
  double direction_and_deg2rad_;
  EffectorState state_;
};

// Value reference assignments
// Inputs (0-99)
constexpr cppfmu::FMIValueReference kVrTime = 0;
constexpr cppfmu::FMIValueReference kVrTiltDeg = 1;
constexpr cppfmu::FMIValueReference kVrProprotorRpm = 2;
constexpr cppfmu::FMIValueReference kVrLiftrotorRpm = 3;

// Outputs: Each effector has 7 values (pos x,y,z and orientation w,x,y,z)
// Tiltrotors 1-6: VR 100-141 (6 * 7 = 42 values)
// Rotors 1-6: VR 200-241 (6 * 7 = 42 values)
constexpr cppfmu::FMIValueReference kVrTiltrotorBase = 100;
constexpr cppfmu::FMIValueReference kVrRotorBase = 200;

constexpr int kNumEffectors = 6;
constexpr int kComponentsPerEffector = 7;
constexpr int kTotalEffectorComponents = kNumEffectors * kComponentsPerEffector;

class EvtolEffectorsFmu : public cppfmu::SlaveInstance {
 public:
  EvtolEffectorsFmu()
      : time_(0.0),
        tilt_deg_(0.0),
        proprotor_rpm_(0.0),
        liftrotor_rpm_(0.0) {
    // Create effector objects

    // Outer right tiltrotor
    tiltrotors_[0] = std::make_unique<Effector>(
        "tiltrotor_1", EffectorType::kTiltrotor, -1.0, 0.0);
    rotors_[0] = std::make_unique<Effector>(
        "rotor_1", EffectorType::kProprotor, 1.0, 0.0);

    // Outer left tiltrotor
    tiltrotors_[1] = std::make_unique<Effector>(
        "tiltrotor_2", EffectorType::kTiltrotor, -1.0, 0.0);
    rotors_[1] = std::make_unique<Effector>(
        "rotor_2", EffectorType::kProprotor, -1.0, 0.0);

    // Inner front left tiltrotor
    tiltrotors_[2] = std::make_unique<Effector>(
        "tiltrotor_3", EffectorType::kTiltrotor, -1.0, 0.0);
    rotors_[2] = std::make_unique<Effector>(
        "rotor_3", EffectorType::kProprotor, -1.0, 0.0);

    // Inner rear right tiltrotor
    tiltrotors_[3] = std::make_unique<Effector>(
        "tiltrotor_4", EffectorType::kTiltrotor, -1.0, 0.0);
    rotors_[3] = std::make_unique<Effector>(
        "rotor_4", EffectorType::kProprotor, -1.0, 0.0);

    // Inner front right tiltrotor
    tiltrotors_[4] = std::make_unique<Effector>(
        "tiltrotor_5", EffectorType::kTiltrotor, -1.0, 0.0);
    rotors_[4] = std::make_unique<Effector>(
        "rotor_5", EffectorType::kProprotor, 1.0, 0.0);

    // Inner rear left tiltrotor
    tiltrotors_[5] = std::make_unique<Effector>(
        "tiltrotor_6", EffectorType::kTiltrotor, -1.0, 0.0);
    rotors_[5] = std::make_unique<Effector>(
        "rotor_6", EffectorType::kProprotor, 1.0, 0.0);
  }

  void SetFloat64(const cppfmu::FMIValueReference vr[], std::size_t nvr,
                  const cppfmu::FMIFloat64 value[],
                  std::size_t nValues) override {
    for (std::size_t i = 0; i < nvr; ++i) {
      switch (vr[i]) {
        case kVrTime:
          time_ = value[i];
          break;
        case kVrTiltDeg:
          tilt_deg_ = value[i];
          break;
        case kVrProprotorRpm:
          proprotor_rpm_ = value[i];
          break;
        case kVrLiftrotorRpm:
          liftrotor_rpm_ = value[i];
          break;
        default:
          break;
      }
    }
  }

  void GetFloat64(const cppfmu::FMIValueReference vr[], std::size_t nvr,
                  cppfmu::FMIFloat64 value[],
                  std::size_t nValues) const override {
    for (std::size_t i = 0; i < nvr; ++i) {
      cppfmu::FMIValueReference ref = vr[i];

      if (ref == kVrTime) {
        value[i] = time_;
      } else if (ref == kVrTiltDeg) {
        value[i] = tilt_deg_;
      } else if (ref == kVrProprotorRpm) {
        value[i] = proprotor_rpm_;
      } else if (ref == kVrLiftrotorRpm) {
        value[i] = liftrotor_rpm_;
      } else if (ref >= kVrTiltrotorBase &&
                 ref < kVrTiltrotorBase + kTotalEffectorComponents) {
        // Tiltrotor outputs (VR 100-141)
        int offset = ref - kVrTiltrotorBase;
        int effector_idx = offset / kComponentsPerEffector;
        int component_idx = offset % kComponentsPerEffector;
        value[i] = GetEffectorComponent(tiltrotors_[effector_idx]->GetState(),
                                        component_idx);
      } else if (ref >= kVrRotorBase &&
                 ref < kVrRotorBase + kTotalEffectorComponents) {
        // Rotor outputs (VR 200-241)
        int offset = ref - kVrRotorBase;
        int effector_idx = offset / kComponentsPerEffector;
        int component_idx = offset % kComponentsPerEffector;
        value[i] = GetEffectorComponent(rotors_[effector_idx]->GetState(),
                                        component_idx);
      } else {
        value[i] = 0.0;
      }
    }
  }

  cppfmu::FMIStatus DoStep(cppfmu::FMIFloat64 current_communication_point,
                           cppfmu::FMIFloat64 communication_step_size,
                           cppfmu::FMIBoolean new_step,
                           cppfmu::FMIBoolean* event_handling_needed,
                           cppfmu::FMIBoolean* terminate_simulation,
                           cppfmu::FMIBoolean* early_return,
                           cppfmu::FMIFloat64& end_of_step) override {
    double dt_s = (current_communication_point + communication_step_size) - time_;
    time_ = current_communication_point + communication_step_size;

    for (int i = 0; i < kNumEffectors; ++i) {
      tiltrotors_[i]->DoStep(dt_s, tilt_deg_, proprotor_rpm_, liftrotor_rpm_);
      rotors_[i]->DoStep(dt_s, tilt_deg_, proprotor_rpm_, liftrotor_rpm_);
    }

    *event_handling_needed = cppfmu::FMIFalse;
    *terminate_simulation = cppfmu::FMIFalse;
    *early_return = cppfmu::FMIFalse;
    end_of_step = current_communication_point + communication_step_size;

    return cppfmu::FMIOK;
  }

  // Model Exchange methods - not used for Co-Simulation but must be implemented
  void SetTime(cppfmu::FMIFloat64 time) override { time_ = time; }

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
  static double GetEffectorComponent(const EffectorState& state,
                                     int component_idx) {
    switch (component_idx) {
      case 0: return state.pose.position.x;
      case 1: return state.pose.position.y;
      case 2: return state.pose.position.z;
      case 3: return state.pose.orientation.w;
      case 4: return state.pose.orientation.x;
      case 5: return state.pose.orientation.y;
      case 6: return state.pose.orientation.z;
      default: return 0.0;
    }
  }

  double time_;
  double tilt_deg_;
  double proprotor_rpm_;
  double liftrotor_rpm_;

  std::array<std::unique_ptr<Effector>, kNumEffectors> tiltrotors_;
  std::array<std::unique_ptr<Effector>, kNumEffectors> rotors_;
};

}  // namespace

// Factory function required by CPPFMU.
// Note: The FMI 3.0 version of CPPFMU uses std::unique_ptr directly, unlike the
// FMI 2.0 version which had cppfmu::AllocateUnique() with a custom Memory
// allocator. The comment in cppfmu_cs.hpp mentioning AllocateUnique is outdated.
std::unique_ptr<cppfmu::SlaveInstance> CppfmuInstantiateSlave(
    cppfmu::FMIString instance_name,
    cppfmu::FMIString fmu_guid,
    cppfmu::FMIString fmu_resource_location,
    cppfmu::FMIString mime_type,
    cppfmu::FMIFloat64 timeout,
    cppfmu::FMIBoolean visible,
    cppfmu::FMIBoolean interactive,
    const cppfmu::Logger& logger) {
  return std::make_unique<EvtolEffectorsFmu>();
}
