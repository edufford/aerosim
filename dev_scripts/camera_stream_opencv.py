from aerosim_data import types as aerosim_types
from aerosim_data import middleware

import cv2
import numpy as np
import argparse
from collections import deque
from pathlib import Path
from datetime import datetime

# Parse command-line arguments
parser = argparse.ArgumentParser(description="Camera stream viewer with image capture")
parser.add_argument(
    "--transport",
    type=str,
    choices=["zenoh", "kafka"],
    default="zenoh",
    help="Middleware transport to use (default: zenoh)"
)
args = parser.parse_args()

image_queue = deque(maxlen=1)
serializer = middleware.BincodeSerializer()

cv2.namedWindow("Camera Preview")

# Setup output directory for saved images
output_dir = Path("camera_captures")
output_dir.mkdir(exist_ok=True)

# Configuration for auto-saving
AUTO_SAVE = False  # Set to False to only save on 's' keypress
save_counter = 0


def on_sensor_data(payload):

    _, data = serializer.deserialize_message(aerosim_types.CompressedImage, payload)

    # Convert bytes to NumPy array
    image_array = np.frombuffer(data.data, dtype=np.uint8)

    image_queue.append(cv2.imdecode(image_array, cv2.IMREAD_COLOR))


# Set up middleware transport and subscribe to vehicle state
transport = middleware.get_transport(args.transport)
transport.subscribe_raw(
    "aerosim::types::CompressedImage", "aerosim.renderer.responses", on_sensor_data
)

print(f"Camera stream viewer started")
print(f"Transport: {args.transport}")
print(f"Images will be saved to: {output_dir.absolute()}")
print("Controls:")
print("  's' - Save current frame")
print("  ESC - Exit")
print(f"  AUTO_SAVE is {'ENABLED' if AUTO_SAVE else 'DISABLED'}")
print()

while True:
    if image_queue:

        image_rgb = image_queue.pop()
        cv2.imshow("Camera Preview", image_rgb)

        # Auto-save if enabled
        if AUTO_SAVE:
            timestamp = datetime.now().strftime("%Y%m%d_%H%M%S_%f")
            filename = output_dir / f"frame_{save_counter:05d}_{timestamp}.png"
            cv2.imwrite(str(filename), image_rgb)
            print(f"Saved: {filename.name}")
            save_counter += 1

    # Exit when pressing ESC
    key = cv2.waitKey(20)
    if key == 27:
        break
    elif key == ord('s'):
        # Manual save on 's' keypress
        if image_queue or 'image_rgb' in locals():
            timestamp = datetime.now().strftime("%Y%m%d_%H%M%S_%f")
            filename = output_dir / f"manual_{save_counter:05d}_{timestamp}.png"
            cv2.imwrite(str(filename), image_rgb)
            print(f"Manually saved: {filename.name}")
            save_counter += 1
