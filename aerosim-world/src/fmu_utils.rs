use fmi::fmi3::schema::VariableType;

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
