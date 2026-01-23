use log::{info, warn};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;

use fmi::fmi3::{import::Fmi3Import, schema::{Fmi3ModelDescription, VariableType}, Common, Fmi3Model as Fmi3ModelTrait, GetSet};
use fmi::schema::fmi3::ArrayableVariableTrait;
use fmi::traits::FmiImport;
use std::ffi::CString;

use aerosim_data::{
    middleware::{Metadata, Middleware, MiddlewareEnum, MiddlewareRaw, Serializer, SerializerEnum},
    types::{
        sensor::{ADSB, GNSS, IMU},
        AircraftEffectorCommand, AutopilotCommand, EffectorState, FlightControlCommand, JsonData,
        PrimaryFlightDisplayData, TimeStamp, TrajectoryVisualization, TypeSupport, VehicleState,
    },
    AerosimMessage,
};

pub const NUM_TIME_DECIMALS: u32 = 6; // round time to microsec decimal place
pub const TIME_SEC_TOL: f64 = 1e-6;

// ----------------------------------------------------------------------------
// Fmi3VarInfo and Fmi3ModelVarInfo structs to collect all of loaded variable info

pub struct Fmi3VarInfo {
    pub fmu_var_ref: u32,
    pub fmu_var_type: VariableType,
    pub fmu_var_causality: fmi::fmi3::schema::Causality,
    pub fmu_var_dim: Vec<u64>,
    pub fmu_var_tot_dim: usize,
}

impl Clone for Fmi3VarInfo {
    fn clone(&self) -> Self {
        let fmu_var_ref = self.fmu_var_ref;
        let fmu_var_type = match self.fmu_var_type {
            VariableType::FmiFloat32 => VariableType::FmiFloat32,
            VariableType::FmiFloat64 => VariableType::FmiFloat64,
            VariableType::FmiInt8 => VariableType::FmiInt8,
            VariableType::FmiUInt8 => VariableType::FmiUInt8,
            VariableType::FmiInt16 => VariableType::FmiInt16,
            VariableType::FmiUInt16 => VariableType::FmiUInt16,
            VariableType::FmiInt32 => VariableType::FmiInt32,
            VariableType::FmiUInt32 => VariableType::FmiUInt32,
            VariableType::FmiInt64 => VariableType::FmiInt64,
            VariableType::FmiUInt64 => VariableType::FmiUInt64,
            VariableType::FmiBoolean => VariableType::FmiBoolean,
            VariableType::FmiString => VariableType::FmiString,
            VariableType::FmiBinary => VariableType::FmiBinary,
        };
        let fmu_var_causality = self.fmu_var_causality;
        let fmu_var_dim = self.fmu_var_dim.clone();
        let fmu_var_tot_dim = self.fmu_var_tot_dim;

        Self {
            fmu_var_ref,
            fmu_var_type,
            fmu_var_causality,
            fmu_var_dim,
            fmu_var_tot_dim,
        }
    }
}

pub struct Fmi3ModelVarInfo {
    pub fmu_var_info: HashMap<String, Fmi3VarInfo>,
}

