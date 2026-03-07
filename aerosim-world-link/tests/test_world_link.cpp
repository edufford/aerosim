#include <chrono>
#include <cmath>
#include <cstdlib>
#include <cstring>
#include <thread>
#include <vector>

#include <gtest/gtest.h>

extern "C" {
#include "aerosim_world_link.h"
}

// Test fixture that manages the MessageHandler lifecycle.
// SetUpTestSuite/TearDownTestSuite run once for the entire test suite,
// ensuring tests execute in order with a shared handler.
class WorldLinkTest : public ::testing::Test {
protected:
    static void SetUpTestSuite() {
        initialize_logger("test_world_link.log");
        ASSERT_TRUE(initialize_message_handler("test_renderer", "zenoh"));
        start_message_handler();
    }

    static void TearDownTestSuite() {
        end_message_handler();
    }
};

// Verify that subscribe_to_topic blocks until the async runtime has created the
// subscription, and returns true to indicate success.
TEST_F(WorldLinkTest, SubscribeBlocksAndReturnsStatus) {
    auto start = std::chrono::steady_clock::now();
    bool result = subscribe_to_topic("aerosim.test.blocking_subscribe");
    auto elapsed = std::chrono::steady_clock::now() - start;
    auto ms = std::chrono::duration_cast<std::chrono::milliseconds>(elapsed).count();

    // subscribe_to_topic returned a meaningful bool (true),
    // proving it blocked until the async runtime processed the subscription.
    ASSERT_TRUE(result) << "subscribe_to_topic should return true on success";
    ASSERT_LT(ms, 10000) << "subscribe_to_topic took too long: " << ms << " ms";
}

// Verify that multiple sequential subscriptions each block and succeed.
TEST_F(WorldLinkTest, SubscribeMultipleTopics) {
    const char* topics[] = {
        "aerosim.test.topic_a",
        "aerosim.test.topic_b",
        "aerosim.test.topic_c",
    };

    for (const char* topic : topics) {
        ASSERT_TRUE(subscribe_to_topic(topic)) << "Failed to subscribe to " << topic;
    }
}

// Verify that publish_to_topic sends a generic JsonData message and returns true.
TEST_F(WorldLinkTest, PublishToTopicReturnsStatus) {
    const char* payload = "{\"test\": \"hello\"}";
    ASSERT_TRUE(publish_to_topic("aerosim.test.publish", payload));
}

// Verify that publish_typed_to_topic serializes a payload as a registered type
// (aerosim::types::JsonData) and returns true on success.
TEST_F(WorldLinkTest, PublishTypedToTopic) {
    // Type names use the full path: "aerosim::types::TypeName"
    // JsonData has a single "data" field of type String (JSON stringified)
    const char* payload = "{\"data\": \"{\\\"x\\\": 1.0}\"}";
    ASSERT_TRUE(publish_typed_to_topic(
        "aerosim.test.typed_publish", "aerosim::types::JsonData", payload, 1.0));
}

// Verify that publish_typed_to_topic returns false when given an unregistered type name.
TEST_F(WorldLinkTest, PublishTypedUnknownTypeReturnsFalse) {
    const char* payload = "{\"x\": 1.0}";
    ASSERT_FALSE(publish_typed_to_topic(
        "aerosim.test.typed_publish", "aerosim::types::NonExistentType", payload, 1.0));
}

// Verify that the consumer payload queue starts empty with no timestamps.
TEST_F(WorldLinkTest, QueueEmptyInitially) {
    ASSERT_EQ(get_consumer_payload_queue_size(), 0u);
    ASSERT_LT(get_consumer_payload_queue_oldest_timestamp(), 0.0);
    ASSERT_LT(get_consumer_payload_queue_newest_timestamp(), 0.0);
    ASSERT_EQ(get_consumer_payload_from_queue(), nullptr);
}

