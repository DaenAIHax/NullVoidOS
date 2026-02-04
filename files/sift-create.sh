#!/bin/bash
set -euo pipefail

# -------------------------------------------------------------------------
# Null Void OS
# SIFT Forensics Lab — Root-Enabled Distrobox Environment
# -------------------------------------------------------------------------
# This script provisions a root-enabled Ubuntu container configured as a
# forensic analysis laboratory using the SIFT workstation (server mode).
#
# Security note:
# Passwordless sudo is enabled *inside the container only* to avoid double
# password prompts while keeping host sudo protected.
# -------------------------------------------------------------------------

# 1. Define the container name
CONTAINER_NAME="Sift"

echo "Initializing Null Void OS forensic environment..."
echo "Creating Distrobox container: ${CONTAINER_NAME}"

# --root is required for forensic operations (mounts, loop devices, losetup)
distrobox create \
  --name "${CONTAINER_NAME}" \
  --image ubuntu:22.04 \
  --root \
  --yes

echo "Container created successfully."
echo "Starting internal environment provisioning..."

# 2. Provision the container
distrobox enter --root "${CONTAINER_NAME}" -- bash -euo pipefail -c '
  echo "Updating package repositories..."
  sudo apt-get update

  echo "Installing base dependencies..."
  sudo apt-get install -y \
    wget \
    curl \
    gnupg2 \
    ca-certificates \
    lsb-release \
    sudo

  # Determine the effective user inside the container (typically host user)
  TARGET_USER="$(id -un)"
  echo "Enabling passwordless sudo for user: ${TARGET_USER}"

  # Configure sudo NOPASSWD inside the container
  sudo install -d -m 0755 /etc/sudoers.d
  sudo tee /etc/sudoers.d/nvx-nopasswd >/dev/null <<EOF
${TARGET_USER} ALL=(ALL) NOPASSWD: ALL
Defaults:${TARGET_USER} !authenticate
EOF

  sudo chmod 0440 /etc/sudoers.d/nvx-nopasswd

  # Fix common broken sudoers.d permissions (some images ship with wrong modes)
if [[ -f /etc/sudoers.d/sudoers ]]; then
  sudo chown root:root /etc/sudoers.d/sudoers
  sudo chmod 0440 /etc/sudoers.d/sudoers
fi


  # Validate sudoers configuration
  sudo visudo -c

  echo "Downloading CAST (stable release)..."
  wget -O /tmp/cast.deb \
    https://github.com/ekristen/cast/releases/download/v1.0.4/cast-v1.0.4-linux-amd64.deb

  echo "Installing CAST..."
  sudo dpkg -i /tmp/cast.deb || sudo apt-get install -f -y

  echo "Installing SIFT Workstation (server mode)..."
  # --user=root is required for system-wide installation
  sudo cast install teamdfir/sift-saltstack --mode=server --user=root

  echo "Cleaning temporary files..."
  sudo rm -f /tmp/cast.deb
  sudo apt-get clean
'

echo "Null Void OS forensic environment setup completed successfully."
echo "To enter the forensic lab, run:"
echo "distrobox enter --root ${CONTAINER_NAME}"
