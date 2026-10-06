#!/bin/bash
# Startup script for a job VM (see cloud/launch.sh). Runs as root at boot.

metadata() {
  curl -s -H "Metadata-Flavor: Google" "http://metadata.google.internal/computeMetadata/v1/instance/$1"
}

JOB=$(metadata attributes/job)
BUCKET=$(metadata attributes/bucket)
COMMAND=$(metadata attributes/command)
RESUME=$(metadata attributes/resume)
ZONE=$(metadata zone | awk -F/ '{print $NF}')
VM=$(metadata name)

LOG=/var/log/job.log
exec > >(tee -a "$LOG") 2>&1

# Whatever happens, save what there is and delete this VM.
finish() {
  echo "=== finishing: $1"
  gcloud storage cp --quiet "$LOG" "$BUCKET/$JOB/log.txt"
  if [ -d /job/working ]; then
    gcloud storage rsync --quiet --recursive /job/working "$BUCKET/$JOB/working"
  fi
  gcloud compute instances delete "$VM" --zone "$ZONE" --quiet
}
trap 'finish "script exited"' EXIT

echo "=== job $JOB on $(nproc) CPUs: $COMMAND"

# The log every minute, and working/ (checkpoints) every 5 minutes, so a
# reclaimed spot VM loses little.
(
  i=0
  while true; do
    sleep 60
    gcloud storage cp --quiet "$LOG" "$BUCKET/$JOB/log.txt" > /dev/null 2>&1
    i=$((i + 1))
    if [ $((i % 5)) -eq 0 ] && [ -d /job/working ]; then
      gcloud storage rsync --quiet --recursive /job/working "$BUCKET/$JOB/working" > /dev/null 2>&1
    fi
  done
) &

set -e
apt-get update -q
apt-get install -y -q build-essential
export HOME=/root
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
source /root/.cargo/env

mkdir -p /job
cd /job
gcloud storage cp --quiet "$BUCKET/$JOB/src.tgz" .
tar xzf src.tgz
mkdir -p working
if [ -n "$RESUME" ]; then
  echo "=== resuming from $RESUME"
  if ! gcloud storage ls "$BUCKET/$RESUME/working/" > /dev/null 2>&1; then
    echo "error: $RESUME saved nothing to resume from"
    exit 1
  fi
  gcloud storage rsync --recursive "$BUCKET/$RESUME/working" working
  # Save it under this job straight away, so that if this VM is reclaimed
  # before the first periodic upload, the next relaunch (which resumes from
  # this job) still starts from these checkpoints rather than from scratch.
  gcloud storage rsync --quiet --recursive working "$BUCKET/$JOB/working"
fi
cargo build --release --bin secondpasstrial

echo "=== running"
start=$(date +%s)
bash -c "$COMMAND"
echo "=== done in $(( $(date +%s) - start ))s"
