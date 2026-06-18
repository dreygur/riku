#!/bin/sh
# Long-running fixture worker for the dashboard E2E suite.
# Prints its own PID once so the test can correlate it against the
# supervisor's reported PID, then heartbeats forever until killed.
set -eu
echo "FIXTURE_WEB_PID=$$"
i=0
while true; do
  i=$((i + 1))
  echo "heartbeat ${i} $(date +%s)"
  sleep 1
done