impl Fmi3ModelVarInfo {
    pub fn new(fmu_id: &str, fmu_model_desc: &Fmi3ModelDescription) -> Self {
        let mut fmu_var_info: HashMap<String, Fmi3VarInfo> = HashMap::new();

        // Collect all of the variable value references
        // Store the vectors first to extend their lifetime
        let float64_vars = fmu_model_desc.model_variables.float64();
        let float32_vars = fmu_model_desc.model_variables.float32();
        let int64_vars = fmu_model_desc.model_variables.int64();
        let int32_vars = fmu_model_desc.model_variables.int32();
        let int16_vars = fmu_model_desc.model_variables.int16();
        let int8_vars = fmu_model_desc.model_variables.int8();
        let uint64_vars = fmu_model_desc.model_variables.uint64();
        let uint32_vars = fmu_model_desc.model_variables.uint32();
        let uint16_vars = fmu_model_desc.model_variables.uint16();
        let uint8_vars = fmu_model_desc.model_variables.uint8();
        let boolean_vars = fmu_model_desc.model_variables.boolean();
        let string_vars = fmu_model_desc.model_variables.string();

        let all_fmu_var_iter = itertools::chain!(
            float64_vars
                .iter()
                .map(|v| *v as &dyn ArrayableVariableTrait),
            float32_vars
                .iter()
                .map(|v| *v as &dyn ArrayableVariableTrait),
            int64_vars
                .iter()
                .map(|v| *v as &dyn ArrayableVariableTrait),
            int32_vars
                .iter()
                .map(|v| *v as &dyn ArrayableVariableTrait),
            int16_vars
                .iter()
                .map(|v| *v as &dyn ArrayableVariableTrait),
            int8_vars
                .iter()
                .map(|v| *v as &dyn ArrayableVariableTrait),
            uint64_vars
                .iter()
                .map(|v| *v as &dyn ArrayableVariableTrait),
            uint32_vars
                .iter()
                .map(|v| *v as &dyn ArrayableVariableTrait),
            uint16_vars
                .iter()
                .map(|v| *v as &dyn ArrayableVariableTrait),
            uint8_vars
                .iter()
                .map(|v| *v as &dyn ArrayableVariableTrait),
            boolean_vars
                .iter()
                .map(|v| *v as &dyn ArrayableVariableTrait),
            string_vars
                .iter()
                .map(|v| *v as &dyn ArrayableVariableTrait),
        );

        for fmu_var in all_fmu_var_iter {
            let fmu_var_ref = fmu_var.value_reference();
            let fmu_var_name = fmu_var.name();
            let fmu_var_type = fmu_var.data_type();
            let fmu_var_causality = fmu_var.causality();
            let fmu_var_dim: Vec<u64> = fmu_var
                .dimensions()
                .iter()
                .map(|d| match d {
                    fmi::fmi3::schema::Dimension::Fixed(start) => *start as u64,
                    fmi::fmi3::schema::Dimension::Variable(_) => {
                        panic!("Error: Only dimensions using 'Fixed' value are supported.")
                    }
                })
                .collect();
            let fmu_var_tot_dim: usize = fmu_var_dim
                .iter()
                .product::<u64>()
                .try_into()
                .expect("Unable to convert total dimension to usize");

            info!(
                "[{}] FMU ref={} var='{}', type={}, dim={:?}, causality={:?}",
                fmu_id,
                fmu_var_ref,
                fmu_var_name,
                fmi3_var_type_to_string(&fmu_var_type),
                fmu_var_dim,
                fmu_var_causality
            );

            fmu_var_info.insert(
                fmu_var_name.to_string(),
                Fmi3VarInfo {
                    fmu_var_ref,
                    fmu_var_type,
                    fmu_var_causality,
                    fmu_var_dim,
                    fmu_var_tot_dim,
                },
            );
        }

        Self { fmu_var_info }
    }

    pub fn get_fmu_var_info(&self, var_name: &str) -> Option<&Fmi3VarInfo> {
        self.fmu_var_info.get(var_name)
    }
}

// ----------------------------------------------------------------------------
// Fmi3Model struct to combine Import and Instance.
// With the fmi crate's main branch (post-v0.5.0), instances no longer hold
// lifetime references to the import, so we can store them together directly.

pub struct Fmi3Model {
    fmu_import: Fmi3Import,
    fmu_instance: fmi::fmi3::instance::InstanceCS,
    var_info: Fmi3ModelVarInfo,
}

impl Fmi3Model {
    pub fn new(fmu_id: &str, fmu_filename: PathBuf) -> Self {
        let fmu_import: Fmi3Import =
            fmi::import::from_path(&fmu_filename).expect("Unable to import FMU file.");

        if std::env::consts::OS == "windows" {
            // Add extracted lib path to system path so any dependency libs can also be found and loaded
            let archive_path = fmu_import.archive_path();
            let mut shared_lib_path = fmu_import
                .shared_lib_path("")
                .expect("Unable to get FMU shared lib path.");
            let _ = shared_lib_path.pop(); // remove blank '.dll' that shared_lib_path() adds at the end
            let compined_lib_path: String = std::path::absolute(archive_path.join(shared_lib_path))
                .expect("Unable to get absolute file path to extracted FMU shared libs.")
                .to_str()
                .unwrap_or_else(|| "")
                .to_string();

            // Join the new path entry with the existing PATH
            let current_path = std::env::var("PATH").unwrap_or_else(|_| String::new());
            let new_path: String = if current_path.is_empty() {
                compined_lib_path
            } else {
                format!("{};{}", current_path, compined_lib_path)
            };
            std::env::set_var("PATH", new_path);
        } else if std::env::consts::OS == "linux" {
            // TODO Update the Linux path like above
        }

        let fmu_instance = fmu_import
            .instantiate_cs("instance1", false, true, false, false, &[])
            .expect("Unable to instantiate FMU instance.");

        let fmu_model_desc = fmu_import.model_description();

        info!(
            "Loaded FMU model name: {}, FMI ver: {}",
            fmu_model_desc.model_name, fmu_model_desc.fmi_version
        );

        let var_info = Fmi3ModelVarInfo::new(fmu_id, fmu_model_desc);

        Self {
            fmu_import,
            fmu_instance,
            var_info,
        }
    }

    pub fn get_model_description_ref(&self) -> &Fmi3ModelDescription {
        self.fmu_import.model_description()
    }

    pub fn get_var_info_ref(&self) -> &Fmi3ModelVarInfo {
        &self.var_info
    }

    pub fn get_fmu_instance_ref(&self) -> &fmi::fmi3::instance::InstanceCS {
        &self.fmu_instance
    }

    pub fn get_fmu_instance_mut(&mut self) -> &mut fmi::fmi3::instance::InstanceCS {
        &mut self.fmu_instance
    }

