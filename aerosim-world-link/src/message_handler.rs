use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::thread;
use std::vec;
use std::{cmp::Ordering, collections::BinaryHeap};

use log::{error, info, warn};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::mpsc::error::TryRecvError;

const SUBSCRIBE_CHANNEL_BUFFER_SIZE: usize = 16;

use aerosim_data::{
    middleware::{
        BincodeSerializer, Metadata, Middleware, MiddlewareEnum, MiddlewareRaw, MiddlewareRegistry,
        Serializer, SerializerEnum,
    },
    types::{CompressedImage, Image, JsonData, TypeRegistry},
    AerosimMessage,
};

#[derive(Deserialize)]
struct RendererConfig {
    #[serde(rename = "renderer_id")]
    instance_id: String,
    #[allow(dead_code)]
    role: String,
    sensors: Vec<String>,
}

// -------------------------------------------------------------------------
// Payload and PayloadQueue

#[derive(Default, Clone, PartialEq)]
pub struct Payload {
    pub timestamp: f64,
    pub raw_payload: String,
}

impl Eq for Payload {}

impl PartialOrd for Payload {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // Reverse ordering for smallest timestamp first
        other.timestamp.partial_cmp(&self.timestamp)
    }
}

impl Ord for Payload {
    fn cmp(&self, other: &Payload) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

#[derive(Default)]
pub struct PayloadQueue {
    pub queue: BinaryHeap<Payload>,
    pub oldest_timestamp: Option<f64>,
    pub newest_timestamp: Option<f64>,
}

impl PayloadQueue {
    pub fn new() -> Self {
        PayloadQueue {
            queue: BinaryHeap::new(),
            oldest_timestamp: None,
            newest_timestamp: None,
        }
    }

    pub fn push(&mut self, payload: Payload) {
        match self.oldest_timestamp {
            Some(oldest_timestamp) => {
                if payload.timestamp < oldest_timestamp {
                    self.oldest_timestamp = Some(payload.timestamp);
                }
            }
            None => {
                self.oldest_timestamp = Some(payload.timestamp);
            }
        }

        match self.newest_timestamp {
            Some(newest_timestamp) => {
                if payload.timestamp > newest_timestamp {
                    self.newest_timestamp = Some(payload.timestamp);
                }
            }
            None => {
                self.newest_timestamp = Some(payload.timestamp);
            }
        }

        self.queue.push(payload);
    }
}

// -------------------------------------------------------------------------
// MessageHandler

// #[derive(Default)]
pub struct MessageHandler {
    renderer_id: String, // Instance ID set by the renderer that creates the MessageHandler
    _sim_config: serde_json::Value,
    runtime: Arc<tokio::runtime::Runtime>,
    transport: Arc<MiddlewareEnum>,
    payload_queue: Arc<Mutex<PayloadQueue>>,
    assigned_sensors: Arc<Mutex<HashSet<String>>>,

    // Handle for the thread running the async message publishing pipeline
    // Used to manage the lifecycle of the thread
    thread_handle: Option<thread::JoinHandle<()>>,

    // Channel used to signal the thread to stop execution
    tx_stop: Arc<tokio::sync::mpsc::Sender<bool>>,
    rx_stop: Option<tokio::sync::mpsc::Receiver<bool>>,

    // Channel for sending images through an asynchronous processing pipeline
    tx_img: Arc<tokio::sync::mpsc::Sender<(String, Image)>>,
    rx_img: Option<tokio::sync::mpsc::Receiver<(String, Image)>>,

