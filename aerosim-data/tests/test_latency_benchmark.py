"""
Performance benchmark tests for middleware publish-subscribe latency.

This module tests round-trip latency between two nodes using various middleware transports,
measuring message latency in both directions. Supports Zenoh and Kafka transports.

Run with:
    pytest aerosim-data/tests/test_latency_benchmark.py -v              # All transports
    pytest aerosim-data/tests/test_latency_benchmark.py -v -k "zenoh"   # Zenoh only
    pytest aerosim-data/tests/test_latency_benchmark.py -v -k "kafka"   # Kafka only
    pytest aerosim-data/tests/test_latency_benchmark.py -v -s           # Show benchmark output
"""
import pytest
import time
import threading
import statistics
import uuid
from dataclasses import dataclass
from typing import List

from aerosim_data import middleware as aerosim_middleware
from aerosim_data import types as aerosim_types


# Transport configuration
# Note: Using "-" as separator for all transports since it's valid for both Zenoh and Kafka.
# Kafka topic names don't support "/" characters.
TRANSPORT_CONFIGS = {
    "zenoh": {
        "serializer_class": "ZenohSerializer",
    },
    "kafka": {
        "serializer_class": "KafkaSerializer",
    },
}

# Topic separator that works on all transports (Kafka doesn't support "/")
TOPIC_SEPARATOR = "-"


def get_serializer(transport_name: str):
    """Get the appropriate JSON serializer for the transport."""
    config = TRANSPORT_CONFIGS.get(transport_name, {})
    serializer_class = config.get("serializer_class", "ZenohSerializer")
    return getattr(aerosim_middleware, serializer_class)()


@dataclass
class LatencyResult:
    """Stores latency measurement results."""
    forward_latencies_ms: List[float]
    return_latencies_ms: List[float]
    roundtrip_latencies_ms: List[float]

    def print_summary(self, title: str = "Publish-Subscribe Latency Benchmark Results"):
        """Print a summary of the latency measurements."""
        print("\n" + "=" * 60)
        print(title)
        print("=" * 60)

        if self.forward_latencies_ms:
            print("\nForward Latency (Node A -> Node B):")
            print(f"  Samples: {len(self.forward_latencies_ms)}")
            print(f"  Average: {statistics.mean(self.forward_latencies_ms):.3f} ms")
            print(f"  Median:  {statistics.median(self.forward_latencies_ms):.3f} ms")
            print(f"  Min:     {min(self.forward_latencies_ms):.3f} ms")
            print(f"  Max:     {max(self.forward_latencies_ms):.3f} ms")
            if len(self.forward_latencies_ms) > 1:
                print(f"  Std Dev: {statistics.stdev(self.forward_latencies_ms):.3f} ms")

        if self.return_latencies_ms:
            print("\nReturn Latency (Node B -> Node A):")
            print(f"  Samples: {len(self.return_latencies_ms)}")
            print(f"  Average: {statistics.mean(self.return_latencies_ms):.3f} ms")
            print(f"  Median:  {statistics.median(self.return_latencies_ms):.3f} ms")
            print(f"  Min:     {min(self.return_latencies_ms):.3f} ms")
            print(f"  Max:     {max(self.return_latencies_ms):.3f} ms")
            if len(self.return_latencies_ms) > 1:
                print(f"  Std Dev: {statistics.stdev(self.return_latencies_ms):.3f} ms")

        if self.roundtrip_latencies_ms:
            print("\nRound-Trip Latency (Node A -> Node B -> Node A):")
            print(f"  Samples: {len(self.roundtrip_latencies_ms)}")
            print(f"  Average: {statistics.mean(self.roundtrip_latencies_ms):.3f} ms")
            print(f"  Median:  {statistics.median(self.roundtrip_latencies_ms):.3f} ms")
            print(f"  Min:     {min(self.roundtrip_latencies_ms):.3f} ms")
            print(f"  Max:     {max(self.roundtrip_latencies_ms):.3f} ms")
            if len(self.roundtrip_latencies_ms) > 1:
                print(f"  Std Dev: {statistics.stdev(self.roundtrip_latencies_ms):.3f} ms")

        print("\n" + "=" * 60)