    /// Returns both var_info (immutable) and fmu_instance (mutable) references together.
    ///
    /// This method exists as a workaround for Rust's borrow checker limitation at method
    /// boundaries. When calling separate methods like `get_var_info_ref()` and
    /// `get_fmu_instance_mut()`, the borrow checker only sees the method signatures
    /// (`&self` vs `&mut self`) and cannot determine that they access different fields.
    /// This prevents holding an immutable reference from one while getting a mutable
    /// reference from the other.
    ///
    /// By returning both references from a single method, Rust can see within the function
    /// body that `self.var_info` and `self.fmu_instance` are disjoint fields, allowing
    /// "split borrowing" of the struct.
    pub fn get_var_info_ref_and_instance_mut(
        &mut self,
    ) -> (&Fmi3ModelVarInfo, &mut fmi::fmi3::instance::InstanceCS) {
        (&self.var_info, &mut self.fmu_instance)
    }

    /// Terminate the FMU instance. This should be called before dropping the model
    /// to properly clean up resources and avoid blocking during fmi3FreeInstance.
    pub fn terminate(&mut self) {
        let _ = Common::terminate(&mut self.fmu_instance);
    }
}

// ----------------------------------------------------------------------------
// FMI 2.0 + 3.0 enum type placeholders to be implemented in the future

// pub enum FmiImportEnum {
//     Fmi2Import(Fmi2Import),
//     Fmi3Import(Fmi3Import),
// }

// pub enum FmiInstanceEnum {
//     Fmi2Instance(fmi::fmi2::instance::InstanceCS),
//     Fmi3Instance(fmi::fmi3::instance::InstanceCS),
// }

// pub enum ModelDescriptionEnum<'md> {
//     Fmi2ModelDescription(&'md fmi::fmi2::schema::Fmi2ModelDescription),
//     Fmi3ModelDescription(&'md fmi::fmi3::schema::Fmi3ModelDescription),
// }

// ---------------------------------------------------------------------------
// FMU Utility functions

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

pub fn aerosim_msg_type_to_prefix(msg_type: &str) -> String {
    match msg_type {
        "aerosim::types::VehicleState" => "vehicle_state".to_string(),
        "aerosim::types::EffectorState" => "effector_state".to_string(),
        "aerosim::types::AutopilotCommand" => "autopilot_command".to_string(),
        "aerosim::types::FlightControlCommand" => "flight_control_command".to_string(),
        "aerosim::types::AircraftEffectorCommand" => "aircraft_effector_command".to_string(),
        "aerosim::types::PrimaryFlightDisplayData" => "primary_flight_display_data".to_string(),
        "aerosim::types::TrajectoryVisualization" => "trajectory_visualization".to_string(),
        "aerosim::types::GNSS" => "gnss".to_string(),
        "aerosim::types::ADSB" => "adsb".to_string(),
        "aerosim::types::IMU" => "imu".to_string(),
        _ => {
            warn!("Unsupported message type: {}", msg_type);
            "".to_string()
        }
    }
}

