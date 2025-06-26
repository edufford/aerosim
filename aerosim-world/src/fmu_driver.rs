use ::log::{info, warn};
use aerosim_data::{
    middleware::MiddlewareRaw,
    types::{ActorState, Vector3, VehicleState},
};
use serde_json::Value;
use std::collections::HashSet;
use std::sync::{
    mpsc::{self, Receiver, Sender, TryRecvError},
    Arc,
};
use std::thread::JoinHandle;

use pyo3::prelude::*;

use aerosim_data::{
    middleware::{Metadata, Middleware, MiddlewareEnum, MiddlewareRegistry, Serializer},
    types::{JsonData, TimeStamp},
};

#[pyclass]
pub struct FmuDriverRust {
    #[pyo3(get)]
    fmu_id: String,
    _working_dir: String,
    middleware: Arc<MiddlewareEnum>,
    runtime: Arc<tokio::runtime::Runtime>,
    fmu_driver_thread_handle: Option<JoinHandle<()>>,
    fmu_driver_thread_tx_stop: Option<Sender<bool>>,
}

#[pymethods]
impl FmuDriverRust {
    #[new]
    fn __new__(fmu_id: &str, working_dir: &str, _middleware_type: &str) -> Self {
        let mut fmu_driver = FmuDriverRust {
            fmu_id: fmu_id.to_string(),
            _working_dir: working_dir.to_string(),
            middleware: MiddlewareRegistry::new()
                .get("kafka")
                .expect("Couldn't create middleware."),
            runtime: Arc::new(
                tokio::runtime::Runtime::new().expect("Couldn't create tokio runtime."),
            ),
            fmu_driver_thread_handle: None,
            fmu_driver_thread_tx_stop: None,
        };

        let middleware = Arc::clone(&fmu_driver.middleware);
        let runtime = Arc::clone(&fmu_driver.runtime);

        // Channels to send received message data from subscriber to FMU Driver thread for processing
        let (tx_clock_msg, rx_clock_msg) = mpsc::channel::<(JsonData, Metadata)>();
        let (tx_orchestrator_msg, rx_orchestrator_msg) = mpsc::channel::<(JsonData, Metadata)>();

        // Subscribe to orchestrator commands topic
        runtime.block_on(async {
            match middleware
                .subscribe::<JsonData>("aerosim.orchestrator.commands", {
                    Box::new(move |data, metadata| {
                        FmuDriverRust::handle_orchestrator_command_message(
                            data,
                            metadata,
                            &tx_orchestrator_msg,
                        );
                        Ok(())
                    })
                })
                .await
            {
                Ok(()) => {
                    info!("Created aerosim.orchestrator.commands subscriber.")
                }
                Err(e) => warn!(
                    "Could not create aerosim.orchestrator.commands subscriber: {}",
                    e
                ),
            }
        });

        // Subscribe to clock topic
        runtime.block_on(async {
            match middleware
                .subscribe::<JsonData>("aerosim.clock", {
                    let fmu_id = fmu_driver.fmu_id.clone();
                    Box::new(move |data, metadata| {
                        FmuDriverRust::handle_clock_message(data, metadata, &tx_clock_msg, &fmu_id);
                        Ok(())
                    })
                })
                .await
            {
                Ok(()) => {
                    info!("Created aerosim.clock subscriber.")
                }
                Err(e) => warn!("Could not create aerosim.clock subscriber: {}", e),
            }
        });

        let (tx_stop, rx_stop): (Sender<bool>, Receiver<bool>) = mpsc::channel();
        fmu_driver.fmu_driver_thread_tx_stop = Some(tx_stop);

        let thread_builder = std::thread::Builder::new().name(format!("fmu_driver [{}]", fmu_id));

        let fmu_id = fmu_driver.fmu_id.clone();
        fmu_driver.fmu_driver_thread_handle = Some(
            thread_builder
                .spawn(move || {
                    runtime.block_on(FmuDriverRust::fmu_driver_main(
                        rx_stop,
                        rx_clock_msg,
                        rx_orchestrator_msg,
                        middleware.clone(),
                        &fmu_id,
                    ))
                })
                .expect("Unable to spawn FMU Driver thread"),
        );

        fmu_driver
    }

