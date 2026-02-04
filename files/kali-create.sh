#!/bin/bash
set -euo pipefail

# -------------------------------------------------------------------------
# Null Void OS
# Kali Linux (Rolling) — Root-Enabled Distrobox Environment
# -------------------------------------------------------------------------
# This script prepares a privileged Kali Linux container intended for
# security research, penetration testing, and advanced networking tasks.
#
# Security note:
# We enable passwordless sudo *inside the container* (NOT on the host),
# to avoid double password prompts while keeping host sudo protected.
# -------------------------------------------------------------------------

CONTAINER_NAME="Kali"

echo "Initializing Null Void OS environment..."
echo "Target container: ${CONTAINER_NAME}"
echo "This process may take several minutes."

# 1) Create the container as rootful (required for low-level networking tools)
distrobox create \
  --image docker.io/kalilinux/kali-rolling \
  --name "${CONTAINER_NAME}" \
  --root -Y

echo "Container successfully created."
echo "Proceeding with Kali Linux headless installation."
echo "NOTICE: Approximately 3 GB of data will be downloaded."

# 2) Install packages + enable passwordless sudo inside the container
distrobox enter --root "${CONTAINER_NAME}" -- bash -euo pipefail -c '
  echo "Updating package index..."
  sudo apt-get update

  echo "Installing kali-linux-headless metapackage..."
  sudo apt-get install -y kali-linux-headless

  # Ensure sudo + visudo are present (usually already installed, but keep it robust)
  echo "Ensuring sudo is installed..."
  sudo apt-get install -y sudo

  # Determine the user we are running as inside the container (typically your host user)
  TARGET_USER="$(id -un)"
  echo "Enabling passwordless sudo for user: ${TARGET_USER}"

  # Write a dedicated sudoers file (safer than editing /etc/sudoers)
  sudo install -d -m 0755 /etc/sudoers.d
  sudo tee /etc/sudoers.d/nvx-nopasswd >/dev/null <<EOF
${TARGET_USER} ALL=(ALL) NOPASSWD: ALL
Defaults:${TARGET_USER} !authenticate
EOF

  # Correct permissions are required, otherwise sudo will ignore the file
  sudo chmod 0440 /etc/sudoers.d/nvx-nopasswd

  # Fix common broken sudoers.d permissions (some images ship with wrong modes)
if [[ -f /etc/sudoers.d/sudoers ]]; then
  sudo chown root:root /etc/sudoers.d/sudoers
  sudo chmod 0440 /etc/sudoers.d/sudoers
fi


  # Validate sudoers syntax
  sudo visudo -c

  echo "Cleaning package cache..."
  sudo apt-get clean
'

echo "Null Void OS environment setup completed successfully."
echo "You may now enter the system using the following command:"
echo "distrobox enter --root ${CONTAINER_NAME}"
