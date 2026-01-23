use ::log::{error, info, warn};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{
    mpsc::{self, Receiver, Sender, TryRecvError},
    Arc, Mutex,
};
use std::thread::JoinHandle;

use pyo3::prelude::*;
use serde_json::Value;

use fmi::fmi2::import::Fmi2Import;
use fmi::fmi3::{CoSimulation, schema::Causality};
use fmi::schema::traits::FmiModelDescription;
use fmi::traits::FmiInstance;

use aerosim_core::math::round_to_decimal_places;
use aerosim_data::{
    middleware::{
        CallbackClosureRaw, Metadata, Middleware, MiddlewareEnum, MiddlewareRaw,
        MiddlewareRegistry, Serializer, SerializerEnum,
    },
    types::{deserialize_to_json, JsonData, TimeStamp, TypeSupport},
};

use crate::fmu_utils::{
    publish_aux_output_topics_fmu3, publish_component_output_topics_fmu3, set_fmu3_from_json,
    set_init_value_fmu3, Fmi3Model, Fmi3VarInfo, NUM_TIME_DECIMALS, TIME_SEC_TOL,
};

// ----------------------------------------------------------------------------
// FmuDriver struct

#[pyclass]
pub struct FmuDriver {
    #[pyo3(get)]
    fmu_id: String,
    working_dir: String,
    middleware: Arc<MiddlewareEnum>,
    runtime: Arc<tokio::runtime::Runtime>,
    fmu_driver_thread_handle: Option<JoinHandle<()>>,
    fmu_driver_thread_tx_stop: Option<Sender<bool>>,
}