    fn start(&mut self) -> PyResult<()> {
        info!("[{}] FMU Driver start (no-op).", self.fmu_id);
        Ok(())
    }

    fn stop(&mut self) {
        info!("[{}] Stopping FMU Driver thread.", self.fmu_id);

        // Stop thread
        match self.fmu_driver_thread_tx_stop.take() {
            Some(tx_stop) => match tx_stop.send(true) {
                Ok(_) => {}
                Err(e) => {
                    warn!("Could not send stop flag to FMU Driver thread: {:?}", e);
                }
            },
            None => {
                return;
            }
        }

        let handle = self
            .fmu_driver_thread_handle
            .take()
            .expect("No FMU Driver thread handle, was it started?");

        handle
            .join()
            .expect("Thread should have stopped after receiving stop flag.");
    }
}

// Rust-only FmuDriverRust functions
impl FmuDriverRust {
    async fn fmu_driver_main(
        rx_stop: Receiver<bool>,
        rx_clock_msg: Receiver<(JsonData, Metadata)>,
        rx_orchestrator_msg: Receiver<(JsonData, Metadata)>,
        middleware: Arc<MiddlewareEnum>,
        fmu_id: &str,
    ) {
        info!("[{}] FMU Driver main thread started.", fmu_id);

        let mut running = true;

        let mut fmu_config_json: Value = serde_json::Value::Null;

        let mut all_topics_to_subscribe: HashSet<(String, String)> = HashSet::new();
        let mut aux_topics_to_subscribe: HashSet<String> = HashSet::new();
        let mut aux_topics_to_publish: HashSet<String> = HashSet::new();

        while running {
            // Check for a received orchestrator command message to process
            match rx_orchestrator_msg.try_recv() {
                Ok((payload, metadata)) => {
                    let msg_json = payload.get_data().expect("Unable to get JsonData payload.");
                    let command = msg_json
                        .get("command")
                        .expect("Unable to get 'command' field from JSON.")
                        .as_str()
                        .expect("Unable to get 'command' as string.");

                    info!(
                        "[{}] FMU Driver thread processing topic: {} command {}",
                        fmu_id, metadata.topic, command
                    );

                    match command {
                        "stop" => {
                            running = false;
                        }

                        "load_config" => {
                            info!("[{}] Received orchestrator load command.", fmu_id);

                            // ----------------------------------------------------------------
                            // Load the FMU config into fmu_config_json

                            let sim_config = msg_json
                                .pointer("/parameters/sim_config")
                                .expect(
                                    "Unable to get ['parameters']['sim_config'] field from JSON",
                                )
                                .clone();

                            for fmu_config in sim_config
                                .get("fmu_models")
                                .expect("Unable to get 'fmu_models' field from JSON")
                                .as_array()
                                .expect("Unable to get 'fmu_models' as array")
                            {
                                if fmu_config
                                    .get("id")
                                    .expect("Unable to get 'id' field from JSON")
                                    .as_str()
                                    .expect("Unable to get 'id' as string")
                                    == fmu_id
                                {
                                    fmu_config_json = fmu_config.clone();
                                    break;
                                }
                            }

                            if fmu_config_json.is_null() {
                                warn!("[{}] FMU ID not found in sim config.", fmu_id);
                            }

                            info!("[{}] Received fmu_config: {:?}", fmu_id, fmu_config_json);

                            // ----------------------------------------------------------------
                            // Process fmu_config_json

                            // TODO Refactor this block into load_config()
                            {
                                if let Some(in_topics) =
                                    fmu_config_json.get("component_input_topics")
                                {
                                    for in_topic_info in in_topics
                                        .as_array()
                                        .expect("Unable to get 'component_input_topics' as array")
                                    {
                                        let in_topic = in_topic_info
                                            .get("topic")
                                            .expect("Unable to get 'topic' field from JSON")
                                            .as_str()
                                            .expect("Unable to get 'topic' as string");
                                        let msg_type = in_topic_info
                                            .get("msg_type")
                                            .expect("Unable to get 'msg_type' field from JSON")
                                            .as_str()
                                            .expect("Unable to get 'msg_type' as string");
                                        all_topics_to_subscribe
                                            .insert((msg_type.to_string(), in_topic.to_string()));
                                        // self.in_topic_data[in_topic] = {}
                                    }
                                }

                                if let Some(aux_in_mapping) =
                                    fmu_config_json.get("fmu_aux_input_mapping")
                                {
                                    for (in_topic_root, _) in aux_in_mapping
                                        .as_object()
                                        .expect("Unable to get 'fmu_aux_input_mapping' as object")
                                    {
                                        all_topics_to_subscribe.insert((
                                            "aerosim::types::JsonData".to_string(),
                                            in_topic_root.to_string(),
                                        ));
                                        aux_topics_to_subscribe.insert(in_topic_root.to_string());
                                        // self.in_topic_data[in_topic_root] = {}
                                    }
                                }

                                if let Some(aux_out_mapping) =
                                    fmu_config_json.get("fmu_aux_output_mapping")
                                {
                                    for (out_topic_root, _) in aux_out_mapping
                                        .as_object()
                                        .expect("Unable to get 'fmu_aux_output_mapping' as object")
                                    {
                                        aux_topics_to_publish.insert(out_topic_root.to_string());
                                        // self.out_topic_data[out_topic_root] = {}
                                    }
                                }
                            }

                            // ----------------------------------------------------------------
                            // Subscribe to all of the topics specified in the sim config

                            let all_topic_to_subscribe: Vec<(String, String)> =
                                all_topics_to_subscribe.clone().into_iter().collect();

                            info!(
                                "[{}] Subscribing to topics: {:?}",
                                fmu_id, all_topic_to_subscribe
                            );

                            let _ = middleware
                                .subscribe_all_raw(all_topic_to_subscribe, {
                                    // Prepare necessary components to be moved into the callback scope.
                                    let serializer = middleware.get_serializer();
                                    let fmu_id_str = fmu_id.to_string();

                                    // TODO Refactor this into input_data_callback()
                                    Box::new(move |payload: &[u8]| {
                                        // Deserialize metadata from the incoming payload and determine the simulation timestamp.
                                        // If the simulation timestamp is not valid, compute it based on the real-time platform timestamp.
                                        let metadata = serializer
                                            .deserialize_metadata(payload)
                                            .ok_or(format!(
                                                "Could not deserialize metadata from payload"
                                            ))?;

                                        // TODO Handle all input data in this callback
                                        info!(
                                            "[{}] Subscribe all callback received topic: {}",
                                            fmu_id_str, metadata.topic
                                        );

                                        match metadata.type_name.as_str() {
                                            "aerosim::types::JsonData" => {
                                                let data = serializer
                                                    .deserialize_data::<JsonData>(payload)
                                                    .expect("Error deserializing JsonData");
                                                info!(
                                                    "Deserialized JsonData: {:?}",
                                                    data.get_data()
                                                );
                                            }
                                            _ => {
                                                warn!(
                                                    "Skipping deserialization of unknown type: {}",
                                                    metadata.type_name
                                                );
                                            }
                                        }

                                        Ok(())
                                    })
                                })
                                .await;

                            // ----------------------------------------------------------------
                            // Load the FMU model file

                            // self.load_fmu()

                            // ----------------------------------------------------------------
                            // After loading FMU model file to populate self.fmu_var_refs, pass
                            // through the world origin values if the FMU has variables for it

                            // if (
                            //     "world_origin_latitude" in self.fmu_var_refs
                            //     and "world_origin_longitude" in self.fmu_var_refs
                            //     and "world_origin_altitude" in self.fmu_var_refs
                            // ):
                            //     self.fmu_config_json["fmu_initial_vals"][
                            //         "world_origin_latitude"
                            //     ] = sim_config["world"]["origin"]["latitude"]

                            //     self.fmu_config_json["fmu_initial_vals"][
                            //         "world_origin_longitude"
                            //     ] = sim_config["world"]["origin"]["longitude"]

                            //     self.fmu_config_json["fmu_initial_vals"][
                            //         "world_origin_altitude"
                            //     ] = sim_config["world"]["origin"]["altitude"]

                            // self._is_sim_config_loaded = True

                            info!("[{}] Done loading sim config.", fmu_id);
                        }

                        "start" => {
                            // Publish dummy "aerosim.actor1.vehicle_state" as initial sync topic
                            let veh_state = VehicleState::new(
                                ActorState::default(),
                                Vector3::default(),
                                Vector3::default(),
                                Vector3::default(),
                                Vector3::default(),
                            );

                            info!("Publishing initial sync topic vehicle_state.");

                            let timestamp_sim = TimeStamp { sec: 0, nanosec: 0 };
                            let _ = middleware
                                .publish(
                                    "aerosim.actor1.vehicle_state",
                                    &veh_state,
                                    Some(timestamp_sim),
                                )
                                .await;
                        }

                        "load_scene_graph" => { /* No-op for FMU driver */ }

                        &_ => {
                            warn!("Unknown orchestrator command: {}", command);
                        }
                    };
                }
                Err(TryRecvError::Disconnected) => {
                    running = false;
                }
                Err(TryRecvError::Empty) => { /* pass to continue looping */ }
            }

            // Check for a received clock message to process
            match rx_clock_msg.try_recv() {
                Ok((payload, _metadata)) => {
                    let msg_json = payload.get_data().expect("Unable to get JsonData payload.");
                    let timestamp_sim = TimeStamp::new(
                        msg_json["timestamp_sim"]["sec"].as_i64().unwrap() as i32,
                        msg_json["timestamp_sim"]["nanosec"].as_u64().unwrap() as u32,
                    );

                    info!(
                        "[{}] FMU Driver thread processing clock step: {:?}",
                        fmu_id, timestamp_sim
                    );

                    // Publish dummy "aerosim.actor1.vehicle_state" topic
                    let veh_state = VehicleState::new(
                        ActorState::default(),
                        Vector3::default(),
                        Vector3::default(),
                        Vector3::default(),
                        Vector3::default(),
                    );

                    let timestamp_sim = TimeStamp { sec: 0, nanosec: 0 };
                    let _ = middleware
                        .publish(
                            "aerosim.actor1.vehicle_state",
                            &veh_state,
                            Some(timestamp_sim),
                        )
                        .await;
                }
                Err(TryRecvError::Disconnected) => {
                    running = false;
                }
                Err(TryRecvError::Empty) => { /* pass to continue looping */ }
            }

            // Check for stop flag to shutdown the thread
            match rx_stop.try_recv() {
                Ok(flag) => {
                    if flag {
                        running = false;
                    }
                }
                Err(TryRecvError::Disconnected) => {
                    running = false;
                }
                Err(TryRecvError::Empty) => { /* pass to continue looping */ }
            }
        }

        tokio::task::yield_now().await;

        info!("[{}] FMU Driver main thread stopped.", fmu_id);
    }

