import shutil

# Use "/" on Linux/macOS, or "C:\\" on Windows
total, used, free = shutil.disk_usage("C:\\")

# Convert bytes to Gigabytes (GiB)
gb = 2**30  # Use 10**9 for decimal GB

print(f"Total: {total / gb:.2f} GB")
print(f"Used:  {used / gb:.2f} GB")
print(f"Free:  {free / gb:.2f} GB")
