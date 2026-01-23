/* Copyright 2024, AeroSim.
 * eVTOL Effectors FMU Model - C++ Implementation using CPPFMU
 *
 * This is a C++ port of evtol_effectors_fmu_model.py
 */

#include "cppfmu/cppfmu_cs.hpp"

#include <cmath>
#include <array>
#include <string>
#include <map>

namespace
{

constexpr double PI = 3.14159265358979323846;
constexpr double TAU = 2.0 * PI;
constexpr double DEG_TO_RAD = PI / 180.0;

// Effector types
enum class EffectorType
{
    Tiltrotor,
    Proprotor,
    Liftrotor
};

// Quaternion structure
struct Quaternion
{
    double w = 1.0;
    double x = 0.0;
    double y = 0.0;
    double z = 0.0;
};

// Position structure
struct Position
{
    double x = 0.0;
    double y = 0.0;
    double z = 0.0;
};

// Pose structure (position + orientation)
struct Pose
{
    Position position;
    Quaternion orientation;
};

// EffectorState structure
struct EffectorState
{
    Pose pose;
};

// Convert Euler angles to Quaternion matching scipy's from_euler("zyx", [roll, pitch, yaw])
// In scipy's convention: "zyx" with [a, b, c] means rotate by 'a' around Z, then 'b' around Y, then 'c' around X
// So: roll->Z rotation, pitch->Y rotation, yaw->X rotation
// Input angles are in radians
Quaternion eulerToQuaternion(double zAngle, double yAngle, double xAngle)
{
    // ZYX intrinsic rotation: first Z, then Y, then X
    // Q = Qz * Qy * Qx
    double cz = std::cos(zAngle * 0.5);
    double sz = std::sin(zAngle * 0.5);
    double cy = std::cos(yAngle * 0.5);
    double sy = std::sin(yAngle * 0.5);
    double cx = std::cos(xAngle * 0.5);
    double sx = std::sin(xAngle * 0.5);

    Quaternion q;
    q.w = cx * cy * cz + sx * sy * sz;
    q.x = sx * cy * cz - cx * sy * sz;
    q.y = cx * sy * cz + sx * cy * sz;
    q.z = cx * cy * sz - sx * sy * cz;

    return q;
}

// Effector class - handles individual effector calculations
class Effector
{
public:
    Effector(const std::string& name, EffectorType type, double direction, double offsetDeg)
        : name_(name)
        , type_(type)
        , offsetDeg_(offsetDeg)
        , rollDeg_(0.0)
        , pitchDeg_(0.0)
        , yawDeg_(0.0)
        , directionAndD2R_(direction * DEG_TO_RAD)
    {
        // Position of an effector on an eVTOL does not change
        state_.pose.position.x = 0.0;
        state_.pose.position.y = 0.0;
        state_.pose.position.z = 0.0;
    }

    void doStep(double dt_s, double tiltDeg, double proprotorRpm, double liftrotorRpm)
    {
        // Convert RPM to degree of rotation in 'dt_s' time duration (RPM/60.0*dt_s*360)
        double rpmToDeg = dt_s * 6.0;

        // Calculate orientation based on effector type
        switch (type_)
        {
        case EffectorType::Tiltrotor:
            pitchDeg_ = (tiltDeg + offsetDeg_) * directionAndD2R_;
            break;

        case EffectorType::Proprotor:
            yawDeg_ += (proprotorRpm * rpmToDeg) * directionAndD2R_;
            yawDeg_ = std::fmod(yawDeg_ + TAU, TAU);  // Convert to 0-2pi range
            break;

        case EffectorType::Liftrotor:
            yawDeg_ += (liftrotorRpm * rpmToDeg) * directionAndD2R_;
            yawDeg_ = std::fmod(yawDeg_ + TAU, TAU);  // Convert to 0-2pi range
            break;
        }

        // Convert RPY to Quaternion (note: angles are in radians after directionAndD2R_ multiplication)
        Quaternion q = eulerToQuaternion(rollDeg_, pitchDeg_, yawDeg_);
        state_.pose.orientation = q;
    }