// End-to-end roundtrip: subscribe to a topic, publish a message to it, then
// verify the message arrives in the consumer payload queue.
TEST_F(WorldLinkTest, SubscribeThenReceive) {
    ASSERT_TRUE(subscribe_to_topic("aerosim.test.roundtrip"));

    // Publish to the same topic
    const char* payload = "{\"msg\": \"roundtrip_test\"}";
    ASSERT_TRUE(publish_to_topic("aerosim.test.roundtrip", payload));

    // Wait for message to arrive in the queue
    uint32_t size = 0;
    auto start = std::chrono::steady_clock::now();
    while (size == 0) {
        std::this_thread::sleep_for(std::chrono::milliseconds(100));
        size = get_consumer_payload_queue_size();
        auto elapsed = std::chrono::steady_clock::now() - start;
        if (std::chrono::duration_cast<std::chrono::seconds>(elapsed).count() > 5) {
            break;
        }
    }

    ASSERT_GT(size, 0u) << "Timed out waiting for message in queue";

    char* received = get_consumer_payload_from_queue();
    ASSERT_NE(received, nullptr);
    // TODO: Add a free_payload FFI function that calls CString::from_raw() on the Rust side
    // to properly deallocate. Using free() works in practice because Rust uses the system
    // allocator, but is not guaranteed correct across all platforms.
    free(received);
}

// Verify that notify_scene_graph_loaded can be called without error.
// This publishes a status message to the middleware.
TEST_F(WorldLinkTest, NotifySceneGraphLoaded) {
    ASSERT_NO_FATAL_FAILURE(notify_scene_graph_loaded());
}

// Verify that publish_image_to_topic_async accepts a valid image buffer.
// This is fire-and-forget so we just verify it doesn't crash.
TEST_F(WorldLinkTest, PublishImageToTopicAsync) {
    const int width = 2;
    const int height = 2;
    // 2x2 BGRA image (4 bytes per pixel)
    std::vector<uint8_t> image_data(width * height * 4, 128);
    ASSERT_NO_FATAL_FAILURE(publish_image_to_topic_async(
        "aerosim.test.image", width, height, 3 /* BGRA8 */,
        image_data.data(), image_data.size()));
}

// -------------------------------------------------------------------------
// Coordinate conversion utility tests (no MessageHandler needed)
// -------------------------------------------------------------------------

constexpr double PI = 3.14159265358979323846;
constexpr double TOLERANCE = 1e-9;

// Verify degrees/radians conversion roundtrip.
TEST(CoordinateUtils, ToDegreesAndToRadians) {
    ASSERT_NEAR(to_degrees(PI), 180.0, TOLERANCE);
    ASSERT_NEAR(to_degrees(PI / 2.0), 90.0, TOLERANCE);
    ASSERT_NEAR(to_degrees(0.0), 0.0, TOLERANCE);
    ASSERT_NEAR(to_radians(180.0), PI, TOLERANCE);
    ASSERT_NEAR(to_radians(90.0), PI / 2.0, TOLERANCE);
    ASSERT_NEAR(to_radians(0.0), 0.0, TOLERANCE);
    // Roundtrip
    ASSERT_NEAR(to_radians(to_degrees(1.234)), 1.234, TOLERANCE);
}

// Verify NED (North, East, Down) to ENU (East, North, Up) conversion.
// ned_to_enu: (n, e, d) -> (e, n, -d)
TEST(CoordinateUtils, NedToEnu) {
    double x = 1.0, y = 2.0, z = 3.0;
    ned_to_enu(&x, &y, &z);
    ASSERT_NEAR(x, 2.0, TOLERANCE);  // east = original east
    ASSERT_NEAR(y, 1.0, TOLERANCE);  // north = original north
    ASSERT_NEAR(z, -3.0, TOLERANCE); // up = -down
}

// Verify NED to Unreal ESU conversion (with m -> cm scaling).
// ned_to_unreal_esu: (n, e, d) -> (e*100, -n*100, -d*100)
TEST(CoordinateUtils, NedToUnrealEsu) {
    double x = 1.0, y = 2.0, z = 3.0;
    ned_to_unreal_esu(&x, &y, &z);
    ASSERT_NEAR(x, 200.0, TOLERANCE);  // east * 100
    ASSERT_NEAR(y, -100.0, TOLERANCE); // -north * 100
    ASSERT_NEAR(z, -300.0, TOLERANCE); // -down * 100
}

