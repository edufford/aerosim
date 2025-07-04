use ::log::{info, warn};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{
    mpsc::{self, Receiver, Sender, TryRecvError},
    Arc, Mutex,
};
use std::thread::JoinHandle;

use pyo3::prelude::*;
use serde_json::Value;

use fmi::fmi2::import::Fmi2Import;
use fmi::fmi3::import::Fmi3Import;
use fmi::fmi3::instance::{CoSimulation, Common};
use fmi::schema::{
    fmi3::{ArrayableVariableTrait, VariableType},
    traits::FmiModelDescription,
};
use fmi::traits::{FmiImport, FmiInstance};

use aerosim_data::{
    middleware::{
        CallbackClosureRaw, Metadata, Middleware, MiddlewareEnum, MiddlewareRaw,
        MiddlewareRegistry, Serializer, SerializerEnum,
    },
    types::{deserialize_to_json, JsonData, TimeStamp, TypeSupport},
};

use crate::fmu_utils::{
    fmi3_var_type_to_string, publish_aux_output_topics_fmu3, publish_component_output_topics_fmu3,
    set_init_value_fmu3,
};

#[pyclass]
pub struct FmuDriverRust {
    #[pyo3(get)]
    fmu_id: String,
    working_dir: String,
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
            working_dir: working_dir.to_string(),
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
        let working_dir = fmu_driver.working_dir.clone();
        fmu_driver.fmu_driver_thread_handle = Some(
            thread_builder
                .spawn(move || {
                    runtime.block_on(FmuDriverRust::fmu_driver_main(
                        rx_stop,
                        rx_clock_msg,
                        rx_orchestrator_msg,
                        middleware.clone(),
                        &fmu_id,
                        &working_dir,
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

pub struct Fmi3Model {
    fmu_import: Box<Fmi3Import>,
    fmu_instance: fmi::fmi3::instance::InstanceCS<'static>,
}

impl Fmi3Model {
    pub fn new(fmu_filename: PathBuf) -> Self {
        // Allocate the import on the heap and leak it to get a 'static reference.
        let fmu_import_box: Box<Fmi3Import> =
            Box::new(fmi::import::from_path(&fmu_filename).expect("Unable to import FMU file."));
        let fmu_import_static: &'static Fmi3Import = Box::leak(fmu_import_box);

        let fmu_instance = fmu_import_static
            .instantiate_cs("instance1", false, true, false, false, &[])
            .expect("Unable to instantiate FMU instance.");

        // Rebuild the Box to deallocate it later.
        let fmu_import =
            unsafe { Box::from_raw(fmu_import_static as *const Fmi3Import as *mut Fmi3Import) };

        Self {
            fmu_import,
            fmu_instance,
        }
    }

    pub fn get_fmu_instance(&mut self) -> &mut fmi::fmi3::instance::InstanceCS<'static> {
        &mut self.fmu_instance
    }
}

// pub struct FmiModel<'a> {
//     fmu_import: Box<Fmi3Import>,
//     fmu_instance: fmi::fmi3::instance::InstanceCS<'a>,
// }

// impl<'a> FmiModel<'a> {
//     pub fn new(fmu_filename: PathBuf) -> Self {
//         // Allocate the import on the heap.
//         let fmu_import: Box<Fmi3Import> =
//             Box::new(fmi::import::from_path(&fmu_filename).expect("Unable to import FMU file."));

//         // Manually extend the lifetime of the reference to match 'a through a raw pointer.
//         let fmu_import_ref: &'a Fmi3Import =
//             unsafe { &*(fmu_import.as_ref() as *const Fmi3Import) };

//         let fmu_instance = fmu_import_ref
//             .instantiate_cs("instance1", false, true, false, false, &[])
//             .expect("Unable to instantiate FMU instance.");

//         Self {
//             fmu_import,
//             fmu_instance,
//         }
//     }
// }

// #[self_referencing]
// pub struct FmiModel {
//     fmu_import: Rc<Fmi3Import>,
//     #[covariant]
//     #[borrows(fmu_import)]
//     fmu_instance: fmi::fmi3::instance::InstanceCS<'this>,
// }

// pub enum FmiImportEnum {
//     Fmi2Import(Fmi2Import),
//     Fmi3Import(Fmi3Import),
// }

// pub enum FmiInstanceEnum<'a> {
//     Fmi2Instance(fmi::fmi2::instance::InstanceCS<'a>),
//     Fmi3Instance(fmi::fmi3::instance::InstanceCS<'a>),
// }

// pub enum ModelDescriptionEnum<'a> {
//     Fmi2ModelDescription(&'a fmi::fmi2::schema::Fmi2ModelDescription),
//     Fmi3ModelDescription(&'a fmi::fmi3::schema::Fmi3ModelDescription),
// }

pub fn round_microsec(sec: f64) -> f64 {
    (sec * 1e6_f64).round() / 1e6_f64
}

fn input_data_callback(
    fmu_id_str: String,
    serializer: SerializerEnum,
    input_data_map: Arc<Mutex<HashMap<String, (TimeStamp, Value)>>>,
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
        if input_data_map_lock.contains_key(&metadata.topic)
            && metadata.is_sim_time_valid()
            && metadata.timestamp_sim < input_data_map_lock.get(&metadata.topic).unwrap().0
        {
            info!(
                "[{}] Ignoring older data for topic: {}",
                fmu_id_str, metadata.topic
            );
            return Ok(());
        }

        // Deserialize the payload into a JSON value and store it in the input data map
        if let Some(msg_json) = deserialize_to_json(&metadata.type_name, &serializer, payload) {
            input_data_map_lock.insert(metadata.topic.clone(), (metadata.timestamp_sim, msg_json));
        } else {
            warn!(
                "[{}] Failed to deserialize payload for topic: {}",
                fmu_id_str, metadata.topic
            );
        }

        Ok(())
    })
}

// Rust-only FmuDriverRust functions
impl FmuDriverRust {
    async fn fmu_driver_main(
        rx_stop: Receiver<bool>,
        rx_clock_msg: Receiver<(JsonData, Metadata)>,
        rx_orchestrator_msg: Receiver<(JsonData, Metadata)>,
        middleware: Arc<MiddlewareEnum>,
        fmu_id: &str,
        working_dir: &str,
    ) {
        info!("[{}] FMU Driver main thread started.", fmu_id);

        let mut running = true;

        let serializer = middleware.get_serializer();

        let mut fmu_config_json: Value = serde_json::Value::Null;

        let mut fmu_var_refs: HashMap<String, u32> = HashMap::new();
        let mut fmu_var_types: HashMap<String, VariableType> = HashMap::new();
        let mut fmu_var_causality: HashMap<String, fmi::fmi3::schema::Causality> = HashMap::new();
        let mut fmu_var_dims: HashMap<String, Vec<u64>> = HashMap::new();

        let mut all_topics_to_subscribe: HashSet<(String, String)> = HashSet::new();
        let mut aux_topics_to_subscribe: HashSet<String> = HashSet::new();
        let mut aux_topics_to_publish: HashSet<String> = HashSet::new();

        let mut fmu_model: Option<Fmi3Model> = None;
        let mut fmu_time: f64 = 0.0;

        let input_data_map: Arc<Mutex<HashMap<String, (TimeStamp, Value)>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let mut fmu_data_f64: HashMap<String, Vec<f64>> = HashMap::new();
        let mut fmu_data_i64: HashMap<String, Vec<i64>> = HashMap::new();

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

                            // info!("[{}] Received fmu_config: {:?}", fmu_id, fmu_config_json);

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
                                .subscribe_all_raw(
                                    all_topic_to_subscribe,
                                    input_data_callback(
                                        fmu_id.to_string(),
                                        middleware.get_serializer(),
                                        Arc::clone(&input_data_map),
                                    ),
                                )
                                .await;

                            // ----------------------------------------------------------------
                            // Load the FMU model file

                            // TODO Refactor this block into load_fmu()
                            {
                                // ------------------------------------------------------------
                                // FMU model file path processing

                                let fmu_model_path = fmu_config_json
                                    .get("fmu_model_path")
                                    .expect("Unable to get 'fmu_model_path' field from JSON")
                                    .as_str()
                                    .expect("Unable to get 'fmu_model_path' as string");
                                let mut fmu_model_path_buf =
                                    Path::new(fmu_model_path).to_path_buf();

                                // Check if file exists at fmu_model_path
                                if !fmu_model_path_buf.exists() {
                                    // If the FMU model path is not found, check if it is relative to
                                    // the AeroSim root dir
                                    let aerosim_root = match std::env::var("AEROSIM_ROOT") {
                                        Ok(root_path) => root_path,
                                        Err(_) => "".to_string(),
                                    };

                                    fmu_model_path_buf =
                                        Path::new(&aerosim_root).join(fmu_model_path);
                                }

                                if !fmu_model_path_buf.exists() {
                                    // If the FMU model path is still not found, check if it is relative to
                                    // the working directory
                                    fmu_model_path_buf =
                                        Path::new(&working_dir).join(fmu_model_path);
                                }

                                let fmu_filename = fmu_model_path_buf
                                    .canonicalize()
                                    .expect("Unable to canonicalize fmu_model_path");

                                info!("[{}] Loading FMU file: {:?}", fmu_id, fmu_filename);

                                // Peek the model description to check the FMI version
                                let min_model_desc = fmi::import::peek_descr_path(&fmu_filename)
                                    .expect("Unable to peek model description.");

                                if min_model_desc.version_string() == "2.0" {
                                    // ------------------------------------------------------------
                                    // FMI 2.0 model processing

                                    let _fmu_import: Fmi2Import =
                                        fmi::import::from_path(&fmu_filename)
                                            .expect("Unable to import FMU file.");

                                    // TODO implement FMI2 model processing
                                } else if min_model_desc.version_string() == "3.0" {
                                    // ------------------------------------------------------------
                                    // FMI 3.0 model processing

                                    // Load the FMU instance
                                    fmu_model = Some(Fmi3Model::new(fmu_filename));

                                    // let fmu_import: Rc<Fmi3Import> = Rc::new(
                                    //     fmi::import::from_path(&fmu_filename)
                                    //         .expect("Unable to import FMU file."),
                                    // );

                                    // TODO load this info inside FmiModel
                                    let fmu_model_desc =
                                        fmu_model.as_ref().unwrap().fmu_import.model_description();

                                    info!(
                                        "[{}] FMU model name: {}, FMI ver: {}",
                                        fmu_id,
                                        fmu_model_desc.model_name,
                                        fmu_model_desc.fmi_version
                                    );

                                    // Collect all of the variable value references
                                    let all_fmu_var_iter = itertools::chain!(
                                        fmu_model_desc
                                            .model_variables
                                            .float64
                                            .iter()
                                            .map(|v| v as &dyn ArrayableVariableTrait),
                                        fmu_model_desc
                                            .model_variables
                                            .float32
                                            .iter()
                                            .map(|v| v as &dyn ArrayableVariableTrait),
                                        fmu_model_desc
                                            .model_variables
                                            .int64
                                            .iter()
                                            .map(|v| v as &dyn ArrayableVariableTrait),
                                        fmu_model_desc
                                            .model_variables
                                            .int32
                                            .iter()
                                            .map(|v| v as &dyn ArrayableVariableTrait),
                                        fmu_model_desc
                                            .model_variables
                                            .int16
                                            .iter()
                                            .map(|v| v as &dyn ArrayableVariableTrait),
                                        fmu_model_desc
                                            .model_variables
                                            .int8
                                            .iter()
                                            .map(|v| v as &dyn ArrayableVariableTrait),
                                        fmu_model_desc
                                            .model_variables
                                            .uint64
                                            .iter()
                                            .map(|v| v as &dyn ArrayableVariableTrait),
                                        fmu_model_desc
                                            .model_variables
                                            .uint32
                                            .iter()
                                            .map(|v| v as &dyn ArrayableVariableTrait),
                                        fmu_model_desc
                                            .model_variables
                                            .uint16
                                            .iter()
                                            .map(|v| v as &dyn ArrayableVariableTrait),
                                        fmu_model_desc
                                            .model_variables
                                            .uint8
                                            .iter()
                                            .map(|v| v as &dyn ArrayableVariableTrait),
                                        fmu_model_desc
                                            .model_variables
                                            .boolean
                                            .iter()
                                            .map(|v| v as &dyn ArrayableVariableTrait),
                                        fmu_model_desc
                                            .model_variables
                                            .string
                                            .iter()
                                            .map(|v| v as &dyn ArrayableVariableTrait),
                                    );

                                    for fmu_var in all_fmu_var_iter {
                                        let fmu_var_ref = fmu_var.value_reference();
                                        let fmu_var_name = fmu_var.name();
                                        let fmu_var_type = fmu_var.data_type();
                                        let fmu_var_caus = fmu_var.causality();
                                        let fmu_var_dim: Vec<u64> = fmu_var
                                            .dimensions()
                                            .iter()
                                            .map(|d| {
                                                d.start.expect("Error: Only dimensions using 'start' value are supported.") as u64
                                            })
                                            .collect();

                                        info!(
                                            "[{}] FMU ref={} var='{}', type={}, dim={:?}, causality={:?}",
                                            fmu_id,
                                            fmu_var_ref,
                                            fmu_var_name,
                                            fmi3_var_type_to_string(&fmu_var_type),
                                            fmu_var_dim,
                                            fmu_var_caus
                                        );

                                        fmu_var_refs.insert(fmu_var_name.to_string(), fmu_var_ref);
                                        fmu_var_types
                                            .insert(fmu_var_name.to_string(), fmu_var_type);
                                        fmu_var_causality
                                            .insert(fmu_var_name.to_string(), fmu_var_caus);
                                        fmu_var_dims.insert(fmu_var_name.to_string(), fmu_var_dim);
                                    }
                                }
                            }

                            // ----------------------------------------------------------------
                            // After loading FMU model file to populate fmu_var_refs, pass
                            // through the world origin values as initial values if the FMU has
                            // variables for it

                            if fmu_var_refs.contains_key("world_origin_latitude")
                                && fmu_var_refs.contains_key("world_origin_longitude")
                                && fmu_var_refs.contains_key("world_origin_altitude")
                            {
                                fmu_config_json["fmu_initial_vals"]["world_origin_latitude"] =
                                    sim_config["world"]["origin"]["latitude"].clone();
                                fmu_config_json["fmu_initial_vals"]["world_origin_longitude"] =
                                    sim_config["world"]["origin"]["longitude"].clone();
                                fmu_config_json["fmu_initial_vals"]["world_origin_altitude"] =
                                    sim_config["world"]["origin"]["altitude"].clone();
                            }

                            // self._is_sim_config_loaded = True

                            info!("[{}] Done loading sim config.", fmu_id);
                        }

                        "start" => {
                            info!("[{}] Received orchestrator start command.", fmu_id);
                            // # Save sim start time from the orchestrator (not used anywhere yet)
                            let sim_start_time_sec = msg_json
                                .pointer("/parameters/sim_start_time/sec")
                                .expect(
                                    "Unable to get ['parameters']['sim_start_time']['sec'] field from JSON",
                                ).as_i64().expect("Unable to get 'sec' as i64");
                            let sim_start_time_nanosec = msg_json
                                .pointer("/parameters/sim_start_time/nanosec")
                                .expect(
                                    "Unable to get ['parameters']['sim_start_time']['nanosec'] field from JSON",
                                ).as_u64().expect("Unable to get 'nanosec' as u64");
                            let sim_start_time = TimeStamp::new(
                                sim_start_time_sec as i32,
                                sim_start_time_nanosec as u32,
                            );
                            info!("[{}] Sim start time: {:?}", fmu_id, sim_start_time);

                            let initial_timestamp = metadata.timestamp_sim;

                            // Initialize the FMU model instance to be ready to start stepping
                            {
                                // self.init_fmu()
                                let fmu_instance = fmu_model.as_mut().unwrap().get_fmu_instance();

                                // Set initial values for FMU variables set in the "fmu_initial_vals" config
                                if let Some(fmu_init_vals_json) =
                                    fmu_config_json.get("fmu_initial_vals")
                                {
                                    let fmu_init_vals_obj = fmu_init_vals_json.as_object().expect(
                                        "Unable to get 'fmu_initial_vals' as a JSON object",
                                    );
                                    for (init_var, init_value) in fmu_init_vals_obj {
                                        set_init_value_fmu3(
                                            fmu_id,
                                            init_var,
                                            init_value,
                                            &fmu_var_refs,
                                            &fmu_var_types,
                                            fmu_instance,
                                        );
                                    }
                                }

                                // Initialize the FMU states
                                FmiInstance::enter_initialization_mode(
                                    fmu_instance,
                                    None,
                                    fmu_time,
                                    None,
                                );

                                // Exit initialization mode to be ready to start stepping
                                FmiInstance::exit_initialization_mode(fmu_instance);
                            }

                            {
                                // self.init_fmu()
                                let fmu_instance = fmu_model.as_mut().unwrap().get_fmu_instance();

                                // Publish initial value output topics for initial timestamp
                                publish_component_output_topics_fmu3(
                                    fmu_id,
                                    &fmu_config_json,
                                    &fmu_var_types,
                                    &fmu_var_refs,
                                    &fmu_var_dims,
                                    fmu_instance,
                                    initial_timestamp,
                                    &middleware,
                                    &serializer,
                                )
                                .await;

                                publish_aux_output_topics_fmu3(
                                    fmu_id,
                                    &fmu_config_json,
                                    &fmu_var_types,
                                    &fmu_var_refs,
                                    &fmu_var_dims,
                                    fmu_instance,
                                    initial_timestamp,
                                    &middleware,
                                )
                                .await;
                            }

                            // self._is_sim_started = True
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

                    // ------------------------------------------------------------------------
                    // Step the FMU model instance

                    {
                        // self.step_fmu(simtime_as_sec)
                        if let Some(fmu_model_ref) = fmu_model.as_mut() {
                            let simtime_sec = round_microsec(timestamp_sim.to_sec());
                            let cur_step_sec = simtime_sec - fmu_time;
                            if cur_step_sec < 0.0 {
                                warn!(
                                    "[{}] Negative time step for simtime_sec='{}' fmu_time='{}'",
                                    fmu_id, simtime_sec, fmu_time
                                );
                            } else {
                                let fmu_instance = fmu_model_ref.get_fmu_instance();

                                // ------------------------------------------------------------
                                // Write inputs to the FMU

                                // TODO Process every input topic that has been received and stored in input_data_map
                                {
                                    let mut input_data_map_lock = input_data_map.lock().unwrap();
                                    let cur_input_data = input_data_map_lock.drain();

                                    for (input_topic, (timestamp, msg_json)) in cur_input_data {
                                        info!(
                                            "[{}] Writing input topic '{}' at timestamp: {:?}",
                                            fmu_id, input_topic, timestamp
                                        );

                                        let flat_fields =
                                            TypeSupport::get_flat_fields_from_json_object(
                                                &msg_json, "",
                                            );

                                        // Iterate through the flat fields and set the FMU variables
                                        for field in flat_fields {
                                            let json_path =
                                                TypeSupport::dot_notation_to_json_path(&field);
                                            let var_ref = fmu_var_refs
                                                .get(&field)
                                                .expect("FMU variable reference not found.");
                                            let var_type = fmu_var_types
                                                .get(&field)
                                                .expect("FMU variable type not found.");

                                            match var_type {
                                                VariableType::FmiFloat64 => {
                                                    let new_val_f64 = msg_json.pointer(&json_path).expect("Unable to get field value from input message.")
                                                                    .as_f64()
                                                                    .expect("Field value is not a valid float64.");

                                                    let values = vec![new_val_f64];

                                                    let _ = fmu_instance
                                                        .set_float64(&[*var_ref], &values);
                                                    info!(
                                                        "[{}] Set FMU variable '{}' to value: {:?}",
                                                        fmu_id, field, values
                                                    );
                                                }
                                                VariableType::FmiInt64 => {
                                                    let new_val_i64 = msg_json.pointer(&json_path).expect("Unable to get field value from input message.")
                                                                    .as_i64()
                                                                    .expect("Field value is not a valid int64.");

                                                    let values = vec![new_val_i64];

                                                    let _ = fmu_instance
                                                        .set_int64(&[*var_ref], &values);
                                                    info!(
                                                        "[{}] Set FMU variable '{}' to value: {:?}",
                                                        fmu_id, field, values
                                                    );
                                                }
                                                _ => {
                                                    warn!(
                                                                    "[{}] Unsupported FMU variable type for '{}': {}",
                                                                    fmu_id,
                                                                    field,
                                                                    fmi3_var_type_to_string(var_type)
                                                                );
                                                    continue;
                                                }
                                            }
                                        }
                                    }
                                }

                                // ------------------------------------------------------------
                                // Do one step of the FMU
                                let no_set_fmu_state_prior_to_current_point = false;

                                let mut event_handling_needed = false;
                                let mut terminate_simulation = false;
                                let mut early_return = false;
                                let mut last_successful_time: f64 = 0.0;

                                fmu_instance.do_step(
                                    fmu_time,
                                    cur_step_sec,
                                    no_set_fmu_state_prior_to_current_point,
                                    &mut event_handling_needed,
                                    &mut terminate_simulation,
                                    &mut early_return,
                                    &mut last_successful_time,
                                );

                                // Advance the time
                                fmu_time = round_microsec(last_successful_time);

                                info!("[{}] FMU step done, fmu_time: {}", fmu_id, fmu_time);

                                // ------------------------------------------------------------
                                // Read outputs from the FMU

                                // Store latest values for all FMU in/output variables in fmu_data_* maps
                                for (fmu_var, fmu_var_type) in fmu_var_types.iter() {
                                    // Read the FMU variable value based on its type
                                    match fmu_var_type {
                                        VariableType::FmiFloat64 => {
                                            let var_ref = fmu_var_refs
                                                .get(fmu_var)
                                                .expect("FMU variable reference not found.");
                                            let var_dim = fmu_var_dims
                                                .get(fmu_var)
                                                .expect("FMU variable dimensions not found.");
                                            let var_dim_tot =
                                                var_dim.iter().product::<u64>() as usize;
                                            let mut values: Vec<f64> = vec![0.0; var_dim_tot];

                                            let _ =
                                                fmu_instance.get_float64(&[*var_ref], &mut values);

                                            fmu_data_f64.insert(fmu_var.clone(), values);
                                        }
                                        VariableType::FmiInt64 => {
                                            let var_ref = fmu_var_refs
                                                .get(fmu_var)
                                                .expect("FMU variable reference not found.");
                                            let var_dim = fmu_var_dims
                                                .get(fmu_var)
                                                .expect("FMU variable dimensions not found.");
                                            let var_dim_tot =
                                                var_dim.iter().product::<u64>() as usize;
                                            let mut values: Vec<i64> = vec![0; var_dim_tot];

                                            let _ =
                                                fmu_instance.get_int64(&[*var_ref], &mut values);

                                            fmu_data_i64.insert(fmu_var.clone(), values);
                                        }
                                        // TODO Refactor into a helper function and handle other variable types
                                        _ => {
                                            warn!(
                                                "[{}] Unsupported FMU variable type: {}",
                                                fmu_id,
                                                fmi3_var_type_to_string(fmu_var_type)
                                            );
                                        }
                                    }
                                }

                                // Debug test read variables
                                for (fmu_var, fmu_value) in fmu_data_f64.iter() {
                                    info!(
                                        "[{}] FMU variable '{}' = {:?}",
                                        fmu_id, fmu_var, fmu_value
                                    );
                                }
                                for (fmu_var, fmu_value) in fmu_data_i64.iter() {
                                    info!(
                                        "[{}] FMU variable '{}' = {:?}",
                                        fmu_id, fmu_var, fmu_value
                                    );
                                }
                            }
                        }
                    }

                    // ------------------------------------------------------------------------
                    // Publish output data for the current timestamp

                    let fmu_instance = fmu_model.as_mut().unwrap().get_fmu_instance();

                    publish_component_output_topics_fmu3(
                        fmu_id,
                        &fmu_config_json,
                        &fmu_var_types,
                        &fmu_var_refs,
                        &fmu_var_dims,
                        fmu_instance,
                        timestamp_sim,
                        &middleware,
                        &serializer,
                    )
                    .await;

                    publish_aux_output_topics_fmu3(
                        fmu_id,
                        &fmu_config_json,
                        &fmu_var_types,
                        &fmu_var_refs,
                        &fmu_var_dims,
                        fmu_instance,
                        timestamp_sim,
                        &middleware,
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
    use super::*;

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
                fmi3_var_type_to_string(&fmu_var_type),
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
                fmi3_var_type_to_string(&fmu_var_type),
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
