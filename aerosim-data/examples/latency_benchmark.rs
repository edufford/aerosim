//! Middleware Publish-Subscribe Latency Benchmark
//!
//! This example benchmarks round-trip latency between two simulated nodes using middleware.
//! Node A publishes messages with timestamps to a forward topic, Node B receives them and echoes
//! back to a return topic, and Node A measures the round-trip latency.
//!
//! Run with:
//!   cargo run --example latency_benchmark --features zenoh -- zenoh
//!   cargo run --example latency_benchmark --features kafka -- kafka
//!   cargo run --example latency_benchmark --features default -- zenoh  # or kafka

use std::env;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::json;
use tokio::sync::mpsc;

use aerosim_data::middleware::{Metadata, Middleware, MiddlewareRaw, MiddlewareRegistry, Serializer};
use aerosim_data::types::JsonData;

/// Latency measurement result (in milliseconds)
#[derive(Debug, Clone)]
struct LatencyMeasurement {
    forward_latency_ms: f64,
    return_latency_ms: f64,
    roundtrip_latency_ms: f64,
}

/// Detailed latency measurement with serialization breakdown (in milliseconds)
#[derive(Debug, Clone)]
struct DetailedLatencyMeasurement {
    // Forward leg breakdown
    forward_serialize_ms: f64,
    forward_transport_ms: f64,
    forward_deserialize_ms: f64,
    forward_total_ms: f64,
    // Return leg breakdown
    return_serialize_ms: f64,
    return_transport_ms: f64,
    return_deserialize_ms: f64,
    return_total_ms: f64,
    // Round-trip total
    roundtrip_total_ms: f64,
}

/// Statistics calculator
struct Stats {
    values: Vec<f64>,
}

impl Stats {
    fn new(values: Vec<f64>) -> Self {
        Self { values }
    }

    fn mean(&self) -> f64 {
        if self.values.is_empty() {
            return 0.0;
        }
        self.values.iter().sum::<f64>() / self.values.len() as f64
    }

    fn median(&self) -> f64 {
        if self.values.is_empty() {
            return 0.0;
        }
        let mut sorted = self.values.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mid = sorted.len() / 2;
        if sorted.len() % 2 == 0 {
            (sorted[mid - 1] + sorted[mid]) / 2.0
        } else {
            sorted[mid]
        }
    }

    fn min(&self) -> f64 {
        self.values.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    fn max(&self) -> f64 {
        self.values
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max)
    }

    fn std_dev(&self) -> f64 {
        if self.values.len() < 2 {
            return 0.0;
        }
        let mean = self.mean();
        let variance = self.values.iter().map(|x| (x - mean).powi(2)).sum::<f64>()
            / (self.values.len() - 1) as f64;
        variance.sqrt()
    }
}

fn print_stats(title: &str, values: &[f64], unit: &str) {
    let stats = Stats::new(values.to_vec());
    println!("\n{}:", title);
    println!("  Samples: {}", values.len());
    println!("  Average: {:.3} {}", stats.mean(), unit);
    println!("  Median:  {:.3} {}", stats.median(), unit);
    println!("  Min:     {:.3} {}", stats.min(), unit);
    println!("  Max:     {:.3} {}", stats.max(), unit);
    if values.len() > 1 {
        println!("  Std Dev: {:.3} {}", stats.std_dev(), unit);
    }
}

fn print_stats_inline(title: &str, values: &[f64], indent: &str) {
    if values.is_empty() {
        return;
    }
    let stats = Stats::new(values.to_vec());
    println!("{}{}:", indent, title);
    println!("{}  Average: {:.3} ms", indent, stats.mean());
    println!("{}  Median:  {:.3} ms", indent, stats.median());
    println!("{}  Min:     {:.3} ms", indent, stats.min());
    println!("{}  Max:     {:.3} ms", indent, stats.max());
}