    // Channel for requesting new topic subscriptions from the async runtime.
    // Sends (topic, oneshot reply) so the caller blocks until the subscription is processed.
    tx_sub: Arc<tokio::sync::mpsc::Sender<(String, tokio::sync::oneshot::Sender<bool>)>>,
    rx_sub: Option<tokio::sync::mpsc::Receiver<(String, tokio::sync::oneshot::Sender<bool>)>>,
}

impl MessageHandler {
    pub fn new(renderer_id: &str, middleware_type: &str) -> Self {
        info!("[aerosim.renderer.message_handler] Creating a new MessageHandler.");

        let (tx_stop, rx_stop) = tokio::sync::mpsc::channel::<bool>(1);
        let (tx_img, rx_img) = tokio::sync::mpsc::channel::<(String, Image)>(1);
        let (tx_sub, rx_sub) = tokio::sync::mpsc::channel::<(
            String,
            tokio::sync::oneshot::Sender<bool>,
        )>(SUBSCRIBE_CHANNEL_BUFFER_SIZE);

        let transport = match middleware_type {
            "kafka" => MiddlewareRegistry::new()
                .get("kafka")
                .expect("Failed to get Kafka middleware."),
            "zenoh" => MiddlewareRegistry::new()
                .get("zenoh")
                .expect("Failed to get Zenoh middleware."),
            _ => {
                warn!("[aerosim.renderer.message_handler] Invalid middleware type: {}. Using 'zenoh' as default.", middleware_type);
                MiddlewareRegistry::new()
                    .get("zenoh")
                    .expect("Failed to get Zenoh middleware.")
            }
        };

        MessageHandler {
            renderer_id: renderer_id.to_string(),
            _sim_config: serde_json::Value::Null,
            runtime: Arc::new(tokio::runtime::Runtime::new().unwrap()),
            transport: transport,
            payload_queue: Arc::new(Mutex::new(PayloadQueue::new())),
            assigned_sensors: Arc::new(Mutex::new(HashSet::new())),
            thread_handle: None,
            tx_stop: Arc::new(tx_stop),
            rx_stop: Some(rx_stop),
            tx_img: Arc::new(tx_img),
            rx_img: Some(rx_img),
            tx_sub: Arc::new(tx_sub),
            rx_sub: Some(rx_sub),
        }
    }

    pub fn start(&mut self) -> Result<(), ()> {
        info!("[aerosim.renderer.message_handler] Starting message handler.");

        let runtime = Arc::clone(&self.runtime);
        let transport = Arc::clone(&self.transport);
        let payload_queue = Arc::clone(&self.payload_queue);
        let assigned_sensors = Arc::clone(&self.assigned_sensors);
        let instance_id = self.renderer_id.clone();
        let rx_img = self.rx_img.take().unwrap();
        let rx_stop = self.rx_stop.take().unwrap();
        let rx_sub = self.rx_sub.take().unwrap();
        self.thread_handle = Some(thread::spawn(move || {
            runtime.block_on(message_handler_main(
                transport,
                payload_queue,
                assigned_sensors,
                instance_id,
                rx_img,
                rx_stop,
                rx_sub,
            ));
        }));

        info!("[aerosim.renderer.message_handler] Message handler started.");
        Ok(())
    }

    pub fn notify_scene_graph_loaded(&self) {
        info!("[aerosim.renderer.message_handler] Notifying that the scene graph has been loaded.");
        let payload = json!({
            "renderer_id": format!("{}", self.renderer_id).as_str(),
            "status": "scene_graph_loaded",
        });
        let payload_jsondata = JsonData::new(payload);
        futures::executor::block_on(self.transport.publish(
            "aerosim.renderer.status",
            &payload_jsondata,
            None,
        ))
        .ok();
    }

    pub fn stop(&mut self) -> Result<(), ()> {
        info!("[aerosim.renderer.message_handler] Stopping message handler.");

        let _ = self.tx_stop.blocking_send(true);
        let handle = self
            .thread_handle
            .take()
            .expect("No message thread handle, was it started?");

        if let Err(e) = handle.join() {
            error!(
                "[aerosim.renderer.message_handler] Message thread panicked during shutdown: {:?}",
                e
            );
        }

        self.transport.shutdown();

        info!("[aerosim.renderer.message_handler] Message handler stopped.");
        Ok(())
    }

