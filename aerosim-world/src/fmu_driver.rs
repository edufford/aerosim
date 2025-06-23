use ::log::{info, warn};
use aerosim_data::types::{ActorState, Vector3, VehicleState};
use std::sync::{
    mpsc::{self, Receiver, Sender, TryRecvError},
    Arc,
};
use std::thread::JoinHandle;

use pyo3::prelude::*;

use aerosim_data::{
    middleware::{Metadata, Middleware, MiddlewareEnum, MiddlewareRegistry},
    types::{JsonData, TimeStamp},
};

#[pyclass]
pub struct FmuDriverRust {
    #[pyo3(get)]
    fmu_id: String,
    _working_dir: String,
    _sim_config: String,
    middleware: Arc<MiddlewareEnum>,
    runtime: Arc<tokio::runtime::Runtime>,
    fmu_driver_thread_handle: Option<JoinHandle<()>>,
    fmu_driver_thread_tx_stop: Option<Sender<bool>>,
}

#[pymethods]
impl FmuDriverRust {
    #[new]
    fn __new__(fmu_id: &str, working_dir: &str, _middleware_type: &str) -> Self {
        FmuDriverRust {
            fmu_id: fmu_id.to_string(),
            _working_dir: working_dir.to_string(),
            _sim_config: Default::default(),
            middleware: MiddlewareRegistry::new()
                .get("kafka")
                .expect("Couldn't create middleware."),
            runtime: Arc::new(
                tokio::runtime::Runtime::new().expect("Couldn't create tokio runtime."),
            ),
            fmu_driver_thread_handle: None,
            fmu_driver_thread_tx_stop: None,
        }
    }

    fn start(&mut self) -> PyResult<()> {
        info!("[{}] FMU Driver start.", self.fmu_id);
        let middleware = Arc::clone(&self.middleware);
        let runtime = Arc::clone(&self.runtime);

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
                    let fmu_id = self.fmu_id.clone();
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
        self.fmu_driver_thread_tx_stop = Some(tx_stop);

        let thread_builder =
            std::thread::Builder::new().name(format!("fmu_driver [{}]", self.fmu_id));

        let fmu_id = self.fmu_id.clone();
        self.fmu_driver_thread_handle = Some(
            thread_builder
                .spawn(move || {
                    runtime.block_on(FmuDriverRust::fmu_driver_main(
                        rx_stop,
                        rx_clock_msg,
                        rx_orchestrator_msg,
                        middleware,
                        &fmu_id,
                    ))
                })
                .expect("Unable to spawn FMU Driver thread"),
        );

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
        while running {
            // Check for a received orchestrator command message to process
            match rx_orchestrator_msg.try_recv() {
                Ok((payload, metadata)) => {
                    let msg_json = payload.get_data().expect("Unable to get JsonData payload.");
                    let command = msg_json
                        .get("command")
                        .expect("Unable to get 'command field from JSON.");
                    info!(
                        "[{}] FMU Driver thread processing topic: {} command {}",
                        fmu_id, metadata.topic, command
                    );

                    if command == "start" {
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
                    } else if command == "stop" {
                        running = false;
                    }
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