@dataclass
class DetailedLatencyResult:
    """Stores detailed latency measurement results with serialization/transport breakdown."""
    # Total latencies
    forward_latencies_ms: List[float]
    return_latencies_ms: List[float]
    roundtrip_latencies_ms: List[float]

    # Forward leg breakdown (Node A -> Node B)
    forward_serialize_ms: List[float]  # Node A serialization time
    forward_transport_ms: List[float]  # Network transport time
    forward_deserialize_ms: List[float]  # Node B deserialization time

    # Return leg breakdown (Node B -> Node A)
    return_serialize_ms: List[float]  # Node B serialization time
    return_transport_ms: List[float]  # Network transport time
    return_deserialize_ms: List[float]  # Node A deserialization time

    def _print_stats(self, name: str, values: List[float], indent: str = "  "):
        """Print statistics for a list of values."""
        if not values:
            return
        print(f"{indent}{name}:")
        print(f"{indent}  Average: {statistics.mean(values):.3f} ms")
        if len(values) > 1:
            print(f"{indent}  Median:  {statistics.median(values):.3f} ms")
            print(f"{indent}  Min:     {min(values):.3f} ms")
            print(f"{indent}  Max:     {max(values):.3f} ms")

    def print_summary(self, title: str = "Detailed Latency Benchmark Results"):
        """Print a summary of the detailed latency measurements."""
        print("\n" + "=" * 70)
        print(title)
        print("=" * 70)
        print(f"Samples: {len(self.roundtrip_latencies_ms)}")

        # Forward leg
        if self.forward_latencies_ms:
            print(f"\nForward Leg (Node A -> Node B): {statistics.mean(self.forward_latencies_ms):.3f} ms avg")
            self._print_stats("Serialize (Node A)", self.forward_serialize_ms)
            self._print_stats("Transport", self.forward_transport_ms)
            self._print_stats("Deserialize (Node B)", self.forward_deserialize_ms)

        # Return leg
        if self.return_latencies_ms:
            print(f"\nReturn Leg (Node B -> Node A): {statistics.mean(self.return_latencies_ms):.3f} ms avg")
            self._print_stats("Serialize (Node B)", self.return_serialize_ms)
            self._print_stats("Transport", self.return_transport_ms)
            self._print_stats("Deserialize (Node A)", self.return_deserialize_ms)

        # Round-trip summary
        if self.roundtrip_latencies_ms:
            print(f"\nRound-Trip Total: {statistics.mean(self.roundtrip_latencies_ms):.3f} ms avg")

            # Calculate aggregate averages
            total_serialize = (
                statistics.mean(self.forward_serialize_ms) +
                statistics.mean(self.return_serialize_ms)
            ) if self.forward_serialize_ms and self.return_serialize_ms else 0

            total_transport = (
                statistics.mean(self.forward_transport_ms) +
                statistics.mean(self.return_transport_ms)
            ) if self.forward_transport_ms and self.return_transport_ms else 0

            total_deserialize = (
                statistics.mean(self.forward_deserialize_ms) +
                statistics.mean(self.return_deserialize_ms)
            ) if self.forward_deserialize_ms and self.return_deserialize_ms else 0

            total_time = total_serialize + total_transport + total_deserialize
            if total_time > 0:
                print("\n  Breakdown (averages):")
                print(f"    Total Serialize:   {total_serialize:.3f} ms ({100*total_serialize/total_time:.1f}%)")
                print(f"    Total Transport:   {total_transport:.3f} ms ({100*total_transport/total_time:.1f}%)")
                print(f"    Total Deserialize: {total_deserialize:.3f} ms ({100*total_deserialize/total_time:.1f}%)")

        print("\n" + "=" * 70)


