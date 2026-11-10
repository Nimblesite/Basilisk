"""Fixture that runs until the debugger disconnects.

Used by the running-not-paused probes: the program must still be alive
(and NOT stopped at any breakpoint) whenever the test interrogates the
debug session, so it just sleeps in a loop until stopDebugging() kills it.
"""
import time

while True:
    time.sleep(0.05)