// FmuDriver interface functions that are exposed as Python class methods
#[pymethods]
impl FmuDriver {
    #[new]
    fn __new__(fmu_id: &str, working_dir: &str, middleware_type: &str) -> Self {
        let middleware_type_mod = match middleware_type {
            "kafka" => "kafka",
            "zenoh" => "zenoh",
            _ => {
                warn!(
                    "Unsupported middleware type '{}', defaulting to 'zenoh'",
                    middleware_type
                );
                "zenoh"
            }
        };

        let mut fmu_driver = FmuDriver {
            fmu_id: fmu_id.to_string(),
            working_dir: working_dir.to_string(),
            middleware: MiddlewareRegistry::new()
                .get(middleware_type_mod)
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
        let (tx_orchestrator_msg, rx_orchestrator_msg) = mpsc::channel::<(JsonData, Metadata)>();

        // Subscribe to orchestrator commands topic
        runtime.block_on(async {
            match middleware
                .subscribe::<JsonData>("aerosim.orchestrator.commands", {
                    Box::new(move |data, metadata| {
                        FmuDriver::receive_orchestrator_command_message(
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

        let (tx_stop, rx_stop): (Sender<bool>, Receiver<bool>) = mpsc::channel();
        fmu_driver.fmu_driver_thread_tx_stop = Some(tx_stop);

        let thread_builder = std::thread::Builder::new().name(format!("fmu_driver [{}]", fmu_id));

        let fmu_id = fmu_driver.fmu_id.clone();
        let working_dir = fmu_driver.working_dir.clone();
        fmu_driver.fmu_driver_thread_handle = Some(
            thread_builder
                .spawn(move || {
                    runtime.block_on(FmuDriver::fmu_driver_main(
                        rx_stop,
                        rx_orchestrator_msg,
                        middleware.clone(),
                        &fmu_id,
                        &working_dir,
                    ));
                })
                .expect("Unable to spawn FMU Driver thread"),
        );

        fmu_driver
    }

    fn start(&mut self) -> PyResult<()> {
        info!("[{}] FMU Driver start (no-op).", self.fmu_id);
        Ok(())
    }

    fn stop(&mut self, py: Python<'_>) {
        info!("[{}] Stopping FMU Driver.", self.fmu_id);

        // Send stop flag to thread (may fail if thread already exited via orchestrator command)
        if let Some(tx_stop) = self.fmu_driver_thread_tx_stop.take() {
            let _ = tx_stop.send(true);
        }

        // Wait for thread to finish
        // IMPORTANT: We must release the Python GIL while waiting, otherwise the FMU driver
        // thread will deadlock when it tries to call into Python (via fmi3Terminate/fmi3FreeInstance)
        // to terminate a pythonfmu3-built FMU model since those calls need to acquire the GIL
        // via PyGILState_Ensure().
        if let Some(handle) = self.fmu_driver_thread_handle.take() {
            py.allow_threads(|| {
                let start = std::time::Instant::now();
                loop {
                    if handle.is_finished() {
                        let _ = handle.join();
                        break;
                    }
                    if start.elapsed().as_secs() >= 30 {
                        error!(
                            "[{}] Thread join timed out after 30s - abandoning join!",
                            self.fmu_id
                        );
                        std::mem::forget(handle);
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            });
        }

        info!("[{}] FMU Driver stopped.", self.fmu_id);
    }
}

// Rust-only FmuDriverRust functions
impl FmuDriver {
    // Function for the FMU driver's main thread loop
    async fn fmu_driver_main(
        rx_stop: Receiver<bool>,
        rx_orchestrator_msg: Receiver<(JsonData, Metadata)>,
        middleware: Arc<MiddlewareEnum>,
        fmu_id: &str,
        working_dir: &str,
    ) {
        info!("[{}] FMU Driver main thread started.", fmu_id);

        let mut running = true;
        let mut is_sim_config_loaded = false;
        let mut is_sim_started = false;

        let serializer = middleware.get_serializer();

        let mut fmu_config_json: Value = serde_json::Value::Null;
        let mut all_topics_to_subscribe: HashSet<(String, String)> = HashSet::new();

        let mut fmu_model: Option<Fmi3Model> = None;
        let mut fmu_time: f64 = 0.0;

        let input_data_map: Arc<Mutex<HashMap<String, (Metadata, Value)>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // Channel to send received step trigger message data to this thread for processing
        let (tx_step_msg, rx_step_msg) = mpsc::channel::<Metadata>();

        while running {
            // Check for a received orchestrator command message to process
            match rx_orchestrator_msg.try_recv() {
                Ok((payload, metadata)) => {
                    FmuDriver::process_orchestrator_command_message(
                        fmu_id,
                        working_dir,
                        &middleware,
                        &serializer,
                        &payload,
                        &metadata,
                        &tx_step_msg,
                        &mut running,
                        &mut is_sim_config_loaded,
                        &mut is_sim_started,
                        &mut fmu_config_json,
                        &mut all_topics_to_subscribe,
                        &mut fmu_time,
                        &mut fmu_model,
                        &input_data_map,
                    )
                    .await;
                }
                Err(TryRecvError::Disconnected) => {
                    running = false;
                }
                Err(TryRecvError::Empty) => { /* pass to continue looping */ }
            }

            // Check for a received step trigger message to process
            if running && is_sim_started {
                match rx_step_msg.try_recv() {
                    Ok(metadata) => {
                        FmuDriver::process_step_trigger_message(
                            fmu_id,
                            &fmu_config_json,
                            &middleware,
                            &serializer,
                            &metadata,
                            &mut fmu_time,
                            &mut fmu_model,
                            &input_data_map,
                        )
                        .await;
                    }
                    Err(TryRecvError::Disconnected) => {
                        running = false;
                    }
                    Err(TryRecvError::Empty) => { /* pass to continue looping */ }
                }
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

            tokio::task::yield_now().await;
        }

        // FMU Driver main thread is stopping
        info!("[{}] FMU Driver main thread is stopping...", fmu_id);

        // Terminate and drop the FMU model. We call terminate() first per the FMI spec
        // to properly transition to the Terminated state before calling fmi3FreeInstance
        // when it is dropped.
        if let Some(mut model) = fmu_model.take() {
            info!("[{}] Terminating the FMU model...", fmu_id);
            model.terminate();
        }

        info!("[{}] FMU Driver main thread stopped.", fmu_id);
    }

    // Function to load the FMU config to parse the input topics and populate all_topics_to_subscribe,
    // and return a tuple of (step_trigger_topic, step_trigger_msg_type)
    fn load_fmu_config(
        fmu_id: &str,
        fmu_config_json: &Value,
        all_topics_to_subscribe: &mut HashSet<(String, String)>,
    ) -> (String, String) {
        if let Some(in_topics) = fmu_config_json.get("component_input_topics") {
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
                all_topics_to_subscribe.insert((msg_type.to_string(), in_topic.to_string()));
            }
        }

        if let Some(aux_in_mapping) = fmu_config_json.get("fmu_aux_input_mapping") {
            for (in_topic_root, _) in aux_in_mapping
                .as_object()
                .expect("Unable to get 'fmu_aux_input_mapping' as object")
            {
                all_topics_to_subscribe.insert((
                    "aerosim::types::JsonData".to_string(),
                    in_topic_root.to_string(),
                ));
            }
        }

        if let Some(step_topic_val) = fmu_config_json.get("step_trigger_topic") {
            let step_topic_obj = step_topic_val
                .as_object()
                .expect("Unable to process 'step_trigger_topic' as a JSON object.");
            let step_trigger_topic = step_topic_obj
                .get("topic")
                .and_then(|v| v.as_str())
                .expect("Unable to parse 'step_trigger_topic.topic' from JSON.");
            let step_trigger_msg_type = step_topic_obj
                .get("msg_type")
                .and_then(|v| v.as_str())
                .expect("Unable to parse 'step_trigger_topic.msg_type' from JSON.");
            return (
                step_trigger_topic.to_string(),
                step_trigger_msg_type.to_string(),
            );
        } else {
            info!("[{}] Unable to find 'step_trigger_topic' field in FMU config. Using base 'aerosim.clock' topic as step trigger.", fmu_id);
            return (
                "aerosim.clock".to_string(),
                "aerosim::types::JsonData".to_string(),
            );
        }
    }

    // Function to load the FMU model instance from the config FMU model file path
    fn load_fmu_model(
        fmu_id: &str,
        working_dir: &str,
        fmu_config_json: &Value,
    ) -> Option<Fmi3Model> {
        // ------------------------------------------------------------
        // FMU model file path processing

        let fmu_model_path = fmu_config_json
            .get("fmu_model_path")
            .expect("Unable to get 'fmu_model_path' field from JSON")
            .as_str()
            .expect("Unable to get 'fmu_model_path' as string");
        let mut fmu_model_path_buf = Path::new(fmu_model_path).to_path_buf();

        // Check if file exists at fmu_model_path
        if !fmu_model_path_buf.exists() {
            // If the FMU model path is not found, check if it is relative to
            // the AeroSim root dir
            let aerosim_root = match std::env::var("AEROSIM_ROOT") {
                Ok(root_path) => root_path,
                Err(_) => "".to_string(),
            };

            fmu_model_path_buf = Path::new(&aerosim_root).join(fmu_model_path);
        }

        if !fmu_model_path_buf.exists() {
            // If the FMU model path is still not found, check if it is relative to
            // the working directory
            fmu_model_path_buf = Path::new(&working_dir).join(fmu_model_path);
        }

        let fmu_filename = std::path::absolute(fmu_model_path_buf)
            .expect("Unable to resolve FMU model path as an absolute path.");

        info!("[{}] Loading FMU file: {:?}", fmu_id, fmu_filename);

        // Peek the model description to check the FMI version
        let min_model_desc =
            fmi::import::peek_descr_path(&fmu_filename).expect("Unable to peek model description.");

        if min_model_desc.version_string() == "2.0" {
            // ------------------------------------------------------------
            // FMI 2.0 model processing

            let _fmu_import: Fmi2Import =
                fmi::import::from_path(&fmu_filename).expect("Unable to import FMU file.");

            todo!("FMI 2.0 model processing not implemented yet.");
        } else if min_model_desc.version_string() == "3.0" {
            // ------------------------------------------------------------
            // FMI 3.0 model processing

            return Some(Fmi3Model::new(fmu_id, fmu_filename));
        }

        None
    }

    // Function to initialize the FMU model (set initial values from the config, publish
    // the initial output topics with values for t=0)
    async fn initialize_fmu_model(
        fmu_id: &str,
        fmu_config_json: &Value,
        fmu_time: f64,
        initial_timestamp: &TimeStamp,
        middleware: &Arc<MiddlewareEnum>,
        serializer: &SerializerEnum,
        fmu_model: &mut Option<Fmi3Model>,
    ) {
        if let Some(fmu_model_mut) = fmu_model.as_mut() {
            let (fmu_var_info, fmu_instance) = fmu_model_mut.get_var_info_ref_and_instance_mut();

            // Set initial values for FMU variables set in the "fmu_initial_vals" config
            let fmu_init_vals_obj: Option<&serde_json::Map<String, Value>> = fmu_config_json
                .get("fmu_initial_vals")
                .and_then(|v| v.as_object());
            let mut fmu_input_var_init_vals: Vec<(&String, &Value, &Fmi3VarInfo)> = Vec::new();
            if let Some(fmu_init_vals) = fmu_init_vals_obj {
                for (init_var, init_value) in fmu_init_vals {
                    if let Some(var_info) = fmu_var_info.get_fmu_var_info(init_var) {
                        set_init_value_fmu3(
                            fmu_id,
                            init_var,
                            init_value,
                            var_info,
                            fmu_instance,
                        );
                        if var_info.fmu_var_causality == Causality::Input {
                            fmu_input_var_init_vals.push((init_var, init_value, var_info));
                        }
                    } else {
                        warn!(
                            "[{}] FMU variable '{}' not found in model info.",
                            fmu_id, init_var
                        );
                    }
                }
            }

            // Call FMU API to execute its 'enter initialization mode' function
            // to process the initial values set above
            FmiInstance::enter_initialization_mode(fmu_instance, None, fmu_time, None);

            // Re-set initial values for FMU input-type variables because they get reset to zero
            // during FmiInstance::enter_initialization_mode()
            for (init_var, init_value, var_info) in fmu_input_var_init_vals {
                set_init_value_fmu3(fmu_id, init_var, init_value, var_info, fmu_instance);
            }

            // Call FMU API to execute its 'exit initialization mode' function
            // to be ready to start stepping
            FmiInstance::exit_initialization_mode(fmu_instance);

            // Publish initial value output topics for initial timestamp
            publish_component_output_topics_fmu3(
                fmu_id,
                &fmu_config_json,
                fmu_var_info,
                fmu_instance,
                initial_timestamp,
                middleware,
                serializer,
            )
            .await;

            publish_aux_output_topics_fmu3(
                fmu_id,
                &fmu_config_json,
                fmu_var_info,
                fmu_instance,
                initial_timestamp,
                &middleware,
            )
            .await;
        }
    }

    // Function to execute one sim clock step of the FMU model
    fn step_fmu_model(
        fmu_id: &str,
        fmu_config_json: &Value,
        simtime_sec: f64,
        cur_step_sec: f64,
        fmu_time: &mut f64,
        fmu_model: &mut Option<Fmi3Model>,
        input_data_map: &Arc<Mutex<HashMap<String, (Metadata, Value)>>>,
    ) {
        if let Some(fmu_model_mut) = fmu_model.as_mut() {
            let (fmu_var_info, fmu_instance) = fmu_model_mut.get_var_info_ref_and_instance_mut();

            // ------------------------------------------------------------
            // Write inputs to the FMU

            // Process every input topic that has been received and stored in input_data_map
            {
                let mut input_data_map_lock = input_data_map.lock().unwrap();
                let cur_input_data = input_data_map_lock.drain();

                for (input_topic, (in_msg_metadata, in_msg_json)) in cur_input_data {
                    // info!(
                    //     "[{}] Writing input topic '{}' at timestamp: {:?}",
                    //     fmu_id, input_topic, in_msg_metadata.timestamp_sim
                    // );

                    let in_msg_flat_fields =
                        TypeSupport::get_flat_fields_from_json_object(&in_msg_json, "");

                    // TODO Move this parsing to load_fmu_config()
                    let aux_in_var_map = fmu_config_json
                        .get("fmu_aux_input_mapping")
                        .and_then(|aux_in_mapping| aux_in_mapping.get(&input_topic))
                        .and_then(|aux_in_mapping| aux_in_mapping.as_object());

                    // Iterate through the flat fields and set the FMU variables
                    for in_msg_var in in_msg_flat_fields {
                        set_fmu3_from_json(
                            &in_msg_var,
                            &in_msg_json,
                            &in_msg_metadata,
                            aux_in_var_map,
                            fmu_id,
                            fmu_var_info,
                            fmu_instance,
                        );
                    }
                }
            }

            // ------------------------------------------------------------
            // Do one step of the FMU

            let no_set_fmu_state_prior_to_current_point = false;
            let local_step_sec: f64 = cur_step_sec;

            let mut last_fmu_time = *fmu_time;
            while last_fmu_time < simtime_sec {
                let mut event_handling_needed = false;
                let mut terminate_simulation = false;
                let mut early_return = false;
                let mut last_successful_time: f64 = 0.0;

                fmu_instance.do_step(
                    last_fmu_time,
                    local_step_sec,
                    no_set_fmu_state_prior_to_current_point,
                    &mut event_handling_needed,
                    &mut terminate_simulation,
                    &mut early_return,
                    &mut last_successful_time,
                );

                // Validate that the step was successfully advanced
                let time_stepped = last_successful_time - last_fmu_time;
                if event_handling_needed
                    || terminate_simulation
                    || early_return
                    || (time_stepped - local_step_sec).abs() > TIME_SEC_TOL
                {
                    error!(
                        "[{}] FMU did not successfully advance by the target step time.",
                        fmu_id
                    );
                    return;
                }

                // Advance the time
                last_fmu_time =
                    round_to_decimal_places(last_fmu_time + local_step_sec, NUM_TIME_DECIMALS);
            }

            // Save the completed step's time
            *fmu_time = last_fmu_time;

            // info!("[{}] FMU step done, fmu_time: {}", fmu_id, fmu_time);
        }
    }

    // Callback function for receiving the orchestrator command messages (passes them to FMU driver's
    // main thread loop)
    fn receive_orchestrator_command_message(
        payload: JsonData,
        metadata: Metadata,
        tx_orchestrator_msg: &Sender<(JsonData, Metadata)>,
    ) {
        // Use send - if receiver is dropped, this will return an error which we ignore
        let _ = tx_orchestrator_msg.send((payload, metadata));
    }

    // Function for processing orchestrator command messages to load the
    // sim config, start the FMU driver, and stop the FMU driver (run on the FMU driver's
    // main thread loop)
    async fn process_orchestrator_command_message(
        fmu_id: &str,
        working_dir: &str,
        middleware: &Arc<MiddlewareEnum>,
        serializer: &SerializerEnum,
        payload: &JsonData,
        metadata: &Metadata,
        tx_step_msg: &Sender<Metadata>,
        running: &mut bool,
        is_sim_config_loaded: &mut bool,
        is_sim_started: &mut bool,
        fmu_config_json: &mut Value,
        all_topics_to_subscribe: &mut HashSet<(String, String)>,
        fmu_time: &mut f64,
        fmu_model: &mut Option<Fmi3Model>,
        input_data_map: &Arc<Mutex<HashMap<String, (Metadata, Value)>>>,
    ) {
        let msg_json = payload.get_data().expect("Unable to get JsonData payload.");
        let command = msg_json
            .get("command")
            .expect("Unable to get 'command' field from JSON.")
            .as_str()
            .expect("Unable to get 'command' as string.");

        info!(
            "[{}] FMU Driver received orchestrator command: {}",
            fmu_id, command
        );

        if command == "stop" {
            // ----------------------------------------------------------------
            // Orchestrator stop command

            *running = false;
            return;
        }

        if *is_sim_config_loaded == false && command == "load_config" {
            // ----------------------------------------------------------------
            // Orchestrator load config command

            // Load the FMU config into fmu_config_json
            let sim_config = msg_json
                .pointer("/parameters/sim_config")
                .expect("Unable to get ['parameters']['sim_config'] field from JSON")
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
                    *fmu_config_json = fmu_config.clone();
                    break;
                }
            }

            if fmu_config_json.is_null() {
                warn!("[{}] FMU ID not found in sim config.", fmu_id);
            }

            // info!("[{}] Received fmu_config: {:?}", fmu_id, fmu_config_json);

            // Process fmu_config_json
            let (step_trigger_topic, step_trigger_msg_type) =
                FmuDriver::load_fmu_config(fmu_id, &fmu_config_json, all_topics_to_subscribe);

            // Subscribe to step trigger topic
            info!(
                "[{}] Subscribing to step trigger topic: '{}'",
                fmu_id, step_trigger_topic
            );
            match middleware
                .subscribe_raw(&step_trigger_msg_type, &step_trigger_topic, {
                    let tx_step_msg_copy = tx_step_msg.clone();
                    let serializer_copy = middleware.get_serializer();
                    Box::new(move |payload| {
                        let metadata = serializer_copy
                            .deserialize_metadata(payload)
                            .expect("Unable to deserialize metadata.");
                        FmuDriver::receive_step_trigger_message(metadata, &tx_step_msg_copy);
                        Ok(())
                    })
                })
                .await
            {
                Ok(()) => {
                    info!("[{}] Created step trigger subscriber.", fmu_id)
                }
                Err(e) => warn!(
                    "[{}] Could not create step trigger subscriber: {}",
                    fmu_id, e
                ),
            }

            // Subscribe to all of the topics specified in the sim config
            let all_topic_to_subscribe: Vec<(String, String)> =
                all_topics_to_subscribe.clone().into_iter().collect();

            info!(
                "[{}] Subscribing to topics: {:?}",
                fmu_id, all_topic_to_subscribe
            );

            let _ = middleware
                .subscribe_all_raw(
                    all_topic_to_subscribe,
                    FmuDriver::process_fmu_input_data(
                        fmu_id.to_string(),
                        middleware.get_serializer(),
                        Arc::clone(&input_data_map),
                    ),
                )
                .await;

            // Load the FMU model file
            *fmu_model = FmuDriver::load_fmu_model(fmu_id, working_dir, &fmu_config_json);

            // After loading FMU model file to populate fmu_var_refs, pass
            // through the world origin values as initial values if the FMU has
            // variables for it
            if let Some(fmu_model_ref) = fmu_model.as_ref() {
                let fmu_var_info = fmu_model_ref.get_var_info_ref();
                if fmu_var_info
                    .get_fmu_var_info("world_origin_latitude")
                    .is_some()
                    && fmu_var_info
                        .get_fmu_var_info("world_origin_longitude")
                        .is_some()
                    && fmu_var_info
                        .get_fmu_var_info("world_origin_altitude")
                        .is_some()
                {
                    fmu_config_json["fmu_initial_vals"]["world_origin_latitude"] =
                        sim_config["world"]["origin"]["latitude"].clone();
                    fmu_config_json["fmu_initial_vals"]["world_origin_longitude"] =
                        sim_config["world"]["origin"]["longitude"].clone();
                    fmu_config_json["fmu_initial_vals"]["world_origin_altitude"] =
                        sim_config["world"]["origin"]["altitude"].clone();
                }
            }

            *is_sim_config_loaded = true;
            info!("[{}] Done loading sim config.", fmu_id);
            return;
        }

        if *is_sim_started == false && command == "start" {
            // ----------------------------------------------------------------
            // Orchestrator start command

            {
                // # Save sim start time from the orchestrator (not used anywhere yet)
                let sim_start_time_sec = msg_json
                    .pointer("/parameters/sim_start_time/sec")
                    .expect("Unable to get ['parameters']['sim_start_time']['sec'] field from JSON")
                    .as_i64()
                    .expect("Unable to get 'sec' as i64");
                let sim_start_time_nanosec = msg_json
                    .pointer("/parameters/sim_start_time/nanosec")
                    .expect(
                        "Unable to get ['parameters']['sim_start_time']['nanosec'] field from JSON",
                    )
                    .as_u64()
                    .expect("Unable to get 'nanosec' as u64");
                let sim_start_time =
                    TimeStamp::new(sim_start_time_sec as i32, sim_start_time_nanosec as u32);
                info!("[{}] Sim start time: {:?}", fmu_id, sim_start_time);
            }

            let initial_timestamp = metadata.timestamp_sim;

            //Initialize the FMU model instance to be ready to start stepping
            FmuDriver::initialize_fmu_model(
                fmu_id,
                &fmu_config_json,
                *fmu_time,
                &initial_timestamp,
                &middleware,
                &serializer,
                fmu_model,
            )
            .await;

            *is_sim_started = true;
            return;
        }

        info!("[{}] Ignoring orchestrator command: {}", fmu_id, command);
    }

    // Callback function for receiving the messages to trigger FMU steps (passes them to FMU driver's
    // main thread loop)
    fn receive_step_trigger_message(metadata: Metadata, tx_step_msg: &Sender<Metadata>) {
        // Use send - if receiver is dropped, this will return an error which we ignore
        let _ = tx_step_msg.send(metadata);
    }

    // Function for processing the received messages to trigger FMU steps (run on the FMU driver's
    // main thread loop)
    async fn process_step_trigger_message(
        fmu_id: &str,
        fmu_config_json: &Value,
        middleware: &Arc<MiddlewareEnum>,
        serializer: &SerializerEnum,
        metadata: &Metadata,
        fmu_time: &mut f64,
        fmu_model: &mut Option<Fmi3Model>,
        input_data_map: &Arc<Mutex<HashMap<String, (Metadata, Value)>>>,
    ) {
        if !metadata.is_sim_time_valid() {
            warn!(
                "[{}] Received a step trigger message with an invalid timestamp_sim.",
                fmu_id
            );
            return;
        }
        let timestamp_sim = &metadata.timestamp_sim;

        // info!(
        //     "[{}] FMU Driver thread processing step trigger for timestamp_sim: {:?}",
        //     fmu_id, timestamp_sim
        // );

        let simtime_sec = timestamp_sim.to_sec_rounded(NUM_TIME_DECIMALS);
        let mut cur_step_sec = simtime_sec - *fmu_time;
        if cur_step_sec < 0.0 {
            warn!(
                "[{}] Negative time step for simtime_sec='{}' fmu_time='{}'",
                fmu_id, simtime_sec, fmu_time
            );
            return;
        } else if cur_step_sec < f64::EPSILON {
            warn!(
                "[{}] Zero time step for simtime_sec='{}' fmu_time='{}'",
                fmu_id, simtime_sec, fmu_time
            );
            return;
        }
        cur_step_sec = round_to_decimal_places(cur_step_sec, NUM_TIME_DECIMALS);

        // ----------------------------------------------------------------
        // Step the FMU model instance

        FmuDriver::step_fmu_model(
            fmu_id,
            fmu_config_json,
            simtime_sec,
            cur_step_sec,
            fmu_time,
            fmu_model,
            input_data_map,
        );

        // ----------------------------------------------------------------
        // Publish output data for the current timestamp

        if let Some(fmu_model_mut) = fmu_model.as_mut() {
            let (fmu_var_info, fmu_instance) = fmu_model_mut.get_var_info_ref_and_instance_mut();

            publish_component_output_topics_fmu3(
                fmu_id,
                fmu_config_json,
                fmu_var_info,
                fmu_instance,
                &timestamp_sim,
                middleware,
                serializer,
            )
            .await;

            publish_aux_output_topics_fmu3(
                fmu_id,
                fmu_config_json,
                fmu_var_info,
                fmu_instance,
                &timestamp_sim,
                middleware,
            )
            .await;
        }
    }

    // Callback function for processing the FMU's input topic messages (deserializes and saves the
    // data into the input data map)
    fn process_fmu_input_data(
        fmu_id_str: String,
        serializer: SerializerEnum,
        input_data_map: Arc<Mutex<HashMap<String, (Metadata, Value)>>>,
    ) -> CallbackClosureRaw {
        Box::new(move |payload: &[u8]| {
            // Deserialize metadata from the incoming payload and determine the simulation timestamp.
            // If the simulation timestamp is not valid, compute it based on the real-time platform timestamp.
            let metadata = serializer
                .deserialize_metadata(payload)
                .ok_or(format!("Could not deserialize metadata from payload"))?;

            let mut input_data_map_lock = input_data_map.lock().unwrap();

            // Check if the topic already has data and if the received data is
            // older than the existing data
            if let Some((existing_metadata, _existing_data)) =
                input_data_map_lock.get(&metadata.topic)
            {
                if metadata.is_sim_time_valid()
                    && existing_metadata.is_sim_time_valid()
                    && metadata.timestamp_sim < existing_metadata.timestamp_sim
                {
                    info!(
                        "[{}] Ignoring older data for topic: {}",
                        fmu_id_str, metadata.topic
                    );
                    return Ok(());
                }
            }

            // Deserialize the payload into a JSON value and store it in the input data map
            if let Some(msg_json) = deserialize_to_json(&metadata.type_name, &serializer, payload) {
                input_data_map_lock.insert(metadata.topic.clone(), (metadata, msg_json));
            } else {
                warn!(
                    "[{}] Failed to deserialize payload for topic: {}",
                    fmu_id_str, metadata.topic
                );
            }

            Ok(())
        })
    }
}