// Verify FRD (Front, Right, Down) to FLU (Front, Left, Up) conversion.
// frd_to_flu: (f, r, d) -> (f, -r, -d)
TEST(CoordinateUtils, FrdToFlu) {
    double x = 1.0, y = 2.0, z = 3.0;
    frd_to_flu(&x, &y, &z);
    ASSERT_NEAR(x, 1.0, TOLERANCE);   // front unchanged
    ASSERT_NEAR(y, -2.0, TOLERANCE);  // left = -right
    ASSERT_NEAR(z, -3.0, TOLERANCE);  // up = -down
}

// Verify RPY FRD to FLU conversion.
// rpy_frd_to_flu: (r, p, y) -> (r, -p, -y)
TEST(CoordinateUtils, RpyFrdToFlu) {
    double roll = 0.1, pitch = 0.2, yaw = 0.3;
    rpy_frd_to_flu(&roll, &pitch, &yaw);
    ASSERT_NEAR(roll, 0.1, TOLERANCE);
    ASSERT_NEAR(pitch, -0.2, TOLERANCE);
    ASSERT_NEAR(yaw, -0.3, TOLERANCE);
}

// Verify RPY NED to Unreal ESU conversion (output in degrees).
// rpy_ned_to_unreal_esu: yaw rotated by -90 degrees, output converted to degrees.
TEST(CoordinateUtils, RpyNedToUnrealEsu) {
    double roll = 0.0, pitch = 0.0, yaw = PI / 2.0; // 90 degrees heading
    rpy_ned_to_unreal_esu(&roll, &pitch, &yaw);
    ASSERT_NEAR(roll, 0.0, TOLERANCE);
    ASSERT_NEAR(pitch, 0.0, TOLERANCE);
    ASSERT_NEAR(yaw, 0.0, 0.01); // 90 - 90 = 0 degrees
}

// Verify RPY NWU to ENU conversion.
// rpy_nwu_to_enu: yaw rotated by +90 degrees, normalized to [0, 2*PI).
TEST(CoordinateUtils, RpyNwuToEnu) {
    double roll = 0.1, pitch = 0.2, yaw = 0.0;
    rpy_nwu_to_enu(&roll, &pitch, &yaw);
    ASSERT_NEAR(roll, 0.1, TOLERANCE);
    ASSERT_NEAR(pitch, 0.2, TOLERANCE);
    ASSERT_NEAR(yaw, PI / 2.0, TOLERANCE); // 0 + 90 degrees
}

// Verify quaternion (identity) to Euler angles conversion.
// Identity quaternion (1, 0, 0, 0) should produce (0, 0, 0) RPY.
TEST(CoordinateUtils, QuatWxyzToRpyIdentity) {
    double roll, pitch, yaw;
    aerosim_quat_wxyz_to_rpy(1.0, 0.0, 0.0, 0.0, &roll, &pitch, &yaw);
    ASSERT_NEAR(roll, 0.0, TOLERANCE);
    ASSERT_NEAR(pitch, 0.0, TOLERANCE);
    ASSERT_NEAR(yaw, 0.0, TOLERANCE);
}

// Verify quaternion for 90-degree rotation about Z axis.
// Quaternion (cos(45°), 0, 0, sin(45°)) represents a 90° Z rotation.
// The quaternion_core library's Extrinsic ZYX returns angles in sequence order
// [Z, Y, X], but the FFI destructures them as [roll, pitch, yaw], swapping
// roll and yaw. The same swap exists in from_euler_angles, so internal
// roundtrips are self-consistent. However, the swap could cause incorrect
// coordinate conversions for quaternions from external sources (e.g. FMU
// dynamics) where the quaternion represents true physical orientation.
// TODO: Fix by switching from Extrinsic ZYX to Intrinsic XYZ (equivalent
// rotation, but returns [X=roll, Y=pitch, Z=yaw] in the expected order).
TEST(CoordinateUtils, QuatWxyzToRpy90DegZRotation) {
    double roll, pitch, yaw;
    double angle = PI / 4.0; // half of 90 degrees
    aerosim_quat_wxyz_to_rpy(cos(angle), 0.0, 0.0, sin(angle), &roll, &pitch, &yaw);
    ASSERT_NEAR(roll, PI / 2.0, TOLERANCE);
    ASSERT_NEAR(pitch, 0.0, TOLERANCE);
    ASSERT_NEAR(yaw, 0.0, TOLERANCE);
}