pub fn set_init_value_fmu3(
    fmu_id: &str,
    init_var: &str,
    init_value: &serde_json::Value,
    fmu_var_info: &Fmi3VarInfo,
    fmu_instance: &mut fmi::fmi3::instance::InstanceCS,
) {
    info!(
        "[{}] Setting initial value '{}' = {}",
        fmu_id, init_var, init_value
    );

    let value_array: Vec<serde_json::Value> = if let Some(val_array) = init_value.as_array() {
        val_array.to_vec()
    } else {
        vec![init_value.clone()]
    };

    match fmu_var_info.fmu_var_type {
        VariableType::FmiFloat64 => {
            let values: Vec<f64> = value_array.iter().filter_map(|v| v.as_f64()).collect();
            let _ = fmu_instance.set_float64(&[fmu_var_info.fmu_var_ref], &values);
        }
        VariableType::FmiFloat32 => {
            let values: Vec<f32> = value_array
                .iter()
                .filter_map(|v| v.as_f64())
                .map(|v| v as f32)
                .collect();
            let _ = fmu_instance.set_float32(&[fmu_var_info.fmu_var_ref], &values);
        }
        VariableType::FmiInt64 => {
            let values: Vec<i64> = value_array.iter().filter_map(|v| v.as_i64()).collect();
            let _ = fmu_instance.set_int64(&[fmu_var_info.fmu_var_ref], &values);
        }
        VariableType::FmiInt32 => {
            let values: Vec<i32> = value_array
                .iter()
                .filter_map(|v| v.as_i64())
                .map(|v| v as i32)
                .collect();
            let _ = fmu_instance.set_int32(&[fmu_var_info.fmu_var_ref], &values);
        }
        VariableType::FmiInt16 => {
            let values: Vec<i16> = value_array
                .iter()
                .filter_map(|v| v.as_i64())
                .map(|v| v as i16)
                .collect();
            let _ = fmu_instance.set_int16(&[fmu_var_info.fmu_var_ref], &values);
        }
        VariableType::FmiInt8 => {
            let values: Vec<i8> = value_array
                .iter()
                .filter_map(|v| v.as_i64())
                .map(|v| v as i8)
                .collect();
            let _ = fmu_instance.set_int8(&[fmu_var_info.fmu_var_ref], &values);
        }
        VariableType::FmiUInt64 => {
            let values: Vec<u64> = value_array.iter().filter_map(|v| v.as_u64()).collect();
            let _ = fmu_instance.set_uint64(&[fmu_var_info.fmu_var_ref], &values);
        }
        VariableType::FmiUInt32 => {
            let values: Vec<u32> = value_array
                .iter()
                .filter_map(|v| v.as_u64())
                .map(|v| v as u32)
                .collect();
            let _ = fmu_instance.set_uint32(&[fmu_var_info.fmu_var_ref], &values);
        }
        VariableType::FmiUInt16 => {
            let values: Vec<u16> = value_array
                .iter()
                .filter_map(|v| v.as_u64())
                .map(|v| v as u16)
                .collect();
            let _ = fmu_instance.set_uint16(&[fmu_var_info.fmu_var_ref], &values);
        }
        VariableType::FmiUInt8 => {
            let values: Vec<u8> = value_array
                .iter()
                .filter_map(|v| v.as_u64())
                .map(|v| v as u8)
                .collect();
            let _ = fmu_instance.set_uint8(&[fmu_var_info.fmu_var_ref], &values);
        }
        VariableType::FmiBoolean => {
            let values: Vec<bool> = value_array.iter().filter_map(|v| v.as_bool()).collect();
            let _ = fmu_instance.set_boolean(&[fmu_var_info.fmu_var_ref], &values);
        }
        VariableType::FmiString => {
            let values: Vec<CString> = value_array
                .iter()
                .filter_map(|v| v.as_str())
                .filter_map(|s| CString::new(s).ok())
                .collect();
            let _ = fmu_instance.set_string(&[fmu_var_info.fmu_var_ref], &values);
        }
        _ => {
            warn!(
                "[{}] Unsupported FMU variable type for variable: {}",
                fmu_id, init_var
            );
        }
    }
}