    const EffectorState& getState() const { return state_; }

private:
    std::string name_;
    EffectorType type_;
    double offsetDeg_;
    double rollDeg_;
    double pitchDeg_;
    double yawDeg_;
    double directionAndD2R_;
    EffectorState state_;
};

// Value reference assignments
// Inputs (0-99)
constexpr cppfmu::FMIValueReference VR_TIME = 0;
constexpr cppfmu::FMIValueReference VR_TILT_DEG = 1;
constexpr cppfmu::FMIValueReference VR_PROPROTOR_RPM = 2;
constexpr cppfmu::FMIValueReference VR_LIFTROTOR_RPM = 3;

// Outputs: Each effector has 7 values (pos x,y,z and orientation w,x,y,z)
// Tiltrotors 1-6: VR 100-141 (6 * 7 = 42 values)
// Rotors 1-6: VR 200-241 (6 * 7 = 42 values)
constexpr cppfmu::FMIValueReference VR_TILTROTOR_BASE = 100;
constexpr cppfmu::FMIValueReference VR_ROTOR_BASE = 200;

// Helper to get value reference for effector output
inline cppfmu::FMIValueReference getEffectorVR(cppfmu::FMIValueReference base, int effectorIndex, int componentIndex)
{
    return base + (effectorIndex * 7) + componentIndex;
}

class EvtolEffectorsFmu : public cppfmu::SlaveInstance
{
public:
    EvtolEffectorsFmu()
        : time_(0.0)
        , tiltDeg_(0.0)
        , proprotorRpm_(0.0)
        , liftrotorRpm_(0.0)
    {
        // Create effector objects

        // Outer right tiltrotor
        tiltrotors_[0] = std::make_unique<Effector>("tiltrotor_1", EffectorType::Tiltrotor, -1.0, 0.0);
        rotors_[0] = std::make_unique<Effector>("rotor_1", EffectorType::Proprotor, 1.0, 0.0);

        // Outer left tiltrotor
        tiltrotors_[1] = std::make_unique<Effector>("tiltrotor_2", EffectorType::Tiltrotor, -1.0, 0.0);
        rotors_[1] = std::make_unique<Effector>("rotor_2", EffectorType::Proprotor, -1.0, 0.0);

        // Inner front left tiltrotor
        tiltrotors_[2] = std::make_unique<Effector>("tiltrotor_3", EffectorType::Tiltrotor, -1.0, 0.0);
        rotors_[2] = std::make_unique<Effector>("rotor_3", EffectorType::Proprotor, -1.0, 0.0);

        // Inner rear right tiltrotor
        tiltrotors_[3] = std::make_unique<Effector>("tiltrotor_4", EffectorType::Tiltrotor, -1.0, 0.0);
        rotors_[3] = std::make_unique<Effector>("rotor_4", EffectorType::Proprotor, -1.0, 0.0);

        // Inner front right tiltrotor
        tiltrotors_[4] = std::make_unique<Effector>("tiltrotor_5", EffectorType::Tiltrotor, -1.0, 0.0);
        rotors_[4] = std::make_unique<Effector>("rotor_5", EffectorType::Proprotor, 1.0, 0.0);

        // Inner rear left tiltrotor
        tiltrotors_[5] = std::make_unique<Effector>("tiltrotor_6", EffectorType::Tiltrotor, -1.0, 0.0);
        rotors_[5] = std::make_unique<Effector>("rotor_6", EffectorType::Proprotor, 1.0, 0.0);
    }

    void SetFloat64(
        const cppfmu::FMIValueReference vr[],
        std::size_t nvr,
        const cppfmu::FMIFloat64 value[],
        std::size_t nValues) override
    {
        for (std::size_t i = 0; i < nvr; ++i)
        {
            switch (vr[i])
            {
            case VR_TIME:
                time_ = value[i];
                break;
            case VR_TILT_DEG:
                tiltDeg_ = value[i];
                break;
            case VR_PROPROTOR_RPM:
                proprotorRpm_ = value[i];
                break;
            case VR_LIFTROTOR_RPM:
                liftrotorRpm_ = value[i];
                break;
            default:
                // Unknown value reference - ignore or log warning
                break;
            }
        }
    }

    void GetFloat64(
        const cppfmu::FMIValueReference vr[],
        std::size_t nvr,
        cppfmu::FMIFloat64 value[],
        std::size_t nValues) const override
    {
        for (std::size_t i = 0; i < nvr; ++i)
        {
            cppfmu::FMIValueReference ref = vr[i];

            // Check for input variables
            if (ref == VR_TIME)
            {
                value[i] = time_;
            }
            else if (ref == VR_TILT_DEG)
            {
                value[i] = tiltDeg_;
            }
            else if (ref == VR_PROPROTOR_RPM)
            {
                value[i] = proprotorRpm_;
            }
            else if (ref == VR_LIFTROTOR_RPM)
            {
                value[i] = liftrotorRpm_;
            }
            // Check for tiltrotor outputs (VR 100-141)
            else if (ref >= VR_TILTROTOR_BASE && ref < VR_TILTROTOR_BASE + 42)
            {
                int offset = ref - VR_TILTROTOR_BASE;
                int effectorIdx = offset / 7;
                int componentIdx = offset % 7;
                value[i] = getEffectorComponent(tiltrotors_[effectorIdx]->getState(), componentIdx);
            }
            // Check for rotor outputs (VR 200-241)
            else if (ref >= VR_ROTOR_BASE && ref < VR_ROTOR_BASE + 42)
            {
                int offset = ref - VR_ROTOR_BASE;
                int effectorIdx = offset / 7;
                int componentIdx = offset % 7;
                value[i] = getEffectorComponent(rotors_[effectorIdx]->getState(), componentIdx);
            }
            else
            {
                value[i] = 0.0;  // Unknown reference
            }
        }
    }

