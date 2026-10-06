#!/bin/bash
# Runs a job on a GCP spot VM that deletes itself when done.
#
#   cloud/launch.sh <name> <machine-type> <max-hours> -- <command>
#
# The command runs in the job directory after building, e.g.
#   cloud/launch.sh test c2d-highcpu-32 2 -- ./target/release/secondpasstrial --words 300 --rules 30 --exact
#   cloud/launch.sh scaling c2d-highcpu-32 1 -- bash cloud/jobs/scaling.sh
# (It goes through instance metadata, so it can't contain commas.)
#
# RESUME=<earlier job> starts from that job's saved working/ (its
# checkpoints), e.g. after a spot VM was reclaimed:
#   RESUME=build-20260927-180000 cloud/launch.sh build c2d-highcpu-32 12 -- ...
#
# Uploads this directory (without target/) to the jobs bucket and starts a VM
# whose startup script (cloud/startup.sh) builds the code, runs the job, copies
# the output and working/ to gs://<bucket>/<job>/, and deletes the VM. The log
# is copied to the bucket every minute while it runs. As a backstop, GCP
# deletes the VM after <max-hours> whatever happens.
#
# PROJECT (required) is the GCP project; BUCKET defaults to gs://$PROJECT-jobs.
#
# Watch:   gcloud storage cat $BUCKET/<job>/log.txt
# List:    gcloud compute instances list --project $PROJECT

set -euo pipefail

PROJECT=${PROJECT:?set PROJECT to the GCP project to run in}
# Zones to try, in order, when one has no spot capacity for the machine
# type (the CPU quota is per region).
ZONES=${ZONES:-us-central1-a us-central1-b us-central1-c us-central1-f}
BUCKET=${BUCKET:-gs://$PROJECT-jobs}

if [ $# -lt 4 ] || [ "$4" != "--" ]; then
  echo "usage: $0 <name> <machine-type> <max-hours> -- <command>" >&2
  exit 1
fi
NAME=$1
MACHINE=$2
HOURS=$3
shift 4
COMMAND="$*"
case "$COMMAND" in *,*) echo "The command can't contain commas." >&2; exit 1;; esac

JOB=$NAME-$(date +%Y%m%d-%H%M%S)
HERE=$(cd "$(dirname "$0")/.." && pwd)

SRC=$(mktemp --suffix=.tgz)
tar czf "$SRC" --exclude=./target -C "$HERE" .
gcloud storage cp --quiet "$SRC" "$BUCKET/$JOB/src.tgz"
rm "$SRC"

created=
for ZONE in $ZONES; do
  if out=$(gcloud compute instances create "$JOB" \
      --project "$PROJECT" \
      --zone "$ZONE" \
      --machine-type "$MACHINE" \
      --provisioning-model SPOT \
      --instance-termination-action DELETE \
      --max-run-duration "${HOURS}h" \
      --scopes cloud-platform \
      --image-family debian-12 \
      --image-project debian-cloud \
      --boot-disk-size 20GB \
      --metadata "job=$JOB,bucket=$BUCKET,command=$COMMAND,resume=${RESUME:-}" \
      --metadata-from-file startup-script="$HERE/cloud/startup.sh" 2>&1); then
    echo "$out" | grep -v "^ - Disk size\|^$" || true
    created=$ZONE
    break
  fi
  if echo "$out" | grep -qiE "stockout|resource_pool_exhausted|does not have enough resources"; then
    echo "No capacity in $ZONE; trying the next zone"
    continue
  fi
  echo "$out" >&2
  exit 1
done
if [ -z "$created" ]; then
  echo "No capacity in any of: $ZONES" >&2
  exit 1
fi

echo "Started $JOB"
echo "Log: gcloud storage cat $BUCKET/$JOB/log.txt"
