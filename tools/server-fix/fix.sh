#!/usr/bin/env bash
# Server fix (owner OK for fixes; restarts fine with no players). 2 Nov sweep: the streamed-game host (wu-play,
# wenilla-host) was killed once in 24 h by its own 256 MB cap; it gets 512 MB, live (docker update, no restart).
# stream-up.sh and slice-serve.sh now start it with 512 MB too. No secrets printed.
set -uo pipefail
echo "== 1. wu-play memory"
if docker inspect wu-play >/dev/null 2>&1; then
  before=$(docker inspect -f '{{.HostConfig.Memory}}' wu-play)
  docker update --memory 512m --memory-swap 1g wu-play >/dev/null && echo "  wu-play: $((before / 1048576)) MB -> $(( $(docker inspect -f '{{.HostConfig.Memory}}' wu-play) / 1048576 )) MB (no restart)"
  echo "  in use: $(docker stats --no-stream --format '{{.MemUsage}}' wu-play 2>/dev/null)"
else
  echo "  no wu-play container here"
fi
echo "== 2. memory now"
free -m | sed 's/^/  /'
echo "SERVER FIX DONE"
