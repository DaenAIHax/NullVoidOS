#!/bin/bash

# -------------------------------------------------------------------------
# Null Void OS
# SIFT Forensics Lab — Root-Enabled Distrobox Environment
# -------------------------------------------------------------------------
# This script provisions a root-enabled Ubuntu container configured as a
# forensic analysis laboratory using the SIFT workstation (server mode).
# -------------------------------------------------------------------------

# 1. Define the container name
CONTAINER_NAME="Sift"

echo "Initializing Null Void OS forensic environment..."
echo "Creating Distrobox container: ${CONTAINER_NAME}"

# --root is required for forensic operations (mount, losetup, loop devices)
distrobox create --name $CONTAINER_NAME --image ubuntu:22.04 --root --yes

echo "Container created successfully."
echo "Starting internal environment provisioning..."

# We use bash -e -c to stop execution on error.
# On first run, you may be prompted to create a user password.
distrobox enter --root $CONTAINER_NAME -- bash -e -c "
    echo 'Updating package repositories...'
    sudo apt-get update
    
    echo 'Installing base dependencies...'
    sudo apt-get install -y wget curl gnupg2 ca-certificates lsb-release
    
    echo 'Downloading CAST (stable release)...'
    # Downloaded to /tmp for easier cleanup
    wget -O /tmp/cast.deb https://github.com/ekristen/cast/releases/download/v1.0.4/cast-v1.0.4-linux-amd64.deb
    
    echo 'Installing CAST...'
    sudo dpkg -i /tmp/cast.deb || sudo apt-get install -f -y
    
    echo 'Installing SIFT Workstation (server mode)...'
    # --user=root is required to allow CAST to write system-wide resources
    sudo cast install teamdfir/sift-saltstack --mode=server --user=root
"

echo "Null Void OS forensic environment setup completed successfully."
echo "To enter the forensic lab, run:"
echo "distrobox enter --root ${CONTAINER_NAME}"

