"""
Minimal latency benchmark for middleware pub/sub round-trip timing.

A simple, readable benchmark that measures round-trip latency between two nodes
with detailed breakdown of serialization, transport, and deserialization times.

Uses the raw publish/subscribe API with explicit serialization for clarity.

Run with:
    pytest test_latency_benchmark_minimal.py -v -s
    pytest test_latency_benchmark_minimal.py -v -s -k "zenoh"
    pytest test_latency_benchmark_minimal.py -v -s -k "kafka"

Configure via constants below: PAYLOAD_SIZE_BYTES, NUM_MESSAGES, USE_BINCODE
"""
import pytest
import time
import threading
import uuid
import statistics

from aerosim_data import middleware as mw
from aerosim_data import types as aerosim_types

# =============================================================================
# CONFIGURATION - Modify these to change benchmark parameters
# =============================================================================
PAYLOAD_SIZE_BYTES = 1024      # Size of test payload (e.g., 64, 1024, 512000)
NUM_MESSAGES = 1               # Number of round-trips to measure
USE_BINCODE = False            # True for bincode, False for JSON serialization
# =============================================================================

DATA_TYPE = "aerosim::types::JsonData"


def get_serializer(transport_name: str):
    """Get the appropriate serializer based on configuration."""
    if USE_BINCODE:
        return mw.BincodeSerializer()
    elif transport_name == "zenoh":
        return mw.ZenohSerializer()
    else:
        return mw.KafkaSerializer()


@pytest.fixture(scope="module", params=["zenoh", "kafka"])
def transport(request):
    """Create middleware transport for the test."""
    transport_name = request.param
    if transport_name == "zenoh":
        transport_obj = mw.ZenohMiddleware()
    else:
        transport_obj = mw.KafkaMiddleware()
    yield transport_name, transport_obj
    transport_obj.close()