pub fn set_fmu3_from_json(
    in_msg_var: &str,
    in_msg_json: &Value,
    in_msg_metadata: &Metadata,
    aux_in_var_map: Option<&serde_json::Map<String, Value>>,
    fmu_id: &str,
    fmu_model_var_info_ref: &Fmi3ModelVarInfo,
    fmu_instance: &mut fmi::fmi3::instance::InstanceCS,
) {
    // Process variable based on if it's an aux input mapped variable or a regular
    // component input variable with dot notation naming.
    let in_fmu_var;
    let var_as_pointer;
    if let Some(aux_in_map) = aux_in_var_map {
        // Aux input variable (direct mapping from topic variable to FMU variable)
        if let Some(mapped_var) = aux_in_map.get(in_msg_var).and_then(|v| v.as_str()) {
            in_fmu_var = mapped_var.to_string();
        } else {
            // Skip setting it if this variable is not mapped in this aux topic's config
            return;
        }
        var_as_pointer = false;
    } else {
        // Regular input variable (dot notation fields with prefix)
        let prefix = aerosim_msg_type_to_prefix(&in_msg_metadata.type_name);
        in_fmu_var = prefix.to_string() + "." + in_msg_var;
        var_as_pointer = true;
    }

    // Get the FMU variable info
    let var_info = fmu_model_var_info_ref
        .get_fmu_var_info(&in_fmu_var)
        .expect("FMU variable not found in fmu_model_var_info");

    // Get the variable value from the JSON message
    let mut new_val;
    if var_as_pointer {
        let in_var_json_path = TypeSupport::dot_notation_to_json_path(&in_msg_var);
        new_val = in_msg_json
            .pointer(&in_var_json_path)
            .expect("Unable to get JSON pointer.");
    } else {
        new_val = in_msg_json
            .get(&in_msg_var)
            .expect("Unable to get JSON value directly.");
    }

    // If the new_val (value reference) is not an array, convert it to a reference to a
    // single-element array for setting in the FMU instance.
    let single_elem_array: Value;
    if !new_val.is_array() {
        single_elem_array = serde_json::Value::Array(vec![new_val.clone()]);
        new_val = &single_elem_array;
    }

    // Set the FMU variable based on its type
    match var_info.fmu_var_type {
        VariableType::FmiFloat64 => {
            let values = new_val
                .as_array()
                .expect("Not a valid array of float64.")
                .iter()
                .filter_map(|v| v.as_f64())
                .collect::<Vec<f64>>();
            let _ = fmu_instance.set_float64(&[var_info.fmu_var_ref], &values);
        }
        VariableType::FmiFloat32 => {
            let values = new_val
                .as_array()
                .expect("Not a valid array of float32.")
                .iter()
                .filter_map(|v| v.as_f64())
                .map(|v| v as f32)
                .collect::<Vec<f32>>();
            let _ = fmu_instance.set_float32(&[var_info.fmu_var_ref], &values);
        }
        VariableType::FmiInt64 => {
            let values = new_val
                .as_array()
                .expect("Not a valid array of int64.")
                .iter()
                .filter_map(|v| v.as_i64())
                .collect::<Vec<i64>>();
            let _ = fmu_instance.set_int64(&[var_info.fmu_var_ref], &values);
        }
        VariableType::FmiInt32 => {
            let values = new_val
                .as_array()
                .expect("Not a valid array of int32.")
                .iter()
                .filter_map(|v| v.as_i64())
                .map(|v| v as i32)
                .collect::<Vec<i32>>();
            let _ = fmu_instance.set_int32(&[var_info.fmu_var_ref], &values);
        }
        VariableType::FmiInt16 => {
            let values = new_val
                .as_array()
                .expect("Not a valid array of int16.")
                .iter()
                .filter_map(|v| v.as_i64())
                .map(|v| v as i16)
                .collect::<Vec<i16>>();
            let _ = fmu_instance.set_int16(&[var_info.fmu_var_ref], &values);
        }
        VariableType::FmiInt8 => {
            let values = new_val
                .as_array()
                .expect("Not a valid array of int8.")
                .iter()
                .filter_map(|v| v.as_i64())
                .map(|v| v as i8)
                .collect::<Vec<i8>>();
            let _ = fmu_instance.set_int8(&[var_info.fmu_var_ref], &values);
        }
        VariableType::FmiUInt64 => {
            let values = new_val
                .as_array()
                .expect("Not a valid array of uint64.")
                .iter()
                .filter_map(|v| v.as_u64())
                .collect::<Vec<u64>>();
            let _ = fmu_instance.set_uint64(&[var_info.fmu_var_ref], &values);
        }
        VariableType::FmiUInt32 => {
            let values = new_val
                .as_array()
                .expect("Not a valid array of uint32.")
                .iter()
                .filter_map(|v| v.as_u64())
                .map(|v| v as u32)
                .collect::<Vec<u32>>();
            let _ = fmu_instance.set_uint32(&[var_info.fmu_var_ref], &values);
        }
        VariableType::FmiUInt16 => {
            let values = new_val
                .as_array()
                .expect("Not a valid array of uint16.")
                .iter()
                .filter_map(|v| v.as_u64())
                .map(|v| v as u16)
                .collect::<Vec<u16>>();
            let _ = fmu_instance.set_uint16(&[var_info.fmu_var_ref], &values);
        }
        VariableType::FmiUInt8 => {
            let values = new_val
                .as_array()
                .expect("Not a valid array of uint8.")
                .iter()
                .filter_map(|v| v.as_u64())
                .map(|v| v as u8)
                .collect::<Vec<u8>>();
            let _ = fmu_instance.set_uint8(&[var_info.fmu_var_ref], &values);
        }
        VariableType::FmiBoolean => {
            let values = new_val
                .as_array()
                .expect("Not a valid array of boolean.")
                .iter()
                .filter_map(|v| v.as_bool())
                .collect::<Vec<bool>>();
            let _ = fmu_instance.set_boolean(&[var_info.fmu_var_ref], &values);
        }
        VariableType::FmiString => {
            let values: Vec<CString> = new_val
                .as_array()
                .expect("Not a valid array of string.")
                .iter()
                .filter_map(|v| v.as_str())
                .filter_map(|s| CString::new(s).ok())
                .collect();
            let _ = fmu_instance.set_string(&[var_info.fmu_var_ref], &values);
        }
        _ => {
            warn!(
                "[{}] Unsupported FMU variable type for '{}': {}",
                fmu_id,
                in_fmu_var,
                fmi3_var_type_to_string(&var_info.fmu_var_type)
            );
            return;
        }
    }

    // info!(
    //     "[{}] Set FMU variable '{}' to value: {:?}",
    //     fmu_id, in_fmu_var, new_val
    // );
}

pub fn get_fmu3_f64(
    fmu_var_info: &Fmi3VarInfo,
    fmu_instance: &mut fmi::fmi3::instance::InstanceCS,
) -> Vec<f64> {
    let mut values: Vec<f64> = vec![0.0; fmu_var_info.fmu_var_tot_dim];
    let _ = fmu_instance.get_float64(&[fmu_var_info.fmu_var_ref], &mut values);
    values
}

pub fn get_fmu3_f32(
    fmu_var_info: &Fmi3VarInfo,
    fmu_instance: &mut fmi::fmi3::instance::InstanceCS,
) -> Vec<f32> {
    let mut values: Vec<f32> = vec![0.0; fmu_var_info.fmu_var_tot_dim];
    let _ = fmu_instance.get_float32(&[fmu_var_info.fmu_var_ref], &mut values);
    values
}

