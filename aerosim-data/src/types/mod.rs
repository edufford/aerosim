pub mod actor;
pub mod adsb;
pub mod controller;
pub mod downlink_format;
pub mod effector;
pub mod flight_deck;
pub mod geometry;
pub mod header;
pub mod json;
pub mod sensor;
pub mod timestamp;
pub mod trajectory;
pub mod vehicle;

// Re-export types so they can be used as aerosim_data::types::Type
pub use actor::{ActorModel, ActorState, ActorType, PhysicalProperties};
pub use controller::{
    AircraftEffectorCommand, AutopilotCommand, AutopilotFlightPlanCommand, FlightControlCommand,
};
pub use effector::EffectorState;
pub use flight_deck::PrimaryFlightDisplayData;
pub use geometry::{Pose, Quaternion, Vector3};
pub use header::Header;
pub use json::JsonData;
pub use sensor::CameraInfo;
pub use sensor::CompressedImage;
pub use sensor::Image;
pub use sensor::ImageEncoding;
pub use sensor::ImageFormat;
pub use sensor::SensorType;
use sensor::ADSB;
use sensor::GNSS;
use sensor::IMU;
pub use timestamp::TimeStamp;
pub use trajectory::TrajectoryVisualization;
pub use vehicle::VehicleState;
pub use vehicle::VehicleType;

use crate::middleware::{AerosimDeserializeEnum, Serializer, SerializerEnum};

#[macro_use]
mod registry;
pub use registry::TypeRegistry;

mod typesupport;
#[cfg(feature = "python")]
pub use typesupport::PyTypeSupport;
pub use typesupport::TypeSupport;

use serde::{Deserialize, Serialize};

pub trait AerosimMessage: Serialize + for<'de> Deserialize<'de> + Clone + Send + Sync {
    fn get_type_name() -> String;
}

#[derive(AerosimDeserializeEnum, Debug)]
pub enum AerosimMessageEnum {
    TimeStamp(TimeStamp),
    Vector3(Vector3),
    JsonData(JsonData),
    VehicleState(VehicleState),
    EffectorState(EffectorState),
    AutopilotCommand(AutopilotCommand),
    FlightControlCommand(FlightControlCommand),
    AircraftEffectorCommand(AircraftEffectorCommand),
    PrimaryFlightDisplayData(PrimaryFlightDisplayData),
    TrajectoryVisualization(TrajectoryVisualization),
    GNSS(GNSS),
    ADSB(ADSB),
    IMU(IMU),
}

pub fn deserialize_to_json(
    type_name: &str,
    serializer: &SerializerEnum,
    data: &[u8],
) -> Option<serde_json::Value> {
    match type_name {
        "aerosim::types::JsonData" => {
            if let Some((_metadata, json_data)) = serializer.deserialize_message::<JsonData>(data) {
                json_data.get_data()
            } else {
                None
            }
        }
        "aerosim::types::TimeStamp" => serializer.to_json::<TimeStamp>(data),
        "aerosim::types::Vector3" => serializer.to_json::<Vector3>(data),
        "aerosim::types::VehicleState" => serializer.to_json::<VehicleState>(data),
        "aerosim::types::EffectorState" => serializer.to_json::<EffectorState>(data),
        "aerosim::types::AutopilotCommand" => serializer.to_json::<AutopilotCommand>(data),
        "aerosim::types::FlightControlCommand" => serializer.to_json::<FlightControlCommand>(data),
        "aerosim::types::AircraftEffectorCommand" => {
            serializer.to_json::<AircraftEffectorCommand>(data)
        }
        "aerosim::types::PrimaryFlightDisplayData" => {
            serializer.to_json::<PrimaryFlightDisplayData>(data)
        }
        "aerosim::types::TrajectoryVisualization" => {
            serializer.to_json::<TrajectoryVisualization>(data)
        }
        "aerosim::types::GNSS" => serializer.to_json::<GNSS>(data),
        "aerosim::types::ADSB" => serializer.to_json::<ADSB>(data),
        "aerosim::types::IMU" => serializer.to_json::<IMU>(data),
        _ => None,
    }
}

register_types!(
    TimeStamp
    Vector3
    JsonData
    VehicleState
    EffectorState
    AutopilotCommand
    FlightControlCommand
    AircraftEffectorCommand
    PrimaryFlightDisplayData
    TrajectoryVisualization
    GNSS
    ADSB
    IMU
);