def test_minimal_roundtrip_latency(transport):
    """
    Minimal round-trip latency benchmark using raw pub/sub API.

    Timing breakdown:
        Forward: serialize_a -> transport_ab -> deserialize_b
        Return:  serialize_b -> transport_ba -> deserialize_a
    """
    transport_name, middleware = transport
    serializer = get_serializer(transport_name)

    # Create unique topic names for this test run
    test_id = uuid.uuid4().hex[:8]
    forward_topic = f"bench-{test_id}-fwd"
    return_topic = f"bench-{test_id}-ret"

    # Create test payload
    payload_str = "x" * PAYLOAD_SIZE_BYTES

    # Storage for detailed measurements (each entry is a dict of timings)
    measurements = []
    roundtrip_complete = threading.Event()

    # --- Node B: Echo service ---
    def echo_callback(raw_bytes: bytes):
        """Receive on forward topic, deserialize, re-serialize, publish to return."""
        t_recv_b = time.perf_counter()

        # Deserialize incoming message
        _metadata, data = serializer.deserialize_message(aerosim_types.JsonData, raw_bytes)
        t_deser_b = time.perf_counter()

        json_data = data.get_data()

        # Create echo with timing data
        echo_data = aerosim_types.JsonData({
            # Forward leg timings from Node A
            "t_start_a": json_data.get("t_start_a"),
            "t_ser_a": json_data.get("t_ser_a"),
            # Forward leg timings from Node B
            "t_recv_b": t_recv_b,
            "t_deser_b": t_deser_b,
        })

        # Serialize echo
        metadata = mw.Metadata(return_topic, DATA_TYPE)
        echo_bytes = serializer.serialize_message(metadata, echo_data)
        t_ser_b = time.perf_counter()

        # Update echo with serialize time and publish
        echo_data = aerosim_types.JsonData({
            "t_start_a": json_data.get("t_start_a"),
            "t_ser_a": json_data.get("t_ser_a"),
            "t_recv_b": t_recv_b,
            "t_deser_b": t_deser_b,
            "t_ser_b": t_ser_b,
        })
        echo_bytes = serializer.serialize_message(metadata, echo_data)

        middleware.publish_raw(DATA_TYPE, return_topic, echo_bytes)

    # --- Node A: Latency measurer ---
    def measure_callback(raw_bytes: bytes):
        """Receive echo, deserialize, calculate detailed latencies."""
        t_recv_a = time.perf_counter()

        # Deserialize echo message
        _metadata, data = serializer.deserialize_message(aerosim_types.JsonData, raw_bytes)
        t_deser_a = time.perf_counter()

        json_data = data.get_data()

        # Extract all timestamps
        t_start_a = json_data.get("t_start_a", 0)
        t_ser_a = json_data.get("t_ser_a", 0)
        t_recv_b = json_data.get("t_recv_b", 0)
        t_deser_b = json_data.get("t_deser_b", 0)
        t_ser_b = json_data.get("t_ser_b", 0)

        # Calculate timing breakdown (in ms)
        measurements.append({
            # Forward leg
            "serialize_a": (t_ser_a - t_start_a) * 1000,
            "transport_ab": (t_recv_b - t_ser_a) * 1000,
            "deserialize_b": (t_deser_b - t_recv_b) * 1000,
            # Return leg
            "serialize_b": (t_ser_b - t_deser_b) * 1000,
            "transport_ba": (t_recv_a - t_ser_b) * 1000,
            "deserialize_a": (t_deser_a - t_recv_a) * 1000,
            # Totals
            "roundtrip": (t_deser_a - t_start_a) * 1000,
        })
        roundtrip_complete.set()

    # Set up subscriptions
    middleware.subscribe_raw(DATA_TYPE, forward_topic, echo_callback)
    middleware.subscribe_raw(DATA_TYPE, return_topic, measure_callback)

    # Wait for subscriptions to be ready
    time.sleep(1.0)

    # --- Run the benchmark ---
    serialization_type = "Bincode" if USE_BINCODE else "JSON"
    print(f"\n{'='*60}")
    print(f"{transport_name.upper()} Minimal Latency Benchmark")
    print(f"Payload: {PAYLOAD_SIZE_BYTES} bytes | Serialization: {serialization_type}")
    print(f"{'='*60}")

    # Warmup loop (not measured)
    roundtrip_complete.clear()
    t_start = time.perf_counter()
    warmup_msg = aerosim_types.JsonData({
        "t_start_a": t_start,
        "t_ser_a": t_start,
        "payload": payload_str
    })
    metadata = mw.Metadata(forward_topic, DATA_TYPE)
    raw_bytes = serializer.serialize_message(metadata, warmup_msg)
    middleware.publish_raw(DATA_TYPE, forward_topic, raw_bytes)
    roundtrip_complete.wait(timeout=30.0)
    measurements.clear()  # Discard warmup measurement

    # Measured loops
    for i in range(NUM_MESSAGES):
        roundtrip_complete.clear()

        # Create message with timestamp and payload
        t_start_a = time.perf_counter()
        message = aerosim_types.JsonData({
            "t_start_a": t_start_a,
            "payload": payload_str
        })

        # Serialize
        metadata = mw.Metadata(forward_topic, DATA_TYPE)
        raw_bytes = serializer.serialize_message(metadata, message)
        t_ser_a = time.perf_counter()

        # Update message with serialization end time and re-serialize
        message = aerosim_types.JsonData({
            "t_start_a": t_start_a,
            "t_ser_a": t_ser_a,
            "payload": payload_str
        })
        raw_bytes = serializer.serialize_message(metadata, message)

        # Publish
        middleware.publish_raw(DATA_TYPE, forward_topic, raw_bytes)

        # Wait for this message to complete round-trip before sending next
        if not roundtrip_complete.wait(timeout=30.0):
            print(f"Timeout waiting for message {i}")
            break

    # --- Print results ---
    if measurements:
        avg_label = " (ave)" if len(measurements) > 1 else ""
        print(f"\nDetailed Latency Breakdown ({len(measurements)} samples):")
        print("\nForward (A -> B):")
        print(f"  Serialize A:   {statistics.mean([m['serialize_a'] for m in measurements]):.3f} ms{avg_label}")
        print(f"  Transport A→B: {statistics.mean([m['transport_ab'] for m in measurements]):.3f} ms{avg_label}")
        print(f"  Deserialize B: {statistics.mean([m['deserialize_b'] for m in measurements]):.3f} ms{avg_label}")
        print("\nReturn (B -> A):")
        print(f"  Serialize B:   {statistics.mean([m['serialize_b'] for m in measurements]):.3f} ms{avg_label}")
        print(f"  Transport B→A: {statistics.mean([m['transport_ba'] for m in measurements]):.3f} ms{avg_label}")
        print(f"  Deserialize A: {statistics.mean([m['deserialize_a'] for m in measurements]):.3f} ms{avg_label}")
        print(f"\nTotal Round-Trip: {statistics.mean([m['roundtrip'] for m in measurements]):.3f} ms{avg_label}")
        print(f"{'='*60}\n")

    assert len(measurements) >= NUM_MESSAGES, f"Too few responses: {len(measurements)}/{NUM_MESSAGES}"