pub fn get_fmu3_i64(
    fmu_var_info: &Fmi3VarInfo,
    fmu_instance: &mut fmi::fmi3::instance::InstanceCS,
) -> Vec<i64> {
    let mut values: Vec<i64> = vec![0; fmu_var_info.fmu_var_tot_dim];
    let _ = fmu_instance.get_int64(&[fmu_var_info.fmu_var_ref], &mut values);
    values
}

pub fn get_fmu3_i32(
    fmu_var_info: &Fmi3VarInfo,
    fmu_instance: &mut fmi::fmi3::instance::InstanceCS,
) -> Vec<i32> {
    let mut values: Vec<i32> = vec![0; fmu_var_info.fmu_var_tot_dim];
    let _ = fmu_instance.get_int32(&[fmu_var_info.fmu_var_ref], &mut values);
    values
}

pub fn get_fmu3_i16(
    fmu_var_info: &Fmi3VarInfo,
    fmu_instance: &mut fmi::fmi3::instance::InstanceCS,
) -> Vec<i16> {
    let mut values: Vec<i16> = vec![0; fmu_var_info.fmu_var_tot_dim];
    let _ = fmu_instance.get_int16(&[fmu_var_info.fmu_var_ref], &mut values);
    values
}

pub fn get_fmu3_i8(
    fmu_var_info: &Fmi3VarInfo,
    fmu_instance: &mut fmi::fmi3::instance::InstanceCS,
) -> Vec<i8> {
    let mut values: Vec<i8> = vec![0; fmu_var_info.fmu_var_tot_dim];
    let _ = fmu_instance.get_int8(&[fmu_var_info.fmu_var_ref], &mut values);
    values
}

pub fn get_fmu3_u64(
    fmu_var_info: &Fmi3VarInfo,
    fmu_instance: &mut fmi::fmi3::instance::InstanceCS,
) -> Vec<u64> {
    let mut values: Vec<u64> = vec![0; fmu_var_info.fmu_var_tot_dim];
    let _ = fmu_instance.get_uint64(&[fmu_var_info.fmu_var_ref], &mut values);
    values
}

pub fn get_fmu3_u32(
    fmu_var_info: &Fmi3VarInfo,
    fmu_instance: &mut fmi::fmi3::instance::InstanceCS,
) -> Vec<u32> {
    let mut values: Vec<u32> = vec![0; fmu_var_info.fmu_var_tot_dim];
    let _ = fmu_instance.get_uint32(&[fmu_var_info.fmu_var_ref], &mut values);
    values
}

pub fn get_fmu3_u16(
    fmu_var_info: &Fmi3VarInfo,
    fmu_instance: &mut fmi::fmi3::instance::InstanceCS,
) -> Vec<u16> {
    let mut values: Vec<u16> = vec![0; fmu_var_info.fmu_var_tot_dim];
    let _ = fmu_instance.get_uint16(&[fmu_var_info.fmu_var_ref], &mut values);
    values
}

pub fn get_fmu3_u8(
    fmu_var_info: &Fmi3VarInfo,
    fmu_instance: &mut fmi::fmi3::instance::InstanceCS,
) -> Vec<u8> {
    let mut values: Vec<u8> = vec![0; fmu_var_info.fmu_var_tot_dim];
    let _ = fmu_instance.get_uint8(&[fmu_var_info.fmu_var_ref], &mut values);
    values
}

pub fn get_fmu3_bool(
    fmu_var_info: &Fmi3VarInfo,
    fmu_instance: &mut fmi::fmi3::instance::InstanceCS,
) -> Vec<bool> {
    let mut values: Vec<bool> = vec![false; fmu_var_info.fmu_var_tot_dim];
    let _ = fmu_instance.get_boolean(&[fmu_var_info.fmu_var_ref], &mut values);
    values
}

pub fn get_fmu3_string(
    fmu_var_info: &Fmi3VarInfo,
    fmu_instance: &mut fmi::fmi3::instance::InstanceCS,
) -> Vec<String> {
    let mut values: Vec<CString> = vec![CString::default(); fmu_var_info.fmu_var_tot_dim];
    let _ = fmu_instance.get_string(&[fmu_var_info.fmu_var_ref], &mut values);
    values
        .into_iter()
        .map(|cs| cs.into_string().unwrap_or_default())
        .collect()
}