fn print_benchmark_results(title: &str, measurements: &[LatencyMeasurement]) {
    println!("\n{}", "=".repeat(60));
    println!("{}", title);
    println!("{}", "=".repeat(60));

    let forward: Vec<f64> = measurements.iter().map(|m| m.forward_latency_ms).collect();
    let return_: Vec<f64> = measurements.iter().map(|m| m.return_latency_ms).collect();
    let roundtrip: Vec<f64> = measurements.iter().map(|m| m.roundtrip_latency_ms).collect();

    print_stats("Forward Latency (Node A -> Node B)", &forward, "ms");
    print_stats("Return Latency (Node B -> Node A)", &return_, "ms");
    print_stats(
        "Round-Trip Latency (Node A -> Node B -> Node A)",
        &roundtrip,
        "ms",
    );

    println!("\n{}", "=".repeat(60));
}

fn print_detailed_benchmark_results(title: &str, measurements: &[DetailedLatencyMeasurement]) {
    println!("\n{}", "=".repeat(70));
    println!("{}", title);
    println!("{}", "=".repeat(70));
    println!("Samples: {}", measurements.len());

    if measurements.is_empty() {
        println!("No measurements collected");
        println!("{}", "=".repeat(70));
        return;
    }

    // Forward leg
    let forward_serialize: Vec<f64> = measurements.iter().map(|m| m.forward_serialize_ms).collect();
    let forward_transport: Vec<f64> = measurements.iter().map(|m| m.forward_transport_ms).collect();
    let forward_deserialize: Vec<f64> = measurements.iter().map(|m| m.forward_deserialize_ms).collect();
    let forward_total: Vec<f64> = measurements.iter().map(|m| m.forward_total_ms).collect();

    println!(
        "\nForward Leg (Node A -> Node B): {:.3} ms avg",
        Stats::new(forward_total.clone()).mean()
    );
    print_stats_inline("Serialize (Node A)", &forward_serialize, "  ");
    print_stats_inline("Transport", &forward_transport, "  ");
    print_stats_inline("Deserialize (Node B)", &forward_deserialize, "  ");

    // Return leg
    let return_serialize: Vec<f64> = measurements.iter().map(|m| m.return_serialize_ms).collect();
    let return_transport: Vec<f64> = measurements.iter().map(|m| m.return_transport_ms).collect();
    let return_deserialize: Vec<f64> = measurements.iter().map(|m| m.return_deserialize_ms).collect();
    let return_total: Vec<f64> = measurements.iter().map(|m| m.return_total_ms).collect();

    println!(
        "\nReturn Leg (Node B -> Node A): {:.3} ms avg",
        Stats::new(return_total.clone()).mean()
    );
    print_stats_inline("Serialize (Node B)", &return_serialize, "  ");
    print_stats_inline("Transport", &return_transport, "  ");
    print_stats_inline("Deserialize (Node A)", &return_deserialize, "  ");

    // Round-trip summary
    let roundtrip_total: Vec<f64> = measurements.iter().map(|m| m.roundtrip_total_ms).collect();
    println!(
        "\nRound-Trip Total: {:.3} ms avg",
        Stats::new(roundtrip_total).mean()
    );

    // Breakdown summary
    let total_serialize = Stats::new(forward_serialize.clone()).mean() + Stats::new(return_serialize.clone()).mean();
    let total_transport = Stats::new(forward_transport.clone()).mean() + Stats::new(return_transport.clone()).mean();
    let total_deserialize = Stats::new(forward_deserialize.clone()).mean() + Stats::new(return_deserialize.clone()).mean();
    let total_time = total_serialize + total_transport + total_deserialize;

    if total_time > 0.0 {
        println!("\n  Breakdown (averages):");
        println!(
            "    Total Serialize:   {:.3} ms ({:.1}%)",
            total_serialize,
            100.0 * total_serialize / total_time
        );
        println!(
            "    Total Transport:   {:.3} ms ({:.1}%)",
            total_transport,
            100.0 * total_transport / total_time
        );
        println!(
            "    Total Deserialize: {:.3} ms ({:.1}%)",
            total_deserialize,
            100.0 * total_deserialize / total_time
        );
    }

    println!("\n{}", "=".repeat(70));
}

/// Message passed through channel from forward subscriber to echo publisher
#[derive(Debug)]
struct EchoRequest {
    msg_id: u64,
    original_send_time_nanos: u64,
    node_b_receive_time_nanos: u64,
}

/// Detailed echo request with serialization timing
#[derive(Debug)]
struct DetailedEchoRequest {
    msg_id: u64,
    original_send_time_nanos: u64,
    send_after_serialize_nanos: u64,
    node_b_receive_time_nanos: u64,
    node_b_deserialize_done_nanos: u64,
}