    fn handle_orchestrator_command_message(
        payload: JsonData,
        metadata: Metadata,
        tx_orchestrator_msg: &Sender<(JsonData, Metadata)>,
    ) {
        tx_orchestrator_msg
            .send((payload, metadata))
            .expect("UNable to send orchestrator msg data to FMU Driver thread.");
    }

    fn handle_clock_message(
        payload: JsonData,
        metadata: Metadata,
        tx_clock_msg: &Sender<(JsonData, Metadata)>,
        _fmu_id: &str,
    ) {
        tx_clock_msg
            .send((payload, metadata))
            .expect("UNable to send clock msg data to FMU Driver thread.");
    }
}

#[cfg(test)]
mod tests {
    // use super::*;

    use fmi::fmi3::{import::Fmi3Import, instance::CoSimulation};
    use fmi::schema::fmi3::VariableType;
    use fmi::traits::{FmiImport, FmiInstance};
    use fmi::{fmi3::instance::Common, schema::fmi3::ArrayableVariableTrait};

    fn var_type_to_string(var_type: VariableType) -> String {
        return match var_type {
            VariableType::FmiBoolean => "bool".to_string(),
            VariableType::FmiFloat32 => "float32".to_string(),
            VariableType::FmiFloat64 => "float64".to_string(),
            VariableType::FmiInt32 => "int32".to_string(),
            VariableType::FmiInt64 => "int64".to_string(),
            _ => "unknown".to_string(),
        };
    }