    /// Publish a JSON payload as a `JsonData` message to the given topic.
    /// The payload is always sent as the generic `JsonData` type regardless of content.
    pub fn publish_to_topic(&self, topic: &str, payload: &str) -> bool {
        let payload_json = serde_json::from_str::<serde_json::Value>(payload)
            .expect("Error serializing payload string to JSON.");
        let payload_jsondata = JsonData::new(payload_json);
        futures::executor::block_on(self.transport.publish(topic, &payload_jsondata, None)).is_ok()
    }

    /// Publish a JSON payload as a specific registered message type to the given topic.
    ///
    /// Unlike `publish_to_topic` which always wraps data as `JsonData`, this method uses
    /// the `TypeRegistry` to look up the `message_type` by name (e.g. "VehicleState",
    /// "EffectorState") and serializes the JSON payload into the correct wire format for
    /// that type. This ensures subscribers receive a properly typed message with matching
    /// metadata.
    ///
    /// - `message_type`: Name of a type registered in the `TypeRegistry`.
    /// - `payload`: JSON string matching the schema of the given message type.
    /// - `timestamp_sim`: Optional simulation timestamp in seconds. Pass `None` to omit.
    ///
    /// Returns `true` on success, `false` if parsing, type lookup, or serialization fails.
    pub fn publish_typed_to_topic(
        &self,
        topic: &str,
        message_type: &str,
        payload: &str,
        timestamp_sim: Option<f64>,
    ) -> bool {
        let data: serde_json::Value = match serde_json::from_str(payload) {
            Ok(v) => v,
            Err(e) => {
                warn!(
                    "[aerosim.renderer.message_handler] Failed to parse JSON payload for topic {}: {:?}",
                    topic, e
                );
                return false;
            }
        };

        let Some(typesupport) = TypeRegistry::new().get(message_type) else {
            warn!(
                "[aerosim.renderer.message_handler] Unknown message type '{}' for topic: {}",
                message_type, topic
            );
            return false;
        };

        let sim_ts = timestamp_sim.map(|t| {
            let sec = t as i32;
            let nanosec = ((t - t.floor()) * 1_000_000_000.0) as u32;
            aerosim_data::types::TimeStamp { sec, nanosec }
        });

        let serializer = self.transport.get_serializer();
        let metadata = Metadata::new(
            topic,
            message_type,
            sim_ts,
            Some(aerosim_data::types::TimeStamp::now()),
        );

        let Some(serialized) = typesupport.from_json(&serializer, &metadata, data) else {
            warn!(
                "[aerosim.renderer.message_handler] Failed to serialize type '{}' for topic: {}",
                message_type, topic
            );
            return false;
        };

        futures::executor::block_on(self.transport.publish_raw(message_type, topic, &serialized))
            .ok();
        true
    }

    pub fn publish_image_to_topic_async(&self, topic: &str, image: Image) {
        let _ = self.tx_img.try_send((topic.to_string(), image));
    }

    pub fn subscribe_to_topic(&self, topic: &str) -> bool {
        info!(
            "[aerosim.renderer.message_handler] Requesting subscription to topic: {}",
            topic
        );
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel::<bool>();
        if self.tx_sub.try_send((topic.to_string(), reply_tx)).is_err() {
            warn!(
                "[aerosim.renderer.message_handler] Failed to send subscription request for topic: {}",
                topic
            );
            return false;
        }
        // Block until the async runtime processes the subscription
        match reply_rx.blocking_recv() {
            Ok(return_status) => return_status,
            Err(_) => {
                warn!(
                    "[aerosim.renderer.message_handler] Subscription reply channel dropped for topic: {}",
                    topic
                );
                false
            }
        }
    }

    pub fn get_payload_queue_size(&self) -> u32 {
        let q_size = self.payload_queue.lock().unwrap().queue.len();
        q_size as u32
    }

    pub fn get_payload_queue_oldest_timestamp(&self) -> f64 {
        let oldest_timestamp = self.payload_queue.lock().unwrap().oldest_timestamp;
        oldest_timestamp.unwrap_or(-1.0)
    }

    pub fn get_payload_queue_newest_timestamp(&self) -> f64 {
        let oldest_timestamp = self.payload_queue.lock().unwrap().newest_timestamp;
        oldest_timestamp.unwrap_or(-1.0)
    }