    cppfmu::FMIStatus DoStep(
        cppfmu::FMIFloat64 currentCommunicationPoint,
        cppfmu::FMIFloat64 communicationStepSize,
        cppfmu::FMIBoolean newStep,
        cppfmu::FMIBoolean* eventHandlingNeeded,
        cppfmu::FMIBoolean* terminateSimulation,
        cppfmu::FMIBoolean* earlyReturn,
        cppfmu::FMIFloat64& endOfStep) override
    {
        // Calculate time step
        double dt_s = (currentCommunicationPoint + communicationStepSize) - time_;
        time_ = currentCommunicationPoint + communicationStepSize;

        // Step all effectors
        for (int i = 0; i < 6; ++i)
        {
            tiltrotors_[i]->doStep(dt_s, tiltDeg_, proprotorRpm_, liftrotorRpm_);
            rotors_[i]->doStep(dt_s, tiltDeg_, proprotorRpm_, liftrotorRpm_);
        }

        // Set output flags
        *eventHandlingNeeded = cppfmu::FMIFalse;
        *terminateSimulation = cppfmu::FMIFalse;
        *earlyReturn = cppfmu::FMIFalse;
        endOfStep = currentCommunicationPoint + communicationStepSize;

        return cppfmu::FMIOK;
    }

    // Model Exchange methods - not used for Co-Simulation but must be implemented
    void GetContinuousStates(cppfmu::FMIFloat64* continuousStates, std::size_t nStates) const override {}
    void GetContinuousStateDerivatives(cppfmu::FMIFloat64* derivatives, std::size_t nStates) const override {}
    void GetNominalsOfContinuousStates(cppfmu::FMIFloat64* nominalContinuousStates, std::size_t nStates) const override {}
    void SetContinuousStates(const cppfmu::FMIFloat64* continuousStates, std::size_t nStates) override {}
    void SetTime(cppfmu::FMIFloat64 time) override { time_ = time; }
    void GetNumberOfContinuousStates(std::size_t& nStates) const override { nStates = 0; }
    void GetNumberOfEventIndicators(std::size_t& nEventIndicators) const override { nEventIndicators = 0; }
    void GetEventIndicators(cppfmu::FMIFloat64* eventIndicators, std::size_t nEventIndicators) const override {}
    void UpdateDiscreteStates(
        cppfmu::FMIBoolean* discreteStatesNeedUpdate,
        cppfmu::FMIBoolean* terminateSimulation,
        cppfmu::FMIBoolean* nominalContinuousStatesChanged,
        cppfmu::FMIBoolean* valuesOfContinuousStatesChanged,
        cppfmu::FMIBoolean* nextEventTimeDefined,
        cppfmu::FMIFloat64* nextEventTime) override
    {
        *discreteStatesNeedUpdate = cppfmu::FMIFalse;
        *terminateSimulation = cppfmu::FMIFalse;
        *nominalContinuousStatesChanged = cppfmu::FMIFalse;
        *valuesOfContinuousStatesChanged = cppfmu::FMIFalse;
        *nextEventTimeDefined = cppfmu::FMIFalse;
    }

    void GetFMUstate(fmi3FMUState& state) override {}
    void SetFMUstate(const fmi3FMUState& state) override {}
    void FreeFMUstate(fmi3FMUState& state) override {}
    size_t SerializedFMUstateSize(const fmi3FMUState& state) override { return 0; }
    void SerializeFMUstate(const fmi3FMUState& state, fmi3Byte bytes[], size_t size) override {}
    void DeSerializeFMUstate(const fmi3Byte bytes[], size_t size, fmi3FMUState& state) override {}

private:
    double getEffectorComponent(const EffectorState& state, int componentIdx) const
    {
        switch (componentIdx)
        {
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
    double tiltDeg_;
    double proprotorRpm_;
    double liftrotorRpm_;

    std::array<std::unique_ptr<Effector>, 6> tiltrotors_;
    std::array<std::unique_ptr<Effector>, 6> rotors_;
};

} // anonymous namespace

// Factory function required by CPPFMU
std::unique_ptr<cppfmu::SlaveInstance> CppfmuInstantiateSlave(
    cppfmu::FMIString instanceName,
    cppfmu::FMIString fmuGUID,
    cppfmu::FMIString fmuResourceLocation,
    cppfmu::FMIString mimeType,
    cppfmu::FMIFloat64 timeout,
    cppfmu::FMIBoolean visible,
    cppfmu::FMIBoolean interactive,
    const cppfmu::Logger& logger)
{
    return std::make_unique<EvtolEffectorsFmu>();
}