class LatencyBenchmark:
    """
    Benchmarks pub-sub latency using a round-trip message pattern.

    Node A publishes messages with timestamps to a forward topic.
    Node B receives them, records forward latency, and echoes back to a return topic.
    Node A receives the echo and records return and round-trip latency.
    """

    def __init__(self, transport, transport_name: str = "zenoh", num_messages: int = 100,
                 warmup_messages: int = 10, topic_prefix: str = None, payload_size: int = 64):
        self.transport = transport
        self.transport_name = transport_name
        self.num_messages = num_messages
        self.warmup_messages = warmup_messages
        self.payload_size = payload_size

        # Use transport-appropriate topic separator
        sep = TOPIC_SEPARATOR
        self.topic_prefix = topic_prefix or f"benchmark{sep}{uuid.uuid4().hex[:8]}"
        self.forward_topic = f"{self.topic_prefix}{sep}forward"
        self.return_topic = f"{self.topic_prefix}{sep}return"

        # Latency storage
        self.forward_latencies_ms: List[float] = []
        self.return_latencies_ms: List[float] = []
        self.roundtrip_latencies_ms: List[float] = []

        # Track message timing
        self.send_times: dict = {}
        self.node_b_receive_times: dict = {}

        # Synchronization
        self.messages_received = 0
        self.lock = threading.Lock()
        self.all_received = threading.Event()

    def _node_b_callback(self, data: dict, metadata):
        """
        Node B's callback: receives forward message, records timing, echoes back.
        """
        receive_time = time.perf_counter()
        msg_id = data.get("msg_id")
        send_time = data.get("send_time")

        if msg_id is not None and send_time is not None:
            # Record Node B receive time for forward latency calculation
            with self.lock:
                self.node_b_receive_times[msg_id] = receive_time

            # Echo the message back with Node B's timestamp added
            echo_data = {
                "msg_id": msg_id,
                "original_send_time": send_time,
                "node_b_receive_time": receive_time,
                "node_b_send_time": time.perf_counter()
            }
            self.transport.publish(self.return_topic, echo_data)

    def _node_a_callback(self, data: dict, metadata):
        """
        Node A's callback: receives echo, calculates latencies.
        """
        receive_time = time.perf_counter()
        msg_id = data.get("msg_id")
        original_send_time = data.get("original_send_time")
        node_b_receive_time = data.get("node_b_receive_time")
        node_b_send_time = data.get("node_b_send_time")

        if all(v is not None for v in [msg_id, original_send_time, node_b_receive_time, node_b_send_time]):
            # Skip warmup messages
            if msg_id >= self.warmup_messages:
                # Forward latency: original send -> Node B receive
                forward_latency = (node_b_receive_time - original_send_time) * 1000

                # Return latency: Node B send -> Node A receive
                return_latency = (receive_time - node_b_send_time) * 1000

                # Round-trip latency: original send -> Node A receive
                roundtrip_latency = (receive_time - original_send_time) * 1000

                with self.lock:
                    self.forward_latencies_ms.append(forward_latency)
                    self.return_latencies_ms.append(return_latency)
                    self.roundtrip_latencies_ms.append(roundtrip_latency)

            with self.lock:
                self.messages_received += 1
                if self.messages_received >= self.num_messages + self.warmup_messages:
                    self.all_received.set()

    def run(self, timeout: float = 60.0) -> LatencyResult:
        """
        Run the latency benchmark.

        Args:
            timeout: Maximum time to wait for all messages (seconds).

        Returns:
            LatencyResult with measured latencies.
        """
        # Set up subscriptions with unique topics
        self.transport.subscribe(aerosim_types.JsonData, self.forward_topic, self._node_b_callback)
        self.transport.subscribe(aerosim_types.JsonData, self.return_topic, self._node_a_callback)

        # Allow subscriptions to establish
        time.sleep(1.0)

        # Send messages (warmup + actual)
        total_messages = self.num_messages + self.warmup_messages
        for i in range(total_messages):
            send_time = time.perf_counter()
            self.send_times[i] = send_time

            message = {
                "msg_id": i,
                "send_time": send_time,
                "payload": "x" * self.payload_size
            }
            self.transport.publish(self.forward_topic, message)

            # Small delay between messages to avoid overwhelming the system
            time.sleep(0.02)

        # Wait for all messages to complete round-trip
        if not self.all_received.wait(timeout=timeout):
            print(f"Warning: Timeout waiting for messages. Received {self.messages_received}/{total_messages}")

        return LatencyResult(
            forward_latencies_ms=self.forward_latencies_ms.copy(),
            return_latencies_ms=self.return_latencies_ms.copy(),
            roundtrip_latencies_ms=self.roundtrip_latencies_ms.copy()
        )


