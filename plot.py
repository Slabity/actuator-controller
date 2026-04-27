#!/usr/bin/env python3
"""
Live plotter for actuator-controller position over USB serial.

Usage:
    python3 plot.py [PORT]

PORT defaults to /dev/ttyACM0. The firmware outputs one integer per
line: the ratiometric position scaled 0–10000.
"""

import sys
import threading
import collections
import queue

import serial
import matplotlib.pyplot as plt
import matplotlib.animation as animation

PORT = sys.argv[1] if len(sys.argv) > 1 else "/dev/ttyACM0"
BAUD = 115200
WINDOW = 500

q = queue.Queue()


def reader():
    while True:
        try:
            with serial.Serial(PORT, BAUD, timeout=1) as ser:
                print(f"Connected to {PORT}", flush=True)
                while True:
                    line = ser.readline().decode("ascii", errors="ignore").strip()
                    try:
                        q.put(int(line))
                    except ValueError:
                        pass
        except serial.SerialException as e:
            print(f"Serial error: {e} — retrying...", flush=True)


threading.Thread(target=reader, daemon=True).start()

data = collections.deque([0] * WINDOW, maxlen=WINDOW)
xs = list(range(WINDOW))

fig, ax = plt.subplots()
(line,) = ax.plot(xs, list(data))
ax.set_ylabel("Position (0–10000)")
ax.set_xlabel("Sample")
ax.set_title("Actuator Position")


def update(_frame):
    changed = False
    while not q.empty():
        data.append(q.get_nowait())
        changed = True
    if changed:
        line.set_ydata(list(data))
        ax.relim()
        ax.autoscale_view()
    return (line,)


ani = animation.FuncAnimation(
    fig, update, interval=50, blit=False, cache_frame_data=False
)
plt.tight_layout()
plt.show()
