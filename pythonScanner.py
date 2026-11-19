import heapq
import json
import os
import time
import tracemalloc

TARGET_DRIVE = "D:\\"
OUTPUT_FILE = "python_result.json"


def scan_drive(root_path: str):
    tracemalloc.start()
    start_time = time.perf_counter()

    total_files = 0
    total_dirs = 0
    errors = 0
    # Max-heap via inverted wrapper or just collect & nsmallest;
    # keeping a bounded list of 5 smallest avoids sorting everything.
    smallest_five = []

    stack = [root_path]
    while stack:
        current_dir = stack.pop()
        try:
            with os.scandir(current_dir) as it:
                for entry in it:
                    try:
                        if entry.is_dir(follow_symlinks=False):
                            total_dirs += 1
                            stack.append(entry.path)
                        elif entry.is_file(follow_symlinks=False):
                            total_files += 1
                            name = entry.name
                            if len(smallest_five) < 5:
                                smallest_five.append(name)
                                smallest_five.sort()
                            elif name < smallest_five[-1]:
                                smallest_five[-1] = name
                                smallest_five.sort()
                    except OSError:
                        errors += 1
        except OSError:
            errors += 1

    elapsed_ms = (time.perf_counter() - start_time) * 1000.0
    _, peak_mem_bytes = tracemalloc.get_traced_memory()
    tracemalloc.stop()

    files_per_sec = (total_files / (elapsed_ms / 1000.0)) if elapsed_ms > 0 else 0

    result = {
        "language": "Python",
        "drive": root_path,
        "elapsed_ms": round(elapsed_ms, 3),
        "total_files": total_files,
        "total_dirs": total_dirs,
        "errors_skipped": errors,
        "files_per_sec": round(files_per_sec, 1),
        "peak_python_heap_kb": round(peak_mem_bytes / 1024.0, 2),
        "top_5_files": smallest_five,
    }

    with open(OUTPUT_FILE, "w", encoding="utf-8") as f:
        json.dump(result, f, indent=2)

    print(f"[Python] Scanned {total_files:,} files in {elapsed_ms:.2f} ms -> saved to {OUTPUT_FILE}")


if __name__ == "__main__":
    scan_drive(TARGET_DRIVE)