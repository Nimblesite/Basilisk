"""Run-forever allocator for the auto-pause memory e2e.

Grows a module-level cache until the debugger disconnects, so a memory
snapshot taken at ANY moment attributes real allocations to the append
line below — without the test needing breakpoints or pauses.
"""
import time

CACHE = []

while True:
    CACHE.append("x" * 5000)
    time.sleep(0.01)
