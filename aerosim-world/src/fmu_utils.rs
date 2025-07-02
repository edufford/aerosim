use log::warn;
use std::sync::Arc;

use fmi::fmi3::schema::VariableType;

use aerosim_data::middleware::{
    Metadata, Middleware, MiddlewareEnum, MiddlewareRaw, Serializer, SerializerEnum,
};
use aerosim_data::types::{
    JsonData, PrimaryFlightDisplayData, TimeStamp, TypeSupport, VehicleState,
};

pub fn fmi3_var_type_to_string(var_type: &VariableType) -> String {
    return match var_type {
        VariableType::FmiFloat64 => "float64".to_string(),
        VariableType::FmiFloat32 => "float32".to_string(),
        VariableType::FmiInt64 => "int64".to_string(),
        VariableType::FmiInt32 => "int32".to_string(),
        VariableType::FmiInt16 => "int16".to_string(),
        VariableType::FmiInt8 => "int8".to_string(),
        VariableType::FmiUInt64 => "uint64".to_string(),
        VariableType::FmiUInt32 => "uint32".to_string(),
        VariableType::FmiUInt16 => "uint16".to_string(),
        VariableType::FmiUInt8 => "uint8".to_string(),
        VariableType::FmiBoolean => "bool".to_string(),
        VariableType::FmiString => "string".to_string(),
        _ => "unknown".to_string(),
    };
}

pub async fn publish_component_output_topics_fmu3(
    fmu_id: &str,
    fmu_config_json: &serde_json::Value,
    fmu_var_types: &std::collections::HashMap<String, VariableType>,
    fmu_data_f64: &std::collections::HashMap<String, Vec<f64>>,
    fmu_data_i64: &std::collections::HashMap<String, Vec<i64>>,
    timestamp: TimeStamp,
    middleware: &Arc<MiddlewareEnum>,
    serializer: &SerializerEnum,
) {
    // Pack and publish component output topics
    if let Some(output_topics) = fmu_config_json.get("component_output_topics") {
        for out_topic_info in output_topics
            .as_array()
            .expect("Unable to get 'component_output_topics' as array")
        {
            let msg_type = out_topic_info
                .get("msg_type")
                .expect("Unable to get 'msg_type' field from JSON")
                .as_str()
                .expect("Unable to get 'msg_type' as string");
            let out_topic = out_topic_info
                .get("topic")
                .expect("Unable to get 'topic' field from JSON")
                .as_str()
                .expect("Unable to get 'topic' as string");

            let mut var_prefix = "".to_string();
            // Override var_prefix if one is provided
            if let Some(var_prefix_config) = out_topic_info.get("var_prefix") {
                var_prefix = var_prefix_config
                    .as_str()
                    .expect("Unable to parse 'var_prefix' as string")
                    .to_string();
            }

            match msg_type {
                "aerosim::types::VehicleState" => {
                    if var_prefix.is_empty() {
                        var_prefix = "vehicle_state".to_string();
                    }

                    // Pack data from FMU into output message struct
                    let msg_struct = VehicleState::default();

                    let mut msg_struct_json = serde_json::to_value(&msg_struct)
                        .expect("Unable to serialize VehicleState struct to JSON");
                    let flat_fields =
                        TypeSupport::get_flat_fields_from_json_object(&msg_struct_json, "");

                    // let flat_fields = TypeSupport::get_flat_field_names(&msg_struct, "");

                    // Set the fields in the message struct using the flat field names and FMU data.
                    for field_name in &flat_fields {
                        set_fmu_field_in_json_struct(
                            fmu_id,
                            field_name,
                            &var_prefix,
                            fmu_var_types,
                            fmu_data_f64,
                            fmu_data_i64,
                            &mut msg_struct_json,
                        );
                    }

                    // let _ = middleware
                    //     .publish(out_topic, &msg_struct, Some(timestamp))
                    //     .await;

                    let metadata = Metadata::new(out_topic, msg_type, Some(timestamp), None);
                    let serialized_msg = serializer
                        .from_json::<VehicleState>(&metadata, msg_struct_json)
                        .expect("Unable to serialize VehicleState struct from JSON");

                    let _ = middleware
                        .publish_raw(msg_type, out_topic, &serialized_msg)
                        .await;
                }
                "aerosim::types::PrimaryFlightDisplayData" => {
                    if var_prefix.is_empty() {
                        var_prefix = "primary_flight_display_data".to_string();
                    }

                    // Pack data from FMU into output message struct
                    let msg_struct = PrimaryFlightDisplayData::default();

                    let mut msg_struct_json = serde_json::to_value(&msg_struct)
                        .expect("Unable to serialize PrimaryFlightDisplayData struct to JSON");
                    let flat_fields =
                        TypeSupport::get_flat_fields_from_json_object(&msg_struct_json, "");

                    // let flat_fields = TypeSupport::get_flat_field_names(&msg_struct, "");

                    // Set the fields in the message struct using the flat field names and FMU data.
                    for field_name in &flat_fields {
                        set_fmu_field_in_json_struct(
                            fmu_id,
                            field_name,
                            &var_prefix,
                            fmu_var_types,
                            fmu_data_f64,
                            fmu_data_i64,
                            &mut msg_struct_json,
                        );
                    }

                    // let _ = middleware
                    //     .publish(out_topic, &msg_struct, Some(timestamp))
                    //     .await;

                    let metadata = Metadata::new(out_topic, msg_type, Some(timestamp), None);
                    let serialized_msg = serializer
                        .from_json::<PrimaryFlightDisplayData>(&metadata, msg_struct_json)
                        .expect("Unable to serialize PrimaryFlightDisplayData struct from JSON");

                    let _ = middleware
                        .publish_raw(msg_type, out_topic, &serialized_msg)
                        .await;
                }
                _ => {
                    warn!("[{}] Unsupported output topic type: {}", fmu_id, msg_type);
                    continue;
                }
            }
        }
    }
}