    #[test]
    fn test_fmu_float64_array() {
        let aerosim_root = match std::env::var("AEROSIM_ROOT") {
            Ok(root_path) => root_path,
            Err(_) => "".to_string(),
        };

        // Import FMU model

        let fmu_import: Fmi3Import =
            fmi::import::from_path(aerosim_root + "/examples/fmu/evtol_vehicle_fmu.fmu")
                .expect("Unable to import FMU file.");

        let fmu_model_desc = fmu_import.model_description();
        println!("Model name: {}", fmu_model_desc.model_name);

        // Parse FMU variable info

        let all_fmu_var_iter = itertools::chain!(
            fmu_model_desc
                .model_variables
                .float32
                .iter()
                .map(|v| v as &dyn ArrayableVariableTrait),
            fmu_model_desc
                .model_variables
                .float64
                .iter()
                .map(|v| v as &dyn ArrayableVariableTrait),
        );

        println!("Loaded FMU variable info:");
        for fmu_var in all_fmu_var_iter {
            let fmu_var_ref = fmu_var.value_reference();
            let fmu_var_name = fmu_var.name();
            let fmu_var_type = fmu_var.data_type();
            let fmu_var_dim = fmu_var.dimensions();
            let fmu_var_caus = fmu_var.causality();
            println!(
                "FMU ref={} var={} type={} dim={:?} causality={:?}",
                fmu_var_ref,
                fmu_var_name,
                var_type_to_string(fmu_var_type),
                fmu_var_dim,
                fmu_var_caus
            );
        }

        // Load FMU instance
        let mut fmu_instance: fmi::fmi3::instance::InstanceCS = fmu_import
            .instantiate_cs("instance1", false, true, false, false, &[])
            .expect("Unable to instantiate FMU.");

        println!(
            "FMU instance name: {}, version: {}",
            FmiInstance::name(&fmu_instance),
            FmiInstance::get_version(&fmu_instance)
        );

        let mut fmu_time: f64 = 0.0;

        FmiInstance::enter_initialization_mode(&mut fmu_instance, None, fmu_time, None);
        FmiInstance::exit_initialization_mode(&mut fmu_instance);

        // Read then set array variable values
        let mut init_ned_m = [0.0, 0.0, 0.0];
        let init_ned_m_vrs = [40];
        let _ = fmu_instance.get_float64(&init_ned_m_vrs, &mut init_ned_m);
        println!("Initial, init_ned_m: {:?}", init_ned_m);
        init_ned_m = [1.0, 2.0, 3.0];
        let _ = fmu_instance.set_float64(&init_ned_m_vrs, &init_ned_m);
        println!("Before step, init_ned_m: {:?}", init_ned_m);

        // Step FMU
        let communication_step_size = 0.02;
        let no_set_fmu_state_prior_to_current_point = false;

        let mut event_handling_needed = false;
        let mut terminate_simulation = false;
        let mut early_return = false;
        let mut last_successful_time: f64 = 0.0;

        fmu_instance.do_step(
            fmu_time,
            communication_step_size,
            no_set_fmu_state_prior_to_current_point,
            &mut event_handling_needed,
            &mut terminate_simulation,
            &mut early_return,
            &mut last_successful_time,
        );

        fmu_time = last_successful_time;

        // Read array variable values
        let mut init_ned_m = [0.0, 0.0, 0.0];
        let init_ned_m_vrs = [40];
        let _ = fmu_instance.get_float64(&init_ned_m_vrs, &mut init_ned_m);
        println!(
            "After step, fmu_time: {}, init_ned_m: {:?}",
            fmu_time, init_ned_m
        );
    }

