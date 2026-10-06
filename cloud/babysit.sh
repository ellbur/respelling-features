#!/bin/bash
# Keeps a spot-VM job going: whenever its VM disappears before the job has
# finished (a preemption), relaunches it from the latest job's checkpoints.
#
#   cloud/babysit.sh <job> <name> <machine-type> <max-hours> -- <command>
#
# <job> is the running job to follow; the rest is as for cloud/launch.sh and
# is used for relaunches. Stops when the job's log shows it finished, when it
# crashed (so it isn't relaunched into the same failure), or after
# MAX_RELAUNCHES relaunches. Checks every 5 minutes; logs to
# working/babysit.log.

set -u

PROJECT=${PROJECT:?set PROJECT to the GCP project to run in}
BUCKET=${BUCKET:-gs://$PROJECT-jobs}
MAX_RELAUNCHES=${MAX_RELAUNCHES:-20}

JOB=$1
shift
LAUNCH_ARGS=("$@")

HERE=$(cd "$(dirname "$0")/.." && pwd)
LOG=$HERE/working/babysit.log
say() { echo "$(date -u +%FT%TZ) $*" >> "$LOG"; }

say "following $JOB"
relaunches=0
while true; do
  sleep 300
  # A failed check (network, API hiccup) says nothing about the VM; only an
  # answer that it isn't there counts as gone.
  if ! vms=$(gcloud compute instances list --project "$PROJECT" --filter="name=$JOB" --format="value(name)" 2>/dev/null); then
    say "couldn't check on $JOB; trying again later"
    continue
  fi
  if [ -n "$vms" ]; then
    continue
  fi

  joblog=$(gcloud storage cat "$BUCKET/$JOB/log.txt" 2>/dev/null)
  if echo "$joblog" | grep -q "^=== done"; then
    say "$JOB finished; stopping"
    exit 0
  fi
  if echo "$joblog" | grep -qE "panicked|^error"; then
    say "$JOB crashed; stopping without relaunching"
    exit 1
  fi
  if [ "$relaunches" -ge "$MAX_RELAUNCHES" ]; then
    say "$JOB gone, but already relaunched $relaunches times; stopping"
    exit 1
  fi

  say "$JOB gone before finishing ($(gcloud compute operations list --project "$PROJECT" --filter="targetLink~$JOB" --format="value(operationType)" 2>/dev/null | tr '\n' ' ')); relaunching"
  out=$(RESUME=$JOB "$HERE/cloud/launch.sh" "${LAUNCH_ARGS[@]}" 2>&1)
  new=$(echo "$out" | sed -n 's/^Started //p')
  if [ -z "$new" ]; then
    say "relaunch failed: $(echo "$out" | tail -3 | tr '\n' ' ')"
    continue
  fi
  relaunches=$((relaunches + 1))
  JOB=$new
  say "now following $JOB"
done
