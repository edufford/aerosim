//! Minimal latency benchmark for middleware pub/sub round-trip timing.
//!
//! A simple, readable benchmark that measures round-trip latency between two nodes
//! with detailed breakdown of serialization, transport, and deserialization times.
//!
//! Uses the raw publish/subscribe API with explicit serialization for clarity.
//!
//! Run with:
//!   cargo run --example latency_benchmark_minimal -- zenoh
//!   cargo run --example latency_benchmark_minimal -- kafka
//!
//! Configure via constants below: PAYLOAD_SIZE_BYTES, NUM_MESSAGES, USE_BINCODE

use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::json;
use tokio::sync::Notify;

use aerosim_data::middleware::{
    BincodeSerializer, Metadata, Middleware, MiddlewareRaw, MiddlewareRegistry, Serializer,
    SerializerEnum,
};
use aerosim_data::types::JsonData;

// =============================================================================
// CONFIGURATION - Modify these to change benchmark parameters
// =============================================================================
const PAYLOAD_SIZE_BYTES: usize = 1024; // Size of test payload (e.g., 64, 1024, 512000)
const NUM_MESSAGES: usize = 1;          // Number of round-trips to measure
const USE_BINCODE: bool = false;        // true for bincode, false for JSON serialization
// =============================================================================

/// Detailed timing measurement for one round-trip
#[derive(Debug, Clone, Default)]
struct TimingMeasurement {
    serialize_a_ms: f64,
    transport_ab_ms: f64,
    deserialize_b_ms: f64,
    serialize_b_ms: f64,
    transport_ba_ms: f64,
    deserialize_a_ms: f64,
    roundtrip_ms: f64,
}