pub fn set_json_from_fmu3(
    fmu_id: &str,
    fmu_var_name: &str,
    fmu_model_var_info: &Fmi3ModelVarInfo,
    fmu_instance: &mut fmi::fmi3::instance::InstanceCS,
    msg_struct_json: &mut serde_json::Value,
    json_var: &str,
    json_var_as_pointer: bool,
) {
    if let Some(fmu_var_info) = fmu_model_var_info.get_fmu_var_info(&fmu_var_name) {
        // Get the JSON value reference to the variable to write to
        let val_mut = if json_var_as_pointer {
            msg_struct_json
                .pointer_mut(&json_var)
                .expect("Unable to get json_var_path as pointer from JSON struct")
        } else {
            msg_struct_json
                .get_mut(&json_var)
                .expect("Unable to get json_var_path directly from JSON struct")
        };

        // Get the FMU variable value based on its type
        let fmu_value: serde_json::Value = match fmu_var_info.fmu_var_type {
            VariableType::FmiFloat64 => get_fmu3_f64(fmu_var_info, fmu_instance).into(),
            VariableType::FmiFloat32 => get_fmu3_f32(fmu_var_info, fmu_instance).into(),
            VariableType::FmiInt64 => get_fmu3_i64(fmu_var_info, fmu_instance).into(),
            VariableType::FmiInt32 => get_fmu3_i32(fmu_var_info, fmu_instance).into(),
            VariableType::FmiInt16 => get_fmu3_i16(fmu_var_info, fmu_instance).into(),
            VariableType::FmiInt8 => get_fmu3_i8(fmu_var_info, fmu_instance).into(),
            VariableType::FmiUInt64 => get_fmu3_u64(fmu_var_info, fmu_instance).into(),
            VariableType::FmiUInt32 => get_fmu3_u32(fmu_var_info, fmu_instance).into(),
            VariableType::FmiUInt16 => get_fmu3_u16(fmu_var_info, fmu_instance).into(),
            VariableType::FmiUInt8 => get_fmu3_u8(fmu_var_info, fmu_instance).into(),
            VariableType::FmiBoolean => get_fmu3_bool(fmu_var_info, fmu_instance).into(),
            VariableType::FmiString => get_fmu3_string(fmu_var_info, fmu_instance).into(),
            _ => {
                warn!(
                    "[{}] Unsupported FMU variable type for field '{}'.",
                    fmu_id, fmu_var_name
                );
                return;
            }
        };

        // Write the FMU value to the JSON value reference
        *val_mut = if fmu_var_info.fmu_var_dim.is_empty() {
            // If the variable is a scalar, set it as a single number value
            // from the FMU value array that should contain only one element.
            fmu_value
                .as_array()
                .and_then(|arr| arr.get(0))
                .cloned()
                .unwrap_or_else(|| {
                    warn!(
                        "[{}] Couldn't parse variable '{}' value as a scalar.",
                        fmu_id, fmu_var_name
                    );
                    serde_json::Value::Null
                })
        } else {
            // Set it directly to the FMU value array.
            fmu_value
        };
    } else {
        warn!(
            "[{}] FMU variable '{}' not found in model variable info.",
            fmu_id, fmu_var_name
        );
    }
}

pub async fn publish_component_output_topics_fmu3(
    fmu_id: &str,
    fmu_config_json: &serde_json::Value,
    fmu_model_var_info: &Fmi3ModelVarInfo,
    fmu_instance: &mut fmi::fmi3::instance::InstanceCS,
    timestamp: &TimeStamp,
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

            let mut var_prefix = aerosim_msg_type_to_prefix(msg_type);
            // Override var_prefix if one is provided in config
            if let Some(var_prefix_config) = out_topic_info.get("var_prefix") {
                var_prefix = var_prefix_config
                    .as_str()
                    .expect("Unable to parse 'var_prefix' as string")
                    .to_string();
            }

            let metadata = Metadata::new(out_topic, msg_type, Some(*timestamp), None);

            let serialized_msg: Vec<u8> = match msg_type {
                "aerosim::types::VehicleState" => pack_raw_aerosim_fmu_msg::<VehicleState>(
                    fmu_id,
                    &var_prefix,
                    fmu_model_var_info,
                    fmu_instance,
                    serializer,
                    &metadata,
                ),
                "aerosim::types::EffectorState" => pack_raw_aerosim_fmu_msg::<EffectorState>(
                    fmu_id,
                    &var_prefix,
                    fmu_model_var_info,
                    fmu_instance,
                    serializer,
                    &metadata,
                ),
                "aerosim::types::AutopilotCommand" => pack_raw_aerosim_fmu_msg::<AutopilotCommand>(
                    fmu_id,
                    &var_prefix,
                    fmu_model_var_info,
                    fmu_instance,
                    serializer,
                    &metadata,
                ),
                "aerosim::types::FlightControlCommand" => {
                    pack_raw_aerosim_fmu_msg::<FlightControlCommand>(
                        fmu_id,
                        &var_prefix,
                        fmu_model_var_info,
                        fmu_instance,
                        serializer,
                        &metadata,
                    )
                }
                "aerosim::types::AircraftEffectorCommand" => {
                    pack_raw_aerosim_fmu_msg::<AircraftEffectorCommand>(
                        fmu_id,
                        &var_prefix,
                        fmu_model_var_info,
                        fmu_instance,
                        serializer,
                        &metadata,
                    )
                }
                "aerosim::types::PrimaryFlightDisplayData" => {
                    pack_raw_aerosim_fmu_msg::<PrimaryFlightDisplayData>(
                        fmu_id,
                        &var_prefix,
                        fmu_model_var_info,
                        fmu_instance,
                        serializer,
                        &metadata,
                    )
                }
                "aerosim::types::TrajectoryVisualization" => {
                    pack_raw_aerosim_fmu_msg::<TrajectoryVisualization>(
                        fmu_id,
                        &var_prefix,
                        fmu_model_var_info,
                        fmu_instance,
                        serializer,
                        &metadata,
                    )
                }
                "aerosim::types::GNSS" => pack_raw_aerosim_fmu_msg::<GNSS>(
                    fmu_id,
                    &var_prefix,
                    fmu_model_var_info,
                    fmu_instance,
                    serializer,
                    &metadata,
                ),
                "aerosim::types::ADSB" => pack_raw_aerosim_fmu_msg::<ADSB>(
                    fmu_id,
                    &var_prefix,
                    fmu_model_var_info,
                    fmu_instance,
                    serializer,
                    &metadata,
                ),
                "aerosim::types::IMU" => pack_raw_aerosim_fmu_msg::<IMU>(
                    fmu_id,
                    &var_prefix,
                    fmu_model_var_info,
                    fmu_instance,
                    serializer,
                    &metadata,
                ),
                _ => {
                    warn!("[{}] Unsupported output topic type: {}", fmu_id, msg_type);
                    continue;
                }
            };

            let _ = middleware
                .publish_raw(msg_type, out_topic, &serialized_msg)
                .await;

            // info!(
            //     "[{}] Published output topic '{}' with message type '{}'",
            //     fmu_id, out_topic, msg_type
            // )
        }
    }
}

