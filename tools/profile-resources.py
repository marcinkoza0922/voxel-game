#!/usr/bin/env python3
"""Sample Linux process CPU/RAM and NVIDIA device/process memory every 0.5s.

Usage: profile-resources.py OUTPUT.csv COMMAND [ARG ...]
GPU utilization is device-wide; gpu_process_mib belongs to the child PID.
"""
import csv
import os
from pathlib import Path
import subprocess
import sys
import time


def query(args):
    result = subprocess.run(["nvidia-smi", *args, "--format=csv,noheader,nounits"],
                            capture_output=True, text=True, timeout=5)
    return result.stdout.strip() if result.returncode == 0 else ""


def main():
    if len(sys.argv) < 3:
        raise SystemExit(__doc__)
    ticks = os.sysconf("SC_CLK_TCK")
    start = previous = time.monotonic()
    previous_cpu = 0
    with open(sys.argv[1], "w", newline="") as output:
        writer = csv.writer(output)
        writer.writerow(["seconds", "cpu_percent", "rss_mib", "rss_peak_mib",
                         "gpu_util_percent", "gpu_memory_util_percent",
                         "gpu_device_used_mib", "gpu_power_w", "gpu_process_mib"])
        child = subprocess.Popen(sys.argv[2:])
        try:
            while child.poll() is None:
                now = time.monotonic()
                try:
                    stat = Path(f"/proc/{child.pid}/stat").read_text().rsplit(")", 1)[1].split()
                    cpu = (int(stat[11]) + int(stat[12])) / ticks
                    status = dict(line.split(":", 1) for line in
                                  Path(f"/proc/{child.pid}/status").read_text().splitlines())
                    rss = float(status.get("VmRSS", "0").split()[0]) / 1024
                    peak = float(status.get("VmHWM", "0").split()[0]) / 1024
                except (FileNotFoundError, ProcessLookupError):
                    break
                gpu = query(["--query-gpu=utilization.gpu,utilization.memory,memory.used,power.draw", "-i", "0"])
                fields = [value.strip() for value in gpu.split(",")] if gpu else [""] * 4
                processes = query(["--query-compute-apps=pid,used_gpu_memory"])
                memory = ""
                for line in processes.splitlines():
                    pid, amount = line.split(",", 1)
                    if pid.strip() == str(child.pid):
                        memory = amount.strip()
                writer.writerow([round(now - start, 3),
                                 round(100 * (cpu - previous_cpu) / (now - previous), 2),
                                 round(rss, 2), round(peak, 2), *fields, memory])
                output.flush()
                previous, previous_cpu = now, cpu
                time.sleep(max(0, 0.5 - (time.monotonic() - now)))
        finally:
            if child.poll() is None:
                child.terminate()
            code = child.wait()
    return code


if __name__ == "__main__":
    sys.exit(main())