#[tokio::main]
async fn main() {
    // Parse transport from command line
    let args: Vec<String> = env::args().collect();
    let transport_name = args.get(1).map(|s| s.as_str()).unwrap_or("zenoh");

    // Create middleware and serializer
    let registry = MiddlewareRegistry::new();
    let middleware = registry
        .get(transport_name)
        .unwrap_or_else(|| panic!("{} middleware not available", transport_name));
    let serializer = if USE_BINCODE {
        SerializerEnum::from(BincodeSerializer)
    } else {
        middleware.get_serializer()
    };

    // Create unique topic names using timestamp
    let unique_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let forward_topic = format!("bench-{}-fwd", unique_id);
    let return_topic = format!("bench-{}-ret", unique_id);
    let type_name = "aerosim::types::JsonData";

    // Create test payload
    let payload_str: String = "x".repeat(PAYLOAD_SIZE_BYTES);

    // Shared state for round-trip synchronization
    let measurements: Arc<Mutex<Vec<TimingMeasurement>>> = Arc::new(Mutex::new(Vec::new()));
    let roundtrip_complete = Arc::new(Notify::new());
    let done = Arc::new(AtomicBool::new(false));

    // Reference time for all measurements (using Instant for monotonic timing)
    let ref_time = Instant::now();

    // --- Node B: Echo service ---
    let echo_middleware = middleware.clone();
    let echo_return_topic = return_topic.clone();
    let echo_type_name = type_name.to_string();
    let echo_ref_time = ref_time;
    let echo_done = done.clone();

    middleware
        .subscribe_raw(
            type_name,
            &forward_topic,
            Box::new(move |raw_bytes: &[u8]| {
                if echo_done.load(Ordering::SeqCst) {
                    return Ok(());
                }

                let t_recv_b = echo_ref_time.elapsed().as_nanos() as u64;

                // Deserialize incoming message
                let echo_ser = if USE_BINCODE {
                    SerializerEnum::from(BincodeSerializer)
                } else {
                    echo_middleware.get_serializer()
                };
                let (_metadata, data): (Metadata, JsonData) = echo_ser
                    .deserialize_message(raw_bytes)
                    .expect("Failed to deserialize");
                let t_deser_b = echo_ref_time.elapsed().as_nanos() as u64;

                let json_data = data.get_data().unwrap();
                let t_start_a = json_data["t_start_a"].as_u64().unwrap_or(0);
                let t_ser_a = json_data["t_ser_a"].as_u64().unwrap_or(0);

                // Create echo with timing data (first pass to measure serialize time)
                let echo_data = JsonData::new(json!({
                    "t_start_a": t_start_a,
                    "t_ser_a": t_ser_a,
                    "t_recv_b": t_recv_b,
                    "t_deser_b": t_deser_b,
                }));

                let metadata = Metadata::new(&echo_return_topic, &echo_type_name, None, None);
                let _ = echo_ser.serialize_message(&metadata, &echo_data);
                let t_ser_b = echo_ref_time.elapsed().as_nanos() as u64;

                // Re-serialize with t_ser_b included
                let echo_data_final = JsonData::new(json!({
                    "t_start_a": t_start_a,
                    "t_ser_a": t_ser_a,
                    "t_recv_b": t_recv_b,
                    "t_deser_b": t_deser_b,
                    "t_ser_b": t_ser_b,
                }));

                let echo_bytes = echo_ser
                    .serialize_message(&metadata, &echo_data_final)
                    .expect("Serialize failed");

                let mw = echo_middleware.clone();
                let topic = echo_return_topic.clone();
                let type_n = echo_type_name.clone();
                tokio::spawn(async move {
                    let _ = mw.publish_raw(&type_n, &topic, &echo_bytes).await;
                });

                Ok(())
            }),
        )
        .await
        .expect("Failed to subscribe to forward topic");

    // --- Node A: Latency measurer ---
    let measure_middleware = middleware.clone();
    let measure_measurements = measurements.clone();
    let measure_notify = roundtrip_complete.clone();
    let measure_ref_time = ref_time;

    middleware
        .subscribe_raw(
            type_name,
            &return_topic,
            Box::new(move |raw_bytes: &[u8]| {
                let t_recv_a = measure_ref_time.elapsed().as_nanos() as u64;

                // Deserialize echo message
                let measure_ser = if USE_BINCODE {
                    SerializerEnum::from(BincodeSerializer)
                } else {
                    measure_middleware.get_serializer()
                };
                let (_metadata, data): (Metadata, JsonData) = measure_ser
                    .deserialize_message(raw_bytes)
                    .expect("Failed to deserialize");
                let t_deser_a = measure_ref_time.elapsed().as_nanos() as u64;

                let json_data = data.get_data().unwrap();

                // Extract all timestamps (in nanoseconds)
                let t_start_a = json_data["t_start_a"].as_u64().unwrap_or(0);
                let t_ser_a = json_data["t_ser_a"].as_u64().unwrap_or(0);
                let t_recv_b = json_data["t_recv_b"].as_u64().unwrap_or(0);
                let t_deser_b = json_data["t_deser_b"].as_u64().unwrap_or(0);
                let t_ser_b = json_data["t_ser_b"].as_u64().unwrap_or(0);

                // Calculate timing breakdown (convert nanos to ms)
                let measurement = TimingMeasurement {
                    serialize_a_ms: (t_ser_a - t_start_a) as f64 / 1_000_000.0,
                    transport_ab_ms: (t_recv_b - t_ser_a) as f64 / 1_000_000.0,
                    deserialize_b_ms: (t_deser_b - t_recv_b) as f64 / 1_000_000.0,
                    serialize_b_ms: (t_ser_b - t_deser_b) as f64 / 1_000_000.0,
                    transport_ba_ms: (t_recv_a - t_ser_b) as f64 / 1_000_000.0,
                    deserialize_a_ms: (t_deser_a - t_recv_a) as f64 / 1_000_000.0,
                    roundtrip_ms: (t_deser_a - t_start_a) as f64 / 1_000_000.0,
                };

                if let Ok(mut m) = measure_measurements.lock() {
                    m.push(measurement);
                }
                measure_notify.notify_one();

                Ok(())
            }),
        )
        .await
        .expect("Failed to subscribe to return topic");

    // Wait for subscriptions to be ready
    tokio::time::sleep(Duration::from_millis(1000)).await;

    // --- Run the benchmark ---
    println!("\n{}", "=".repeat(60));
    let serialization_type = if USE_BINCODE { "Bincode" } else { "JSON" };
    println!("{} Minimal Latency Benchmark", transport_name.to_uppercase());
    println!(
        "Payload: {} bytes | Serialization: {}",
        PAYLOAD_SIZE_BYTES, serialization_type
    );
    println!("{}", "=".repeat(60));

    // --- Warmup loop (not measured) ---
    {
        let t_start = ref_time.elapsed().as_nanos() as u64;
        let warmup_msg = JsonData::new(json!({
            "t_start_a": t_start,
            "t_ser_a": t_start,
            "payload": payload_str.clone(),
        }));
        let metadata = Metadata::new(&forward_topic, type_name, None, None);
        let raw_bytes = serializer
            .serialize_message(&metadata, &warmup_msg)
            .expect("Serialize failed");
        middleware
            .publish_raw(type_name, &forward_topic, &raw_bytes)
            .await
            .expect("Publish failed");

        // Wait for warmup to complete
        tokio::time::timeout(Duration::from_secs(30), roundtrip_complete.notified())
            .await
            .ok();
        if let Ok(mut m) = measurements.lock() {
            m.clear();
        }
    }

    // --- Measured loops ---
    for i in 0..NUM_MESSAGES {
        // Create message with timestamp and payload
        let t_start_a = ref_time.elapsed().as_nanos() as u64;
        let message = JsonData::new(json!({
            "t_start_a": t_start_a,
            "payload": payload_str.clone(),
        }));

        // Serialize (first pass to measure time)
        let metadata = Metadata::new(&forward_topic, type_name, None, None);
        let _ = serializer.serialize_message(&metadata, &message);
        let t_ser_a = ref_time.elapsed().as_nanos() as u64;

        // Re-serialize with t_ser_a included
        let message_final = JsonData::new(json!({
            "t_start_a": t_start_a,
            "t_ser_a": t_ser_a,
            "payload": payload_str.clone(),
        }));
        let raw_bytes = serializer
            .serialize_message(&metadata, &message_final)
            .expect("Serialize failed");

        // Publish
        middleware
            .publish_raw(type_name, &forward_topic, &raw_bytes)
            .await
            .expect("Publish failed");

        // Wait for this message to complete round-trip before sending next
        if tokio::time::timeout(Duration::from_secs(30), roundtrip_complete.notified())
            .await
            .is_err()
        {
            println!("Timeout waiting for message {}", i);
            break;
        }
    }

    // Signal done
    done.store(true, Ordering::SeqCst);

    // --- Print results ---
    let results = measurements.lock().unwrap();
    if !results.is_empty() {
        let avg_label = if results.len() > 1 { " (ave)" } else { "" };
        let n = results.len() as f64;

        let avg = |f: fn(&TimingMeasurement) -> f64| -> f64 {
            results.iter().map(f).sum::<f64>() / n
        };

        println!("\nDetailed Latency Breakdown ({} samples):", results.len());
        println!("\nForward (A -> B):");
        println!(
            "  Serialize A:   {:.3} ms{}",
            avg(|m| m.serialize_a_ms),
            avg_label
        );
        println!(
            "  Transport A→B: {:.3} ms{}",
            avg(|m| m.transport_ab_ms),
            avg_label
        );
        println!(
            "  Deserialize B: {:.3} ms{}",
            avg(|m| m.deserialize_b_ms),
            avg_label
        );
        println!("\nReturn (B -> A):");
        println!(
            "  Serialize B:   {:.3} ms{}",
            avg(|m| m.serialize_b_ms),
            avg_label
        );
        println!(
            "  Transport B→A: {:.3} ms{}",
            avg(|m| m.transport_ba_ms),
            avg_label
        );
        println!(
            "  Deserialize A: {:.3} ms{}",
            avg(|m| m.deserialize_a_ms),
            avg_label
        );
        println!(
            "\nTotal Round-Trip: {:.3} ms{}",
            avg(|m| m.roundtrip_ms),
            avg_label
        );
        println!("{}\n", "=".repeat(60));
    } else {
        println!("No latency measurements received!");
    }
}
