"""Decode the (downscaled) burned-in counters of all 6 slots in an exported
composite to verify camera placement and inter-camera sync.

Usage: python3 -I check-export.py <export.mp4> [every_nth_frame]
"""
import subprocess
import sys

W, H = 1280, 720
SCALE = 0.225  # 1280x960 contain-fit into a 216 px tall slot
# (camera index, fitted x, fitted y) from layout.rs CAMERA_SLOTS + contain_rect
SLOTS = [(0, 496.0, 12.0), (2, 60.0, 180.0), (4, 932.0, 180.0),
         (3, 60.0, 492.0), (1, 496.0, 492.0), (5, 932.0, 492.0)]


def decode(frame, fx, fy):
    def white(bx, by):
        x = int(round(fx + (bx * 16 + 8) * SCALE - 0.5))
        y = int(round(fy + (by * 16 + 8) * SCALE - 0.5))
        return frame[y * W + x] >= 128
    bits = [white(k, 0) for k in range(17)]
    value = sum(1 << k for k in range(16) if bits[k])
    if sum(bits[:16]) % 2 != bits[16]:
        return None, None
    cam = sum(1 << k for k in range(3) if white(k, 1)) - 1
    return value, cam


def main():
    path = sys.argv[1]
    nth = int(sys.argv[2]) if len(sys.argv) > 2 else 15
    proc = subprocess.Popen(
        ["ffmpeg", "-hide_banner", "-loglevel", "error", "-i", path, "-vf",
         f"select='not(mod(n\\,{nth}))',format=gray", "-vsync", "0", "-f", "rawvideo", "-"],
        stdout=subprocess.PIPE)
    size = W * H
    index = 0
    spreads = {}
    lag = []
    bad = 0
    cam_mismatch = 0
    while True:
        frame = proc.stdout.read(size)
        if len(frame) < size:
            break
        n = index * nth
        expected = int((n / 30.0) * 36 + 1e-6)
        values = []
        for cam, fx, fy in SLOTS:
            value, got_cam = decode(frame, fx, fy)
            if value is None:
                bad += 1
                continue
            if got_cam != cam:
                cam_mismatch += 1
            values.append(value)
        if len(values) == 6:
            spread = max(values) - min(values)
            spreads[spread] = spreads.get(spread, 0) + 1
            lag.append(expected - min(values))
        index += 1
    total = sum(spreads.values())
    print(f"CHECK frames_sampled={index} fully_decoded={total} decode_fail_slots={bad} "
          f"cam_mismatch={cam_mismatch} spread_hist={dict(sorted(spreads.items()))} "
          f"lag(expected-shown) min={min(lag) if lag else None} max={max(lag) if lag else None}")


main()