class LatencyBenchmarkBincode:
    """
    Benchmarks pub-sub latency using bincode serialization instead of JSON.

    This uses raw pub/sub with BincodeSerializer to compare binary vs JSON serialization.
    """

    def __init__(self, transport, transport_name: str = "zenoh", num_messages: int = 100,
                 warmup_messages: int = 10, topic_prefix: str = None, payload_size: int = 64):
        self.transport = transport
        self.transport_name = transport_name
        self.serializer = aerosim_middleware.BincodeSerializer()
        self.num_messages = num_messages
        self.warmup_messages = warmup_messages
        self.payload_size = payload_size

        # Use transport-appropriate topic separator
        sep = TOPIC_SEPARATOR
        self.topic_prefix = topic_prefix or f"benchmark{sep}bincode{sep}{uuid.uuid4().hex[:8]}"
        self.forward_topic = f"{self.topic_prefix}{sep}forward"
        self.return_topic = f"{self.topic_prefix}{sep}return"

        # Latency storage
        self.forward_latencies_ms: List[float] = []
        self.return_latencies_ms: List[float] = []
        self.roundtrip_latencies_ms: List[float] = []

        # Track message timing
        self.send_times: dict = {}
        self.node_b_receive_times: dict = {}

        # Synchronization
        self.messages_received = 0
        self.lock = threading.Lock()
        self.all_received = threading.Event()

    def _node_b_callback(self, payload: bytes):
        """
        Node B's callback: receives forward message, records timing, echoes back using bincode.
        """
        receive_time = time.perf_counter()

        # Deserialize using bincode
        _metadata, data = self.serializer.deserialize_message(aerosim_types.JsonData, payload)
        json_data = data.get_data()
        msg_id = json_data.get("msg_id")
        send_time = json_data.get("send_time")

        if msg_id is not None and send_time is not None:
            with self.lock:
                self.node_b_receive_times[msg_id] = receive_time

            # Echo back with bincode serialization
            echo_data = aerosim_types.JsonData({
                "msg_id": msg_id,
                "original_send_time": send_time,
                "node_b_receive_time": receive_time,
                "node_b_send_time": time.perf_counter()
            })
            metadata = aerosim_middleware.Metadata(
                self.return_topic, "aerosim::types::JsonData"
            )
            echo_payload = self.serializer.serialize_message(metadata, echo_data)
            self.transport.publish_raw("aerosim::types::JsonData", self.return_topic, echo_payload)

    def _node_a_callback(self, payload: bytes):
        """
        Node A's callback: receives echo, calculates latencies.
        """
        receive_time = time.perf_counter()

        # Deserialize using bincode
        _metadata, data = self.serializer.deserialize_message(aerosim_types.JsonData, payload)
        json_data = data.get_data()

        msg_id = json_data.get("msg_id")
        original_send_time = json_data.get("original_send_time")
        node_b_receive_time = json_data.get("node_b_receive_time")
        node_b_send_time = json_data.get("node_b_send_time")

        if all(v is not None for v in [msg_id, original_send_time, node_b_receive_time, node_b_send_time]):
            # Skip warmup messages
            if msg_id >= self.warmup_messages:
                forward_latency = (node_b_receive_time - original_send_time) * 1000
                return_latency = (receive_time - node_b_send_time) * 1000
                roundtrip_latency = (receive_time - original_send_time) * 1000

                with self.lock:
                    self.forward_latencies_ms.append(forward_latency)
                    self.return_latencies_ms.append(return_latency)
                    self.roundtrip_latencies_ms.append(roundtrip_latency)

            with self.lock:
                self.messages_received += 1
                if self.messages_received >= self.num_messages + self.warmup_messages:
                    self.all_received.set()

    def run(self, timeout: float = 60.0) -> LatencyResult:
        """
        Run the latency benchmark using bincode serialization.

        Args:
            timeout: Maximum time to wait for all messages (seconds).

        Returns:
            LatencyResult with measured latencies.
        """
        # Set up raw subscriptions with unique topics
        self.transport.subscribe_raw(
            "aerosim::types::JsonData", self.forward_topic, self._node_b_callback
        )
        self.transport.subscribe_raw(
            "aerosim::types::JsonData", self.return_topic, self._node_a_callback
        )

        # Allow subscriptions to establish
        time.sleep(1.0)

        # Generate payload
        payload_str = "x" * self.payload_size

        # Send messages (warmup + actual)
        total_messages = self.num_messages + self.warmup_messages
        for i in range(total_messages):
            send_time = time.perf_counter()

            message_data = aerosim_types.JsonData({
                "msg_id": i,
                "send_time": send_time,
                "payload": payload_str
            })
            metadata = aerosim_middleware.Metadata(
                self.forward_topic, "aerosim::types::JsonData"
            )
            payload = self.serializer.serialize_message(metadata, message_data)
            self.transport.publish_raw("aerosim::types::JsonData", self.forward_topic, payload)

            # Small delay between messages to avoid overwhelming the system
            time.sleep(0.02)

        # Wait for all messages to complete round-trip
        if not self.all_received.wait(timeout=timeout):
            print(f"Warning: Timeout waiting for messages. Received {self.messages_received}/{total_messages}")

        return LatencyResult(
            forward_latencies_ms=self.forward_latencies_ms.copy(),
            return_latencies_ms=self.return_latencies_ms.copy(),
            roundtrip_latencies_ms=self.roundtrip_latencies_ms.copy()
        )