pub fn pack_raw_aerosim_fmu_msg<T: AerosimMessage + Default>(
    fmu_id: &str,
    var_prefix: &str,
    fmu_model_var_info: &Fmi3ModelVarInfo,
    fmu_instance: &mut fmi::fmi3::instance::InstanceCS,
    serializer: &SerializerEnum,
    metadata: &Metadata,
) -> Vec<u8> {
    // Create a new instance of the aerosim message struct with default values
    let msg_struct = T::default();

    // Convert the aerosim message struct to a JSON object
    let mut out_msg_json =
        serde_json::to_value(&msg_struct).expect("Unable to serialize struct to JSON");

    let out_flat_fields = TypeSupport::get_flat_fields_from_json_object(&out_msg_json, "");

    // Set the fields in the JSON object using the dot notation field names as JSON path pointers.
    for field_name in &out_flat_fields {
        let fmu_var_name = var_prefix.to_string() + "." + field_name;
        let json_var_path = TypeSupport::dot_notation_to_json_path(field_name);
        set_json_from_fmu3(
            fmu_id,
            &fmu_var_name,
            fmu_model_var_info,
            fmu_instance,
            &mut out_msg_json,
            &json_var_path,
            true,
        );
    }

    // Return the serialized raw aerosim message from the JSON object
    serializer
        .from_json::<T>(metadata, out_msg_json)
        .expect("Unable to serialize struct from JSON")
}

pub async fn publish_aux_output_topics_fmu3(
    fmu_id: &str,
    fmu_config_json: &serde_json::Value,
    fmu_model_var_info: &Fmi3ModelVarInfo,
    fmu_instance: &mut fmi::fmi3::instance::InstanceCS,
    timestamp: &TimeStamp,
    middleware: &Arc<MiddlewareEnum>,
) {
    // Pack and publish auxiliary output topics as JsonData
    if let Some(aux_out_mapping) = fmu_config_json.get("fmu_aux_output_mapping") {
        for (out_topic, out_var_map) in aux_out_mapping
            .as_object()
            .expect("Unable to get 'fmu_aux_output_mapping' as object")
            .iter()
        {
            // Create a new JSON object to hold the output data
            let mut data_value = serde_json::Value::from(serde_json::Map::new());

            // Set the fields in the JSON object using the direct variable names
            // from the aux output mapping.
            for (out_topic_var, out_fmu_var) in out_var_map
                .as_object()
                .expect("Unable to get 'out_var_map' as object")
                .iter()
            {
                // Insert the variable as a key with a null value to be able
                // to set it using the variable name as a JSON pointer.
                data_value
                    .as_object_mut()
                    .unwrap()
                    .insert(out_topic_var.clone(), serde_json::Value::Null);

                let out_fmu_var_str = out_fmu_var
                    .as_str()
                    .expect("FMU variable name should be a string");

                // Set the value from the FMU instance to the JSON object
                set_json_from_fmu3(
                    fmu_id,
                    out_fmu_var_str,
                    fmu_model_var_info,
                    fmu_instance,
                    &mut data_value,
                    out_topic_var,
                    false,
                );
            }

            let data_msg = JsonData::new(data_value);

            let _ = middleware
                .publish(out_topic, &data_msg, Some(*timestamp))
                .await;

            // info!(
            //     "[{}] Published auxiliary output topic '{}' with data: {:?}",
            //     fmu_id, out_topic, data_msg
            // );
        }
    }
}