    pub fn get_payload_from_queue(&self) -> Option<String> {
        let payload_q = &mut self.payload_queue.lock().unwrap();
        if payload_q.queue.len() > 0 {
            // Get the oldest payload from the queue
            let payload = payload_q.queue.pop().unwrap().raw_payload;

            // If there are still more payloads in the queue, update the oldest timestamp,
            // otherwise clear the oldest and newest timestamps
            if payload_q.queue.len() > 0 {
                let next_timestamp = payload_q.queue.peek().unwrap().timestamp;
                payload_q.oldest_timestamp = Some(next_timestamp);
            } else {
                payload_q.oldest_timestamp = None;
                payload_q.newest_timestamp = None;
            }

            return Some(payload);
        }
        None
    }
}

// -------------------------------------------------------------------------
// Message Handler main function run from start()

async fn message_handler_main(
    transport: Arc<MiddlewareEnum>,
    payload_queue: Arc<Mutex<PayloadQueue>>,
    assigned_sensors: Arc<Mutex<HashSet<String>>>,
    renderer_id: String,
    mut rx_img: tokio::sync::mpsc::Receiver<(String, Image)>,
    mut rx_stop: tokio::sync::mpsc::Receiver<bool>,
    mut rx_sub: tokio::sync::mpsc::Receiver<(String, tokio::sync::oneshot::Sender<bool>)>,
) {
    {
        match transport
            .subscribe::<JsonData>("aerosim.orchestrator.commands", {
                let transport = Arc::clone(&transport);
                let payload_queue = Arc::clone(&payload_queue);
                let assigned_sensors = Arc::clone(&assigned_sensors);
                let instance_id = renderer_id.clone();
                Box::new(move |data, metadata| {
                    handle_orchestrator_command_message(
                        &data,
                        &metadata,
                        &instance_id,
                        &assigned_sensors,
                        &payload_queue,
                        &transport,
                    );
                    Ok(())
                })
            })
            .await
        {
            Ok(()) => {
                println!("[aerosim.world.link] Created aerosim.orchestrator.commands subscriber.")
            }
            Err(_) => eprintln!(
                "[aerosim.world.link] Could not create aerosim.orchestrator.commands subscriber."
            ),
        }

        match transport
            .subscribe::<JsonData>("aerosim.scene_graph.update", {
                let payload_queue = Arc::clone(&payload_queue);
                let assigned_sensors = Arc::clone(&assigned_sensors);
                let instance_id = renderer_id.clone();
                Box::new(move |data, metadata| {
                    // Process scene graph update message
                    handle_scene_graph_update_message(
                        &data,
                        &metadata,
                        &assigned_sensors,
                        &instance_id,
                        &payload_queue,
                    );
                    Ok(())
                })
            })
            .await
        {
            Ok(()) => {
                println!("[aerosim.world.link] Created aerosim.scene_graph.update subscriber.")
            }
            Err(_) => eprintln!(
                "[aerosim.world.link] Could not create aerosim.scene_graph.update subscriber."
            ),
        }
    }

    // Process image publishing, subscription requests, and stop signals asynchronously.
    // The image channel buffer between the renderer thread and this thread is set to 1.
    // If the buffer already contains an image and the renderer tries to add a new one,
    // the new image will be discarded (i.e., image loss is possible).
    loop {
        // Process any pending subscription requests
        while let Ok((topic, reply_tx)) = rx_sub.try_recv() {
            info!(
                "[aerosim.world.link] Creating subscription for topic: {}",
                topic
            );
            let serializer = transport.get_serializer();
            let subscribe_result = transport
                // message_type parameter ("") is unused by both Kafka and Zenoh subscribe_raw implementations;
                // the actual type is resolved from metadata inside the raw payload callback.
                .subscribe_raw("", &topic, {
                    let payload_queue = Arc::clone(&payload_queue);
                    let topic = topic.clone();
                    Box::new(move |raw_payload: &[u8]| {
                        handle_raw_topic_message(raw_payload, &serializer, &topic, &payload_queue);
                        Ok(())
                    })
                })
                .await;
            let return_status = match subscribe_result {
                Ok(()) => {
                    info!(
                        "[aerosim.world.link] Created subscriber for topic: {}",
                        topic
                    );
                    true
                }
                Err(e) => {
                    warn!(
                        "[aerosim.world.link] Could not create subscriber for topic {}: {:?}",
                        topic, e
                    );
                    false
                }
            };
            let _ = reply_tx.send(return_status);
        }

        match rx_img.try_recv() {
            Ok((topic, image)) => {
                // TODO: Properly pass `timestamp_sim` and `timestamp_platform` from the renderer.
                let metadata = Metadata::new(&topic, &CompressedImage::get_type_name(), None, None);
                match image.compress() {
                    Ok(compressed_image) => {
                        // The Kafka middleware defaults to a JSON serializer. A Bincode serializer is used
                        // here to improve encoding and decoding performance, handled through the middleware's raw API.
                        let serializer = BincodeSerializer {};
                        match serializer.serialize_message(&metadata, &compressed_image) {
                            Some(payload) => {
                                let _ = transport
                                    .publish_raw(
                                        &CompressedImage::get_type_name(),
                                        &topic,
                                        &payload,
                                    )
                                    .await;
                            }
                            None => eprintln!("Could not serialize image using bincode"),
                        }
                    }
                    Err(_) => eprintln!("Could not compress raw image to jpeg"),
                }
            }
            Err(TryRecvError::Empty) => { /* pass to continue looping */ }
            Err(e) => eprintln!("Error receiving image: {:?}", e),
        }

        match rx_stop.try_recv() {
            Ok(stop) => {
                if stop {
                    break;
                }
            }
            Err(_) => {}
        }
    }
}

// Function to extract transform parameters from JSON and send to renderer
fn handle_orchestrator_command_message(
    payload: &JsonData,
    metadata: &Metadata,
    instance_id: &str,
    assigned_sensors: &Arc<Mutex<HashSet<String>>>,
    payload_queue: &Arc<Mutex<PayloadQueue>>,
    transport: &Arc<MiddlewareEnum>,
) {
    // Parse JSON message from the payload
    let Some(msg_data) = payload.get_data() else {
        println!("Failed to parse message JSON: {:?}", payload);
        return;
    };

    let command_str = msg_data["command"]
        .as_str()
        .expect("Error parsing 'command' data");

    println!(
        "[aerosim.world.link] Received orchestrator command: {}",
        command_str
    );

    if command_str == "load_config" {
        // --------------------------------------------------------------
        // Process orchestrator load config command
        // --------------------------------------------------------------

        // Check 'renderers' config for this renderer's instance ID
        let sim_config = &msg_data["parameters"]["sim_config"];

        if let Some(renderer_configs_json) = sim_config.get("renderers") {
            let renderer_configs: Vec<RendererConfig> =
                match serde_json::from_value(renderer_configs_json.clone()) {
                    Ok(renderer_configs) => renderer_configs,
                    Err(e) => {
                        warn!(
                            "[aerosim.world.link] Failed to parse renderer configurations: {:?}",
                            e
                        );
                        vec![]
                    }
                };

            for renderer_config in renderer_configs {
                if renderer_config.instance_id == instance_id {
                    info!(
                        "[aerosim.world.link] Renderer configurations found for renderer ID: {}",
                        instance_id
                    );
                    let mut assigned_sensors_lock = assigned_sensors
                        .lock()
                        .expect("Failed to lock assigned_sensors");
                    *assigned_sensors_lock = HashSet::from_iter(renderer_config.sensors);
                    if assigned_sensors_lock.is_empty() {
                        info!("[aerosim.world.link] No sensors assigned to renderer.");
                    } else {
                        info!(
                            "[aerosim.world.link] Assigned sensors: {:?}",
                            assigned_sensors_lock
                        );
                    }
                }
            }

            info!(
                "[aerosim.world.link] Broadcasting availability of renderer with Instance ID: {}.",
                instance_id
            );
            let announcement = json!({
                "renderer_id": format!("{}", instance_id).as_str(),
                "status": "config_loaded",
            });
            let payload = JsonData::new(announcement);

            futures::executor::block_on(transport.publish(
                "aerosim.renderer.status",
                &payload,
                Some(metadata.timestamp_sim),
            ))
            .ok();
        } else {
            warn!("[aerosim.world.link] Failed to find renderer configurations in message.");

            info!(
                "[aerosim.world.link] Broadcasting availability of renderer with Instance ID: {}.",
                instance_id
            );
            let announcement = json!({
                "renderer_id": format!("{}", instance_id).as_str(),
                "status": "config_error",
            });
            let payload = JsonData::new(announcement);

            futures::executor::block_on(transport.publish(
                "aerosim.renderer.status",
                &payload,
                Some(metadata.timestamp_sim),
            ))
            .ok();
        }
    } else if command_str == "load_scene_graph" {
        info!(
            "[aerosim.world.link] Processing load_scene_graph command for renderer with Instance ID: {}.",
            instance_id
        );

        let scene_graph = &msg_data["parameters"]["scene_graph"];

        // TODO: Filter out data for non-assigned sensors
        let assigned_sensors_vec = assigned_sensors
            .lock()
            .expect("Failed to lock assigned_sensors")
            .iter()
            .cloned()
            .collect::<Vec<String>>();
        let filtered_scene_graph =
            filter_scene_graph_data(&scene_graph, assigned_sensors_vec, instance_id);

        // Set payload timestamp as the message's platform timestamp since it
        // is used for real-time pacing of the renderer.
        let payload_timestamp: f64 = metadata.timestamp_platform.sec as f64
            + metadata.timestamp_platform.nanosec as f64 / 1_000_000_000.0;

        let mut payload_queue_lock = payload_queue.lock().unwrap();
        payload_queue_lock.push(Payload {
            timestamp: payload_timestamp,
            raw_payload: serde_json::to_string(&filtered_scene_graph)
                .expect("Error serializing sene_graph message to JSON."),
        });
    } else if command_str == "stop" {
        info!(
            "[aerosim.world.link] Processing stop command for renderer with Instance ID: {}.",
            instance_id
        );

        // Forward the stop command to the payload queue so the renderer can detect it
        let payload_timestamp: f64 = metadata.timestamp_platform.sec as f64
            + metadata.timestamp_platform.nanosec as f64 / 1_000_000_000.0;

        let mut payload_queue_lock = payload_queue.lock().unwrap();
        payload_queue_lock.push(Payload {
            timestamp: payload_timestamp,
            raw_payload: serde_json::to_string(&msg_data)
                .expect("Error serializing stop command to JSON."),
        });
    }
}

fn filter_scene_graph_data(
    scene_graph: &serde_json::Value,
    _assigned_sensors: Vec<String>,
    instance_id: &str,
) -> serde_json::Value {
    let mut filtered_scene_graph = scene_graph.clone();

    // Filter viewport_configs in resources: pick a single matching viewport_config.
    // If no matching config exists, remove the viewport configuration key.
    if let Some(resources) = filtered_scene_graph.get_mut("resources") {
        if let Some(viewport_configs) = resources.get_mut("viewport_configs") {
            if let Some(vp_array) = viewport_configs.as_array_mut() {
                // Retain only viewport configs for the provided instance_id.
                let matching_configs: Vec<serde_json::Value> = vp_array
                    .iter()
                    .filter(|vp| {
                        vp.get("renderer_instance")
                            .and_then(|v| v.as_str())
                            .map(|id| id == instance_id)
                            .unwrap_or(false)
                    })
                    .cloned()
                    .collect();

                // If a matching config exists, replace the array with a single object.
                if let Some(selected_config) = matching_configs.into_iter().next() {
                    if let Some(resources_obj) = resources.as_object_mut() {
                        resources_obj.remove("viewport_configs");
                        resources_obj.insert("viewport_config".to_string(), selected_config);
                    }
                } else if let Some(resources_obj) = resources.as_object_mut() {
                    // No matching viewport configuration exists: remove the key entirely.
                    resources_obj.remove("viewport_configs");
                }
            }
        }
    }

    // TODO: Filter out entities based on assigned sensors.

    filtered_scene_graph
}

fn handle_raw_topic_message(
    raw_payload: &[u8],
    serializer: &SerializerEnum,
    topic: &str,
    payload_queue: &Arc<Mutex<PayloadQueue>>,
) {
    // Deserialize metadata first to get the message type and timestamps
    let Some(metadata) = serializer.deserialize_metadata(raw_payload) else {
        warn!(
            "[aerosim.world.link] Failed to deserialize metadata on topic: {}",
            topic
        );
        return;
    };

    // Use the TypeRegistry to deserialize the data to JSON based on the message type
    let msg_data: serde_json::Value =
        if let Some(typesupport) = TypeRegistry::new().get(&metadata.type_name) {
            match typesupport.to_json(serializer, raw_payload) {
                Some(json_val) => json_val,
                None => {
                    warn!(
                    "[aerosim.world.link] Failed to deserialize data for type '{}' on topic: {}",
                    metadata.type_name, topic
                );
                    return;
                }
            }
        } else {
            warn!(
                "[aerosim.world.link] Unknown message type '{}' on topic: {}",
                metadata.type_name, topic
            );
            return;
        };

    let payload_timestamp: f64 = metadata.timestamp_platform.sec as f64
        + metadata.timestamp_platform.nanosec as f64 / 1_000_000_000.0;

    let timestamp_sim_f64: f64 =
        metadata.timestamp_sim.sec as f64 + metadata.timestamp_sim.nanosec as f64 / 1_000_000_000.0;

    // Wrap the data with topic and type metadata so the consumer can distinguish messages
    let envelope = json!({
        "topic": topic,
        "message_type": metadata.type_name,
        "timestamp_sim": {
            "sec": metadata.timestamp_sim.sec,
            "nanosec": metadata.timestamp_sim.nanosec,
        },
        "timestamp_sim_f64": timestamp_sim_f64,
        "data": msg_data,
    });

    let mut payload_queue_lock = payload_queue.lock().unwrap();
    payload_queue_lock.push(Payload {
        timestamp: payload_timestamp,
        raw_payload: serde_json::to_string(&envelope)
            .expect("Error serializing raw topic message to JSON."),
    });
}

fn handle_scene_graph_update_message(
    payload: &JsonData,
    metadata: &Metadata,
    assigned_sensors: &Arc<Mutex<HashSet<String>>>,
    instance_id: &str,
    payload_queue: &Arc<Mutex<PayloadQueue>>,
) {
    // Parse JSON message from the payload
    let Some(msg_data) = payload.get_data() else {
        println!("Failed to parse message JSON: {:?}", payload);
        return;
    };

    // --------------------------------------------------------------
    // TODO: Filter out data for non-assigned sensors
    // --------------------------------------------------------------
    let assigned_sensors_vec = assigned_sensors
        .lock()
        .expect("Failed to lock assigned_sensors")
        .iter()
        .cloned()
        .collect::<Vec<String>>();

    let filtered_scene_graph =
        filter_scene_graph_data(&msg_data, assigned_sensors_vec, instance_id);
    {
        // Set payload timestamp as the message's platform timestamp since it
        // is used for real-time pacing of the renderer.
        let payload_timestamp: f64 = metadata.timestamp_platform.sec as f64
            + metadata.timestamp_platform.nanosec as f64 / 1_000_000_000.0;

        let mut payload_queue_lock = payload_queue.lock().unwrap();
        payload_queue_lock.push(Payload {
            timestamp: payload_timestamp,
            raw_payload: serde_json::to_string(&filtered_scene_graph)
                .expect("Error serializing renderer message to JSON."),
        });
    } // unlock payload_queue
}