class DetailedLatencyBenchmark:
    """
    Benchmarks pub-sub latency with detailed breakdown of serialization vs transport time.

    This measures:
    - Serialization time (time to encode the message)
    - Transport time (network transfer time)
    - Deserialization time (time to decode the message)
    """

    def __init__(self, transport, transport_name: str = "zenoh", serializer=None,
                 serializer_name: str = "unknown", num_messages: int = 100,
                 warmup_messages: int = 10, topic_prefix: str = None, payload_size: int = 64):
        self.transport = transport
        self.transport_name = transport_name
        self.serializer = serializer or get_serializer(transport_name)
        self.serializer_name = serializer_name
        self.num_messages = num_messages
        self.warmup_messages = warmup_messages
        self.payload_size = payload_size

        # Use transport-appropriate topic separator
        sep = TOPIC_SEPARATOR
        self.topic_prefix = topic_prefix or f"benchmark{sep}detailed{sep}{uuid.uuid4().hex[:8]}"
        self.forward_topic = f"{self.topic_prefix}{sep}forward"
        self.return_topic = f"{self.topic_prefix}{sep}return"

        # Total latency storage
        self.forward_latencies_ms: List[float] = []
        self.return_latencies_ms: List[float] = []
        self.roundtrip_latencies_ms: List[float] = []

        # Detailed timing breakdown
        self.forward_serialize_ms: List[float] = []
        self.forward_transport_ms: List[float] = []
        self.forward_deserialize_ms: List[float] = []
        self.return_serialize_ms: List[float] = []
        self.return_transport_ms: List[float] = []
        self.return_deserialize_ms: List[float] = []

        # Synchronization
        self.messages_received = 0
        self.lock = threading.Lock()
        self.all_received = threading.Event()

    def _node_b_callback(self, payload: bytes):
        """
        Node B's callback: receives forward message, records timing, echoes back.
        """
        receive_time = time.perf_counter()

        # Deserialize the incoming message and measure time
        _metadata, data = self.serializer.deserialize_message(aerosim_types.JsonData, payload)
        deserialize_done_time = time.perf_counter()

        json_data = data.get_data()
        msg_id = json_data.get("msg_id")
        original_send_time = json_data.get("send_time")
        send_after_serialize_time = json_data.get("send_after_serialize_time")

        if msg_id is not None and original_send_time is not None:
            # Measure Node B's serialization time
            serialize_start_time = time.perf_counter()
            echo_data = aerosim_types.JsonData({
                "msg_id": msg_id,
                "original_send_time": original_send_time,
                "send_after_serialize_time": send_after_serialize_time,
                "node_b_receive_time": receive_time,
                "node_b_deserialize_done_time": deserialize_done_time,
                "node_b_serialize_start_time": serialize_start_time,
            })
            echo_metadata = aerosim_middleware.Metadata(
                self.return_topic, "aerosim::types::JsonData"
            )
            echo_payload = self.serializer.serialize_message(echo_metadata, echo_data)
            serialize_done_time = time.perf_counter()

            # Add the serialize time to the echo data by re-serializing (slight overhead for accuracy)
            echo_data = aerosim_types.JsonData({
                "msg_id": msg_id,
                "original_send_time": original_send_time,
                "send_after_serialize_time": send_after_serialize_time,
                "node_b_receive_time": receive_time,
                "node_b_deserialize_done_time": deserialize_done_time,
                "node_b_serialize_time_ms": (serialize_done_time - serialize_start_time) * 1000,
                "node_b_send_after_serialize_time": time.perf_counter(),
            })
            echo_payload = self.serializer.serialize_message(echo_metadata, echo_data)

            self.transport.publish_raw("aerosim::types::JsonData", self.return_topic, echo_payload)

    def _node_a_callback(self, payload: bytes):
        """
        Node A's callback: receives echo, calculates detailed latencies.
        """
        receive_time = time.perf_counter()

        # Deserialize and measure time
        _metadata, data = self.serializer.deserialize_message(aerosim_types.JsonData, payload)
        deserialize_done_time = time.perf_counter()

        json_data = data.get_data()

        msg_id = json_data.get("msg_id")
        original_send_time = json_data.get("original_send_time")
        send_after_serialize_time = json_data.get("send_after_serialize_time")
        node_b_receive_time = json_data.get("node_b_receive_time")
        node_b_deserialize_done_time = json_data.get("node_b_deserialize_done_time")
        node_b_serialize_time_ms = json_data.get("node_b_serialize_time_ms")
        node_b_send_after_serialize_time = json_data.get("node_b_send_after_serialize_time")

        required_fields = [
            msg_id, original_send_time, send_after_serialize_time,
            node_b_receive_time, node_b_deserialize_done_time,
            node_b_serialize_time_ms, node_b_send_after_serialize_time
        ]

        if all(v is not None for v in required_fields):
            # Skip warmup messages
            if msg_id >= self.warmup_messages:
                # Forward leg breakdown
                forward_transport = (node_b_receive_time - send_after_serialize_time) * 1000
                forward_deserialize = (node_b_deserialize_done_time - node_b_receive_time) * 1000
                forward_total = (node_b_deserialize_done_time - original_send_time) * 1000

                # Return leg breakdown
                return_serialize = node_b_serialize_time_ms
                return_transport = (receive_time - node_b_send_after_serialize_time) * 1000
                return_deserialize = (deserialize_done_time - receive_time) * 1000
                return_total = (deserialize_done_time - node_b_deserialize_done_time) * 1000

                # Round-trip total
                roundtrip_total = (deserialize_done_time - original_send_time) * 1000

                with self.lock:
                    # Total latencies
                    self.forward_latencies_ms.append(forward_total)
                    self.return_latencies_ms.append(return_total)
                    self.roundtrip_latencies_ms.append(roundtrip_total)

                    # Forward breakdown (serialize time stored per-message in send loop)
                    self.forward_transport_ms.append(forward_transport)
                    self.forward_deserialize_ms.append(forward_deserialize)

                    # Return breakdown
                    self.return_serialize_ms.append(return_serialize)
                    self.return_transport_ms.append(return_transport)
                    self.return_deserialize_ms.append(return_deserialize)

            with self.lock:
                self.messages_received += 1
                if self.messages_received >= self.num_messages + self.warmup_messages:
                    self.all_received.set()

    def run(self, timeout: float = 60.0) -> DetailedLatencyResult:
        """
        Run the detailed latency benchmark.

        Args:
            timeout: Maximum time to wait for all messages (seconds).

        Returns:
            DetailedLatencyResult with measured latencies and breakdown.
        """
        # Set up raw subscriptions with unique topics
        self.transport.subscribe_raw(
            "aerosim::types::JsonData", self.forward_topic, self._node_b_callback
        )
        self.transport.subscribe_raw(
            "aerosim::types::JsonData", self.return_topic, self._node_a_callback
        )

        # Allow subscriptions to establish
        time.sleep(1.0)

        # Generate payload
        payload_str = "x" * self.payload_size

        # Track forward serialize times separately
        forward_serialize_times: List[float] = []

        # Send messages (warmup + actual)
        total_messages = self.num_messages + self.warmup_messages
        for i in range(total_messages):
            send_time = time.perf_counter()

            message_data = aerosim_types.JsonData({
                "msg_id": i,
                "send_time": send_time,
                "payload": payload_str
            })
            metadata = aerosim_middleware.Metadata(
                self.forward_topic, "aerosim::types::JsonData"
            )

            # Measure serialization time
            serialize_start = time.perf_counter()
            payload = self.serializer.serialize_message(metadata, message_data)
            serialize_done = time.perf_counter()

            # Re-serialize with the timing info included
            message_data = aerosim_types.JsonData({
                "msg_id": i,
                "send_time": send_time,
                "send_after_serialize_time": serialize_done,
                "payload": payload_str
            })
            payload = self.serializer.serialize_message(metadata, message_data)

            if i >= self.warmup_messages:
                forward_serialize_times.append((serialize_done - serialize_start) * 1000)

            self.transport.publish_raw("aerosim::types::JsonData", self.forward_topic, payload)

            # Small delay between messages to avoid overwhelming the system
            time.sleep(0.02)

        # Wait for all messages to complete round-trip
        if not self.all_received.wait(timeout=timeout):
            print(f"Warning: Timeout waiting for messages. Received {self.messages_received}/{total_messages}")

        # Store forward serialize times (collected from send loop)
        self.forward_serialize_ms = forward_serialize_times[:len(self.forward_latencies_ms)]

        return DetailedLatencyResult(
            forward_latencies_ms=self.forward_latencies_ms.copy(),
            return_latencies_ms=self.return_latencies_ms.copy(),
            roundtrip_latencies_ms=self.roundtrip_latencies_ms.copy(),
            forward_serialize_ms=self.forward_serialize_ms.copy(),
            forward_transport_ms=self.forward_transport_ms.copy(),
            forward_deserialize_ms=self.forward_deserialize_ms.copy(),
            return_serialize_ms=self.return_serialize_ms.copy(),
            return_transport_ms=self.return_transport_ms.copy(),
            return_deserialize_ms=self.return_deserialize_ms.copy(),
        )