    #[test]
    fn test_fmu_int64_array() {
        let aerosim_root = match std::env::var("AEROSIM_ROOT") {
            Ok(root_path) => root_path,
            Err(_) => "".to_string(),
        };

        // Import FMU model

        let fmu_import: Fmi3Import =
            fmi::import::from_path(aerosim_root + "/examples/fmu/Int64ArrayBouncingBall.fmu")
                .expect("Unable to import FMU file.");

        let fmu_model_desc = fmu_import.model_description();
        println!("Model name: {}", fmu_model_desc.model_name);

        // Parse FMU variable info

        let all_fmu_var_iter = itertools::chain!(
            fmu_model_desc
                .model_variables
                .float32
                .iter()
                .map(|v| v as &dyn ArrayableVariableTrait),
            fmu_model_desc
                .model_variables
                .float64
                .iter()
                .map(|v| v as &dyn ArrayableVariableTrait),
            fmu_model_desc
                .model_variables
                .int64
                .iter()
                .map(|v| v as &dyn ArrayableVariableTrait),
        );

        println!("Loaded FMU variable info:");
        for fmu_var in all_fmu_var_iter {
            let fmu_var_ref = fmu_var.value_reference();
            let fmu_var_name = fmu_var.name();
            let fmu_var_type = fmu_var.data_type();
            let fmu_var_dim = fmu_var.dimensions();
            let fmu_var_caus = fmu_var.causality();
            println!(
                "FMU ref={} var={} type={} dim={:?} causality={:?}",
                fmu_var_ref,
                fmu_var_name,
                var_type_to_string(fmu_var_type),
                fmu_var_dim,
                fmu_var_caus
            );
        }

        // Load FMU instance
        let mut fmu_instance: fmi::fmi3::instance::InstanceCS = fmu_import
            .instantiate_cs("instance1", false, true, false, false, &[])
            .expect("Unable to instantiate FMU.");

        println!(
            "FMU instance name: {}, version: {}",
            FmiInstance::name(&fmu_instance),
            FmiInstance::get_version(&fmu_instance)
        );

        let fmu_time: f64 = 0.0;

        FmiInstance::enter_initialization_mode(&mut fmu_instance, None, fmu_time, None);
        FmiInstance::exit_initialization_mode(&mut fmu_instance);

        // Read then set array variable values
        let mut h_int_array = [0, 0, 0];
        let h_int_array_vrs = [8];
        let res = fmu_instance.get_int64(&h_int_array_vrs, &mut h_int_array);
        println!("{:?}", res);
        println!("Initial, h_int_array: {:?}", h_int_array);

        h_int_array = [11, 22, 33];
        let res = fmu_instance.set_int64(&h_int_array_vrs, &h_int_array);
        println!("{:?}", res);
        println!("Before step, h_int_array: {:?}", h_int_array);

        h_int_array = [0, 0, 0];
        let res = fmu_instance.get_int64(&h_int_array_vrs, &mut h_int_array);
        println!("{:?}", res);
        println!("Written, h_int_array: {:?}", h_int_array);
    }
}