pub fn set_fmu_field_in_json_struct(
    fmu_id: &str,
    field_name: &str,
    var_prefix: &str,
    fmu_var_types: &std::collections::HashMap<String, VariableType>,
    fmu_data_f64: &std::collections::HashMap<String, Vec<f64>>,
    fmu_data_i64: &std::collections::HashMap<String, Vec<i64>>,
    msg_struct_json: &mut serde_json::Value,
) {
    // Construct the FMU variable name
    let fmu_var_name = var_prefix.to_string() + "." + field_name;
    match fmu_var_types.get(&fmu_var_name) {
        Some(VariableType::FmiFloat64) => {
            if let Some(value) = fmu_data_f64.get(&fmu_var_name) {
                // info!(
                //     "[{}] Setting field {} to f64 value: {:?}",
                //     fmu_id, fmu_var_name, value[0]
                // );

                // *msg_struct.path_mut::<f64>(field_name.as_str()).unwrap() = value[0];

                let json_path = TypeSupport::dot_notation_to_json_path(field_name);
                *msg_struct_json
                    .pointer_mut(&json_path)
                    .expect("Unable to get field from VehicleState struct") = value[0].into();
            } else {
                warn!("[{}] FMU variable '{}' not found.", fmu_id, fmu_var_name);
            }
        }
        Some(VariableType::FmiInt64) => {
            if let Some(value) = fmu_data_i64.get(&fmu_var_name) {
                // info!(
                //     "[{}] Setting field {} to i64 value: {:?}",
                //     fmu_id, fmu_var_name, value
                // );

                // *msg_struct.path_mut::<i64>(field_name.as_str()).unwrap() = value[0];

                let json_path = TypeSupport::dot_notation_to_json_path(field_name);
                *msg_struct_json
                    .pointer_mut(&json_path)
                    .expect("Unable to get field from VehicleState struct") = value[0].into();
            } else {
                warn!("[{}] FMU variable '{}' not found.", fmu_id, fmu_var_name);
            }
        }
        _ => {
            warn!(
                "[{}] Unsupported FMU variable type for field '{}'.",
                fmu_id, fmu_var_name
            );
        }
    }
}

pub async fn publish_aux_output_topics_fmu3(
    fmu_id: &str,
    fmu_config_json: &serde_json::Value,
    fmu_var_types: &std::collections::HashMap<String, VariableType>,
    fmu_data_f64: &std::collections::HashMap<String, Vec<f64>>,
    timestamp: TimeStamp,
    middleware: &Arc<MiddlewareEnum>,
) {
    // Pack and publish auxiliary output topics as JsonData
    if let Some(aux_out_mapping) = fmu_config_json.get("fmu_aux_output_mapping") {
        for (out_topic, out_var_map) in aux_out_mapping
            .as_object()
            .expect("Unable to get 'fmu_aux_output_mapping' as object")
            .iter()
        {
            let mut data_dict = serde_json::Map::new();
            for (out_topic_var, out_fmu_var) in out_var_map
                .as_object()
                .expect("Unable to get 'out_var_map' as object")
                .iter()
            {
                let out_fmu_var_str = out_fmu_var
                    .as_str()
                    .expect("FMU variable name should be a string");
                let out_fmu_var_type = fmu_var_types
                    .get(out_fmu_var_str)
                    .expect("FMU variable type not found.");

                match out_fmu_var_type {
                    VariableType::FmiFloat64 => {
                        if let Some(out_value) = fmu_data_f64.get(out_fmu_var_str) {
                            if out_value.len() == 1 {
                                // Single value, insert directly
                                data_dict.insert(out_topic_var.to_string(), out_value[0].into());
                            } else {
                                // Array, insert as array
                                data_dict
                                    .insert(out_topic_var.to_string(), out_value.clone().into());
                            }
                        } else {
                            warn!(
                                "[{}] FMU variable '{}' not found for output topic '{}'.",
                                fmu_id, out_fmu_var_str, out_topic
                            );
                        }
                    }
                    _ => {
                        warn!(
                            "[{}] Unsupported FMU variable type '{}' for output topic '{}'.",
                            fmu_id,
                            fmi3_var_type_to_string(out_fmu_var_type),
                            out_topic
                        );
                    }
                };
            }

            let data_msg: JsonData = JsonData::new(data_dict.into());

            let _ = middleware
                .publish(out_topic, &data_msg, Some(timestamp))
                .await;

            // info!(
            //     "[{}] Published auxiliary output topic '{}' with data: {:?}",
            //     fmu_id, out_topic, data_msg
            // );
        }
    }
}