# Module-level transports to be shared across tests (singleton pattern)
_transports = {}


@pytest.fixture(scope="module")
def zenoh_transport():
    """
    Module-scoped fixture providing the Zenoh transport.

    The transport is created once and shared across all tests in the module.
    ZenohMiddleware is a singleton, so we don't close it between tests.
    """
    global _transports
    try:
        _transports["zenoh"] = aerosim_middleware.get_transport("zenoh")
        yield _transports["zenoh"]
    except (AttributeError, Exception) as e:
        pytest.skip(f"Zenoh middleware not available: {e}")


@pytest.fixture(scope="module")
def kafka_transport():
    """
    Module-scoped fixture providing the Kafka transport.

    The transport is created once and shared across all tests in the module.
    """
    global _transports
    try:
        _transports["kafka"] = aerosim_middleware.get_transport("kafka")
        yield _transports["kafka"]
    except (AttributeError, Exception) as e:
        pytest.skip(f"Kafka middleware not available: {e}")


@pytest.fixture(scope="module", params=["zenoh", "kafka"])
def transport(request):
    """
    Parameterized fixture providing transport for each middleware type.
    """
    transport_name = request.param
    global _transports
    try:
        if transport_name not in _transports:
            _transports[transport_name] = aerosim_middleware.get_transport(transport_name)
        return (transport_name, _transports[transport_name])
    except (AttributeError, Exception) as e:
        pytest.skip(f"{transport_name.capitalize()} middleware not available: {e}")


# =============================================================================
# Parameterized tests that run for all transports
# =============================================================================

def test_roundtrip_latency_benchmark(transport, capsys):
    """
    Benchmark test for publish-subscribe round-trip latency.

    This test:
    1. Sets up a publisher and subscriber on two topics
    2. Sends messages that are echoed back
    3. Measures and reports forward, return, and round-trip latencies
    """
    transport_name, transport_obj = transport
    benchmark = LatencyBenchmark(transport_obj, transport_name=transport_name,
                                  num_messages=500, warmup_messages=20)
    result = benchmark.run(timeout=120.0)

    # Print results (visible with pytest -s)
    result.print_summary(title=f"{transport_name.capitalize()} Publish-Subscribe Latency Benchmark Results")

    # Basic assertions to ensure the test ran correctly
    assert len(result.roundtrip_latencies_ms) > 0, "No round-trip latency measurements collected"
    assert len(result.forward_latencies_ms) > 0, "No forward latency measurements collected"
    assert len(result.return_latencies_ms) > 0, "No return latency measurements collected"

    # Sanity check: latencies should be positive and reasonable (< 1 second)
    avg_roundtrip = statistics.mean(result.roundtrip_latencies_ms)
    assert avg_roundtrip > 0, "Average round-trip latency should be positive"
    assert avg_roundtrip < 1000, f"Average round-trip latency too high: {avg_roundtrip:.3f} ms"