async fn run_benchmark(
    transport_name: &str,
    num_messages: usize,
    warmup_messages: usize,
    payload_size: usize,
    topic_prefix: &str,
) -> Vec<LatencyMeasurement> {
    let registry = MiddlewareRegistry::new();
    let middleware = registry
        .get(transport_name)
        .unwrap_or_else(|| panic!("{} middleware not available", transport_name));

    // Use dash separator for topics (works with both Zenoh and Kafka)
    let forward_topic = format!("{}-forward", topic_prefix);
    let return_topic = format!("{}-return", topic_prefix);

    // Reference time for consistent nanosecond measurements
    let reference_time = Instant::now();

    // Channels for communication
    let (echo_tx, mut echo_rx) = mpsc::channel::<EchoRequest>(1024);
    let (measurement_tx, mut measurement_rx) = mpsc::channel::<LatencyMeasurement>(1024);

    // Counters
    let received_count = Arc::new(AtomicUsize::new(0));
    let total_expected = num_messages + warmup_messages;
    let done = Arc::new(AtomicBool::new(false));

    // Clone reference time for callbacks
    let ref_time_b = reference_time;

    // Node B callback: receives forward message, sends echo request through channel
    middleware
        .subscribe(
            &forward_topic,
            Box::new(move |data: JsonData, _metadata: Metadata| {
                let receive_time_nanos = ref_time_b.elapsed().as_nanos() as u64;

                let json_data = data.get_data().unwrap();
                let msg_id = json_data["msg_id"].as_u64().unwrap();
                let original_send_time_nanos = json_data["send_time_nanos"].as_u64().unwrap();

                let _ = echo_tx.try_send(EchoRequest {
                    msg_id,
                    original_send_time_nanos,
                    node_b_receive_time_nanos: receive_time_nanos,
                });

                Ok(())
            }),
        )
        .await
        .expect("Failed to subscribe to forward topic");

    // Node A callback: receives echo and calculates latencies
    let ref_time_a = reference_time;
    let warmup = warmup_messages;
    let received_count_clone = received_count.clone();

    middleware
        .subscribe(
            &return_topic,
            Box::new(move |data: JsonData, _metadata: Metadata| {
                let receive_time_nanos = ref_time_a.elapsed().as_nanos() as u64;

                let json_data = data.get_data().unwrap();
                let msg_id = json_data["msg_id"].as_u64().unwrap() as usize;
                let original_send_time_nanos = json_data["original_send_time_nanos"].as_u64().unwrap();
                let node_b_receive_time_nanos = json_data["node_b_receive_time_nanos"].as_u64().unwrap();
                let node_b_send_time_nanos = json_data["node_b_send_time_nanos"].as_u64().unwrap();

                // Skip warmup messages
                if msg_id >= warmup {
                    let forward_latency_ms =
                        (node_b_receive_time_nanos - original_send_time_nanos) as f64 / 1_000_000.0;
                    let return_latency_ms =
                        (receive_time_nanos - node_b_send_time_nanos) as f64 / 1_000_000.0;
                    let roundtrip_latency_ms =
                        (receive_time_nanos - original_send_time_nanos) as f64 / 1_000_000.0;

                    let _ = measurement_tx.try_send(LatencyMeasurement {
                        forward_latency_ms,
                        return_latency_ms,
                        roundtrip_latency_ms,
                    });
                }

                received_count_clone.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }),
        )
        .await
        .expect("Failed to subscribe to return topic");

    // Allow subscriptions to establish
    tokio::time::sleep(Duration::from_millis(1000)).await;

    // Spawn echo publisher task (Node B's echo response)
    let middleware_echo = middleware.clone();
    let return_topic_echo = return_topic.clone();
    let ref_time_echo = reference_time;
    let done_clone = done.clone();

    let echo_task = tokio::spawn(async move {
        while let Some(req) = echo_rx.recv().await {
            if done_clone.load(Ordering::SeqCst) {
                break;
            }

            let node_b_send_time_nanos = ref_time_echo.elapsed().as_nanos() as u64;

            let return_msg = json!({
                "msg_id": req.msg_id,
                "original_send_time_nanos": req.original_send_time_nanos,
                "node_b_receive_time_nanos": req.node_b_receive_time_nanos,
                "node_b_send_time_nanos": node_b_send_time_nanos,
            });

            let return_data = JsonData::new(return_msg);
            let _ = middleware_echo
                .publish(&return_topic_echo, &return_data, None)
                .await;
        }
    });

    // Generate payload
    let payload = "x".repeat(payload_size);

    // Send messages
    let total_messages = num_messages + warmup_messages;
    for i in 0..total_messages {
        let send_time_nanos = reference_time.elapsed().as_nanos() as u64;

        let message = json!({
            "msg_id": i,
            "send_time_nanos": send_time_nanos,
            "payload": payload,
        });

        let data = JsonData::new(message);
        middleware
            .publish(&forward_topic, &data, None)
            .await
            .expect("Failed to publish forward message");

        // Small delay between messages
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    // Wait for all messages with timeout
    let timeout = Duration::from_secs(120);
    let start = Instant::now();
    while received_count.load(Ordering::SeqCst) < total_expected {
        if start.elapsed() > timeout {
            eprintln!(
                "Warning: Timeout waiting for messages. Received {}/{}",
                received_count.load(Ordering::SeqCst),
                total_expected
            );
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Signal done and cleanup
    done.store(true, Ordering::SeqCst);

    // Wait for echo task to finish
    let _ = tokio::time::timeout(Duration::from_secs(2), echo_task).await;

    // Collect measurements
    let mut measurements = Vec::new();
    while let Ok(m) = measurement_rx.try_recv() {
        measurements.push(m);
    }

    measurements
}

async fn run_detailed_benchmark(
    transport_name: &str,
    num_messages: usize,
    warmup_messages: usize,
    payload_size: usize,
    topic_prefix: &str,
) -> Vec<DetailedLatencyMeasurement> {
    let registry = MiddlewareRegistry::new();
    let middleware = registry
        .get(transport_name)
        .unwrap_or_else(|| panic!("{} middleware not available", transport_name));
    let serializer = middleware.get_serializer();

    // Use dash separator for topics (works with both Zenoh and Kafka)
    let forward_topic = format!("{}-forward", topic_prefix);
    let return_topic = format!("{}-return", topic_prefix);
    let type_name = "aerosim::types::JsonData";

    // Reference time for consistent nanosecond measurements
    let reference_time = Instant::now();

    // Channels for communication
    let (echo_tx, mut echo_rx) = mpsc::channel::<DetailedEchoRequest>(1024);
    let (measurement_tx, mut measurement_rx) = mpsc::channel::<DetailedLatencyMeasurement>(1024);
    let (forward_serialize_tx, forward_serialize_rx) = mpsc::channel::<f64>(1024);
    let forward_serialize_rx = Arc::new(std::sync::Mutex::new(forward_serialize_rx));

    // Counters
    let received_count = Arc::new(AtomicUsize::new(0));
    let total_expected = num_messages + warmup_messages;
    let done = Arc::new(AtomicBool::new(false));

    // Clone middleware for callbacks (get serializer inside each callback)
    let middleware_b = middleware.clone();
    let ref_time_b = reference_time;

    // Node B callback: receives forward message with detailed timing
    middleware
        .subscribe_raw(
            type_name,
            &forward_topic,
            Box::new(move |payload: &[u8]| {
                let receive_time_nanos = ref_time_b.elapsed().as_nanos() as u64;

                // Deserialize and measure time
                let serializer_b = middleware_b.get_serializer();
                let (_metadata, data): (Metadata, JsonData) = serializer_b
                    .deserialize_message(payload)
                    .expect("Failed to deserialize forward message");
                let deserialize_done_nanos = ref_time_b.elapsed().as_nanos() as u64;

                let json_data = data.get_data().unwrap();
                let msg_id = json_data["msg_id"].as_u64().unwrap();
                let original_send_time_nanos = json_data["send_time_nanos"].as_u64().unwrap();
                let send_after_serialize_nanos = json_data["send_after_serialize_nanos"].as_u64().unwrap_or(original_send_time_nanos);

                let _ = echo_tx.try_send(DetailedEchoRequest {
                    msg_id,
                    original_send_time_nanos,
                    send_after_serialize_nanos,
                    node_b_receive_time_nanos: receive_time_nanos,
                    node_b_deserialize_done_nanos: deserialize_done_nanos,
                });

                Ok(())
            }),
        )
        .await
        .expect("Failed to subscribe to forward topic");

    // Node A callback: receives echo and calculates detailed latencies
    let middleware_a = middleware.clone();
    let ref_time_a = reference_time;
    let warmup = warmup_messages;
    let received_count_clone = received_count.clone();
    let forward_serialize_rx_clone = forward_serialize_rx.clone();

    middleware
        .subscribe_raw(
            type_name,
            &return_topic,
            Box::new(move |payload: &[u8]| {
                let receive_time_nanos = ref_time_a.elapsed().as_nanos() as u64;

                // Deserialize and measure time
                let serializer_a = middleware_a.get_serializer();
                let (_metadata, data): (Metadata, JsonData) = serializer_a
                    .deserialize_message(payload)
                    .expect("Failed to deserialize return message");
                let deserialize_done_nanos = ref_time_a.elapsed().as_nanos() as u64;

                let json_data = data.get_data().unwrap();
                let msg_id = json_data["msg_id"].as_u64().unwrap() as usize;
                let original_send_time_nanos = json_data["original_send_time_nanos"].as_u64().unwrap();
                let send_after_serialize_nanos = json_data["send_after_serialize_nanos"].as_u64().unwrap();
                let node_b_receive_time_nanos = json_data["node_b_receive_time_nanos"].as_u64().unwrap();
                let node_b_deserialize_done_nanos = json_data["node_b_deserialize_done_nanos"].as_u64().unwrap();
                let node_b_serialize_time_ns = json_data["node_b_serialize_time_ns"].as_u64().unwrap();
                let node_b_send_after_serialize_nanos = json_data["node_b_send_after_serialize_nanos"].as_u64().unwrap();

                // Skip warmup messages
                if msg_id >= warmup {
                    // Forward leg breakdown
                    let forward_transport_ms = (node_b_receive_time_nanos - send_after_serialize_nanos) as f64 / 1_000_000.0;
                    let forward_deserialize_ms = (node_b_deserialize_done_nanos - node_b_receive_time_nanos) as f64 / 1_000_000.0;
                    let forward_total_ms = (node_b_deserialize_done_nanos - original_send_time_nanos) as f64 / 1_000_000.0;

                    // Return leg breakdown
                    let return_serialize_ms = node_b_serialize_time_ns as f64 / 1_000_000.0;
                    let return_transport_ms = (receive_time_nanos - node_b_send_after_serialize_nanos) as f64 / 1_000_000.0;
                    let return_deserialize_ms = (deserialize_done_nanos - receive_time_nanos) as f64 / 1_000_000.0;
                    let return_total_ms = (deserialize_done_nanos - node_b_deserialize_done_nanos) as f64 / 1_000_000.0;

                    // Round-trip total
                    let roundtrip_total_ms = (deserialize_done_nanos - original_send_time_nanos) as f64 / 1_000_000.0;

                    // Get forward serialize time from channel (stored during send)
                    let forward_serialize_ms = forward_serialize_rx_clone
                        .lock()
                        .ok()
                        .and_then(|mut rx| rx.try_recv().ok())
                        .unwrap_or(0.0);

                    let _ = measurement_tx.try_send(DetailedLatencyMeasurement {
                        forward_serialize_ms,
                        forward_transport_ms,
                        forward_deserialize_ms,
                        forward_total_ms,
                        return_serialize_ms,
                        return_transport_ms,
                        return_deserialize_ms,
                        return_total_ms,
                        roundtrip_total_ms,
                    });
                }

                received_count_clone.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }),
        )
        .await
        .expect("Failed to subscribe to return topic");

    // Allow subscriptions to establish
    tokio::time::sleep(Duration::from_millis(1000)).await;

    // Spawn echo publisher task (Node B's echo response with detailed timing)
    let middleware_echo = middleware.clone();
    let return_topic_echo = return_topic.clone();
    let type_name_echo = type_name.to_string();
    let ref_time_echo = reference_time;
    let done_clone = done.clone();

    let echo_task = tokio::spawn(async move {
        let serializer_echo = middleware_echo.get_serializer();
        while let Some(req) = echo_rx.recv().await {
            if done_clone.load(Ordering::SeqCst) {
                break;
            }

            // Measure serialization time
            let serialize_start = ref_time_echo.elapsed().as_nanos() as u64;
            let return_msg = json!({
                "msg_id": req.msg_id,
                "original_send_time_nanos": req.original_send_time_nanos,
                "send_after_serialize_nanos": req.send_after_serialize_nanos,
                "node_b_receive_time_nanos": req.node_b_receive_time_nanos,
                "node_b_deserialize_done_nanos": req.node_b_deserialize_done_nanos,
                "node_b_serialize_time_ns": 0u64, // Placeholder
                "node_b_send_after_serialize_nanos": 0u64, // Placeholder
            });
            let return_data = JsonData::new(return_msg);
            let metadata = Metadata::new(&return_topic_echo, &type_name_echo, None, None);
            let _ = serializer_echo.serialize_message(&metadata, &return_data);
            let serialize_done = ref_time_echo.elapsed().as_nanos() as u64;
            let serialize_time_ns = serialize_done - serialize_start;

            // Re-serialize with actual timing
            let return_msg = json!({
                "msg_id": req.msg_id,
                "original_send_time_nanos": req.original_send_time_nanos,
                "send_after_serialize_nanos": req.send_after_serialize_nanos,
                "node_b_receive_time_nanos": req.node_b_receive_time_nanos,
                "node_b_deserialize_done_nanos": req.node_b_deserialize_done_nanos,
                "node_b_serialize_time_ns": serialize_time_ns,
                "node_b_send_after_serialize_nanos": ref_time_echo.elapsed().as_nanos() as u64,
            });
            let return_data = JsonData::new(return_msg);
            let payload = serializer_echo.serialize_message(&metadata, &return_data).unwrap();

            let _ = middleware_echo
                .publish_raw(&type_name_echo, &return_topic_echo, &payload)
                .await;
        }
    });

    // Generate payload
    let payload_str = "x".repeat(payload_size);

    // Send messages with detailed timing
    let total_messages = num_messages + warmup_messages;
    for i in 0..total_messages {
        let send_time_nanos = reference_time.elapsed().as_nanos() as u64;

        // First serialize to measure time
        let message = json!({
            "msg_id": i,
            "send_time_nanos": send_time_nanos,
            "send_after_serialize_nanos": 0u64, // Placeholder
            "payload": payload_str,
        });
        let data = JsonData::new(message);
        let metadata = Metadata::new(&forward_topic, type_name, None, None);

        let serialize_start = reference_time.elapsed().as_nanos() as u64;
        let _ = serializer.serialize_message(&metadata, &data);
        let serialize_done = reference_time.elapsed().as_nanos() as u64;

        // Re-serialize with actual timing
        let message = json!({
            "msg_id": i,
            "send_time_nanos": send_time_nanos,
            "send_after_serialize_nanos": serialize_done,
            "payload": payload_str,
        });
        let data = JsonData::new(message);
        let payload = serializer.serialize_message(&metadata, &data).unwrap();

        // Store serialize time for warmup-filtered messages
        if i >= warmup_messages {
            let serialize_ms = (serialize_done - serialize_start) as f64 / 1_000_000.0;
            let _ = forward_serialize_tx.try_send(serialize_ms);
        }

        middleware
            .publish_raw(type_name, &forward_topic, &payload)
            .await
            .expect("Failed to publish forward message");

        // Small delay between messages
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    // Wait for all messages with timeout
    let timeout = Duration::from_secs(120);
    let start = Instant::now();
    while received_count.load(Ordering::SeqCst) < total_expected {
        if start.elapsed() > timeout {
            eprintln!(
                "Warning: Timeout waiting for messages. Received {}/{}",
                received_count.load(Ordering::SeqCst),
                total_expected
            );
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Signal done and cleanup
    done.store(true, Ordering::SeqCst);

    // Wait for echo task to finish
    let _ = tokio::time::timeout(Duration::from_secs(2), echo_task).await;

    // Collect measurements
    let mut measurements = Vec::new();
    while let Ok(m) = measurement_rx.try_recv() {
        measurements.push(m);
    }

    measurements
}

#[tokio::main]
async fn main() {
    // Get transport name from command line args (default to "zenoh")
    let args: Vec<String> = env::args().collect();
    let transport_name = args.get(1).map(|s| s.as_str()).unwrap_or("zenoh");

    println!("{} Publish-Subscribe Latency Benchmark (Rust)", transport_name.to_uppercase());
    println!("{}", "=".repeat(60));

    // Topic prefix using dash separator (works with both Zenoh and Kafka)
    let topic_prefix = format!("benchmark-rust-{}", transport_name);

    // Small messages benchmark (64 bytes)
    println!("\nRunning small messages benchmark (64 bytes payload)...");
    let measurements = run_benchmark(transport_name, 200, 10, 64, &format!("{}-small", topic_prefix)).await;
    print_benchmark_results("Small Messages (64 bytes) Latency Benchmark", &measurements);

    // Main benchmark (matches Python: 500 messages, 20 warmup, 64 bytes)
    println!("\nRunning main benchmark (500 messages, 64 bytes payload)...");
    let measurements = run_benchmark(transport_name, 500, 20, 64, &format!("{}-main", topic_prefix)).await;
    print_benchmark_results("Main Benchmark (500 messages, 64 bytes)", &measurements);

    // Large messages benchmark (500 KB)
    println!("\nRunning large messages benchmark (500 KB payload)...");
    let measurements = run_benchmark(transport_name, 100, 10, 500 * 1024, &format!("{}-large", topic_prefix)).await;
    print_benchmark_results("Large Messages (500 KB) Latency Benchmark", &measurements);

    // Detailed breakdown benchmark (500 KB)
    println!("\nRunning detailed breakdown benchmark (500 KB payload)...");
    let detailed_measurements = run_detailed_benchmark(transport_name, 100, 10, 500 * 1024, &format!("{}-detailed", topic_prefix)).await;
    print_detailed_benchmark_results("Detailed Breakdown (500 KB) - Serialization vs Transport", &detailed_measurements);

    // Varying load test
    println!("\n\nVarying Load Test Results:");
    println!("{}", "-".repeat(60));
    for num_messages in [50, 200, 500] {
        let measurements = run_benchmark(
            transport_name,
            num_messages,
            10,
            64,
            &format!("{}-load-{}", topic_prefix, num_messages),
        )
        .await;
        if !measurements.is_empty() {
            let avg_roundtrip: f64 = measurements.iter().map(|m| m.roundtrip_latency_ms).sum::<f64>()
                / measurements.len() as f64;
            println!(
                "{:>4} messages: avg round-trip = {:.3} ms",
                num_messages,
                avg_roundtrip
            );
        }
    }
    println!("{}", "-".repeat(60));
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require the appropriate middleware feature to be enabled
    // Run with: cargo test --example latency_benchmark --features zenoh
    // or:       cargo test --example latency_benchmark --features kafka

    #[tokio::test]
    #[ignore] // Ignore by default as it requires middleware to be available
    async fn test_latency_benchmark() {
        // Try zenoh first, then kafka
        let transport_name = if MiddlewareRegistry::new().get("zenoh").is_some() {
            "zenoh"
        } else if MiddlewareRegistry::new().get("kafka").is_some() {
            "kafka"
        } else {
            panic!("No middleware available for testing");
        };

        let measurements = run_benchmark(transport_name, 50, 5, 64, "test-benchmark").await;

        assert!(
            !measurements.is_empty(),
            "Should have collected measurements"
        );

        // Verify latencies are positive and reasonable
        for m in &measurements {
            assert!(
                m.forward_latency_ms > 0.0,
                "Forward latency should be positive"
            );
            assert!(
                m.return_latency_ms > 0.0,
                "Return latency should be positive"
            );
            assert!(
                m.roundtrip_latency_ms > 0.0,
                "Round-trip latency should be positive"
            );
            assert!(
                m.roundtrip_latency_ms < 5000.0,
                "Round-trip latency should be less than 5 seconds"
            );
        }
    }

    #[tokio::test]
    async fn test_stats_calculation() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let stats = Stats::new(values);

        assert!((stats.mean() - 3.0).abs() < 0.001);
        assert!((stats.median() - 3.0).abs() < 0.001);
        assert!((stats.min() - 1.0).abs() < 0.001);
        assert!((stats.max() - 5.0).abs() < 0.001);
    }
}
