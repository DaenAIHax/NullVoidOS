#!/bin/bash

# -------------------------------------------------------------------------
# Null Void OS
# Kali Linux (Rolling) — Root-Enabled Distrobox Environment
# -------------------------------------------------------------------------
# Thank you for testing Null Void OS.
# This script prepares a privileged Kali Linux container intended for
# security research, penetration testing, and advanced networking tasks.
# -------------------------------------------------------------------------

# 1. Container name definition (case-sensitive)
CONTAINER_NAME="Kali"

echo "Initializing Null Void OS environment..."
echo "Target container: ${CONTAINER_NAME}"
echo "This process may take several minutes."

# 2. Create the container with ROOT privileges
# Required for tools such as Nmap, OpenVPN, and Wireshark
distrobox create --image docker.io/kalilinux/kali-rolling --name $CONTAINER_NAME --root -Y

echo "Container successfully created."
echo "Proceeding with Kali Linux headless installation."
echo "NOTICE: Approximately 3 GB of data will be downloaded."

# 3. Package installation
# 'bash -e -c' ensures the process stops on any error.
# 'sudo' is required because the session may start as a non-root user.
distrobox enter --root $CONTAINER_NAME -- bash -e -c "
    echo 'Updating package index...'
    sudo apt-get update
    
    echo 'Installing kali-linux-headless metapackage...'
    sudo apt-get install -y kali-linux-headless
    
    echo 'Cleaning package cache...'
    sudo apt-get clean
"

echo "Null Void OS environment setup completed successfully."
echo "You may now enter the system using the following command:"
echo "distrobox enter --root ${CONTAINER_NAME}"