def test_latency_small_messages(transport, capsys):
    """Benchmark with small message payload (64 bytes)."""
    transport_name, transport_obj = transport
    benchmark = LatencyBenchmark(transport_obj, transport_name=transport_name,
                                  num_messages=200, warmup_messages=10, payload_size=64)
    result = benchmark.run(timeout=60.0)
    result.print_summary(title=f"{transport_name.capitalize()} Small Messages (64 bytes) Latency Benchmark")

    assert len(result.roundtrip_latencies_ms) >= 180, "Expected at least 180 measurements"


def test_latency_large_messages(transport, capsys):
    """Benchmark with large message payload (500 KB)."""
    transport_name, transport_obj = transport
    # 500 KB payload
    payload_size = 500 * 1024
    benchmark = LatencyBenchmark(transport_obj, transport_name=transport_name,
                                  num_messages=200, warmup_messages=10, payload_size=payload_size)
    result = benchmark.run(timeout=120.0)
    result.print_summary(title=f"{transport_name.capitalize()} Large Messages ({payload_size // 1024} KB) Latency Benchmark")

    assert len(result.roundtrip_latencies_ms) >= 180, "Expected at least 180 measurements"

    # Large messages will have higher latency, but should still complete
    avg_roundtrip = statistics.mean(result.roundtrip_latencies_ms)
    assert avg_roundtrip > 0, "Average round-trip latency should be positive"


@pytest.mark.parametrize("num_messages", [50, 200, 500])
def test_latency_varying_load(transport, num_messages, capsys):
    """Benchmark with varying number of messages to see if latency changes under load."""
    transport_name, transport_obj = transport
    benchmark = LatencyBenchmark(transport_obj, transport_name=transport_name,
                                  num_messages=num_messages, warmup_messages=10)
    result = benchmark.run(timeout=120.0)

    print(f"\n--- {transport_name.capitalize()} Results for {num_messages} messages ---")
    if result.roundtrip_latencies_ms:
        avg = statistics.mean(result.roundtrip_latencies_ms)
        print(f"Average round-trip latency: {avg:.3f} ms")

    assert len(result.roundtrip_latencies_ms) > 0


def test_latency_large_messages_bincode(transport, capsys):
    """Benchmark large messages with bincode serialization (compare to JSON)."""
    transport_name, transport_obj = transport
    payload_size = 500 * 1024  # 500 KB

    # First run with JSON (standard)
    print(f"\n--- {transport_name.capitalize()} JSON Serialization (500 KB payload) ---")
    benchmark_json = LatencyBenchmark(transport_obj, transport_name=transport_name,
                                       num_messages=100, warmup_messages=10, payload_size=payload_size)
    result_json = benchmark_json.run(timeout=120.0)
    result_json.print_summary(title=f"{transport_name.capitalize()} Large Messages - JSON Serialization (500 KB)")

    # Then run with Bincode
    print(f"\n--- {transport_name.capitalize()} Bincode Serialization (500 KB payload) ---")
    benchmark_bincode = LatencyBenchmarkBincode(transport_obj, transport_name=transport_name,
                                                  num_messages=100, warmup_messages=10, payload_size=payload_size)
    result_bincode = benchmark_bincode.run(timeout=120.0)
    result_bincode.print_summary(title=f"{transport_name.capitalize()} Large Messages - Bincode Serialization (500 KB)")

    # Print comparison
    if result_json.roundtrip_latencies_ms and result_bincode.roundtrip_latencies_ms:
        avg_json = statistics.mean(result_json.roundtrip_latencies_ms)
        avg_bincode = statistics.mean(result_bincode.roundtrip_latencies_ms)
        improvement = ((avg_json - avg_bincode) / avg_json) * 100

        print("\n" + "=" * 60)
        print(f"{transport_name.capitalize()} Serialization Comparison (500 KB payload)")
        print("=" * 60)
        print(f"  JSON avg round-trip:    {avg_json:.3f} ms")
        print(f"  Bincode avg round-trip: {avg_bincode:.3f} ms")
        print(f"  Difference:             {avg_json - avg_bincode:.3f} ms ({improvement:+.1f}%)")
        print("=" * 60)

    assert len(result_json.roundtrip_latencies_ms) > 0
    assert len(result_bincode.roundtrip_latencies_ms) > 0


def test_latency_detailed_breakdown(transport, capsys):
    """Benchmark with detailed serialization vs transport time breakdown."""
    transport_name, transport_obj = transport
    payload_size = 500 * 1024  # 500 KB

    # Run detailed benchmark with transport-specific JSON serializer
    print(f"\n--- {transport_name.capitalize()} JSON Serializer Detailed Breakdown (500 KB payload) ---")
    json_serializer = get_serializer(transport_name)
    benchmark_json = DetailedLatencyBenchmark(
        transport_obj,
        transport_name=transport_name,
        serializer=json_serializer,
        serializer_name="JSON",
        num_messages=100,
        warmup_messages=10,
        payload_size=payload_size
    )
    result_json = benchmark_json.run(timeout=120.0)
    result_json.print_summary(title=f"{transport_name.capitalize()} JSON Serializer - Detailed Breakdown (500 KB)")

    # Run detailed benchmark with Bincode serializer
    print(f"\n--- {transport_name.capitalize()} Bincode Serializer Detailed Breakdown (500 KB payload) ---")
    bincode_serializer = aerosim_middleware.BincodeSerializer()
    benchmark_bincode = DetailedLatencyBenchmark(
        transport_obj,
        transport_name=transport_name,
        serializer=bincode_serializer,
        serializer_name="Bincode",
        num_messages=100,
        warmup_messages=10,
        payload_size=payload_size
    )
    result_bincode = benchmark_bincode.run(timeout=120.0)
    result_bincode.print_summary(title=f"{transport_name.capitalize()} Bincode Serializer - Detailed Breakdown (500 KB)")

    # Print comparison summary
    if result_json.roundtrip_latencies_ms and result_bincode.roundtrip_latencies_ms:
        print("\n" + "=" * 70)
        print(f"{transport_name.capitalize()} Serialization vs Transport Comparison (500 KB payload)")
        print("=" * 70)

        # JSON totals
        json_serialize = (
            statistics.mean(result_json.forward_serialize_ms) +
            statistics.mean(result_json.return_serialize_ms)
        )
        json_transport_time = (
            statistics.mean(result_json.forward_transport_ms) +
            statistics.mean(result_json.return_transport_ms)
        )
        json_deserialize = (
            statistics.mean(result_json.forward_deserialize_ms) +
            statistics.mean(result_json.return_deserialize_ms)
        )
        json_total = statistics.mean(result_json.roundtrip_latencies_ms)

        # Bincode totals
        bincode_serialize = (
            statistics.mean(result_bincode.forward_serialize_ms) +
            statistics.mean(result_bincode.return_serialize_ms)
        )
        bincode_transport_time = (
            statistics.mean(result_bincode.forward_transport_ms) +
            statistics.mean(result_bincode.return_transport_ms)
        )
        bincode_deserialize = (
            statistics.mean(result_bincode.forward_deserialize_ms) +
            statistics.mean(result_bincode.return_deserialize_ms)
        )
        bincode_total = statistics.mean(result_bincode.roundtrip_latencies_ms)

        print(f"\n{'Component':<20} {'JSON (ms)':<15} {'Bincode (ms)':<15} {'Diff (ms)':<15}")
        print("-" * 65)
        print(f"{'Serialize':<20} {json_serialize:<15.3f} {bincode_serialize:<15.3f} {json_serialize - bincode_serialize:<+15.3f}")
        print(f"{'Transport':<20} {json_transport_time:<15.3f} {bincode_transport_time:<15.3f} {json_transport_time - bincode_transport_time:<+15.3f}")
        print(f"{'Deserialize':<20} {json_deserialize:<15.3f} {bincode_deserialize:<15.3f} {json_deserialize - bincode_deserialize:<+15.3f}")
        print("-" * 65)
        print(f"{'Total Round-Trip':<20} {json_total:<15.3f} {bincode_total:<15.3f} {json_total - bincode_total:<+15.3f}")
        print("=" * 70)

    assert len(result_json.roundtrip_latencies_ms) > 0
    assert len(result_bincode.roundtrip_latencies_ms) > 0


if __name__ == "__main__":
    # Allow running directly for quick testing
    import sys

    # Default to zenoh, but allow specifying transport via command line
    transport_name = sys.argv[1] if len(sys.argv) > 1 else "zenoh"

    print(f"Running {transport_name.capitalize()} latency benchmark...")
    transport_obj = aerosim_middleware.get_transport(transport_name)

    # Small messages benchmark
    print("\n*** Small Messages (64 bytes) ***")
    benchmark = LatencyBenchmark(transport_obj, transport_name=transport_name,
                                  num_messages=200, warmup_messages=10, payload_size=64)
    result = benchmark.run()
    result.print_summary(title=f"{transport_name.capitalize()} Small Messages (64 bytes)")

    # Large messages benchmark
    print("\n*** Large Messages (500 KB) ***")
    benchmark = LatencyBenchmark(transport_obj, transport_name=transport_name,
                                  num_messages=100, warmup_messages=10, payload_size=500 * 1024)
    result = benchmark.run(timeout=120.0)
    result.print_summary(title=f"{transport_name.capitalize()} Large Messages (500 KB)")
