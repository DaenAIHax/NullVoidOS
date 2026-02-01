#!/bin/bash

# 1. Define the container name
CONTAINER_NAME="sift"

echo "--- Creating Distrobox container: $CONTAINER_NAME ---"
# We use Ubuntu 22.04 as it is the most stable base for SIFT SaltStack
distrobox create --name $CONTAINER_NAME --image ubuntu:22.04 --yes

echo "--- Starting installation inside the container ---"
# Execute commands inside the newly created container
distrobox enter $CONTAINER_NAME -- bash -c "
    sudo apt-get update && sudo apt-get install -y wget curl gnupg2;

    # Ci spostiamo in /tmp per non sporcare la home
    cd /tmp
    
    echo '--- Downloading CAST v1.0.4 ---';
    wget https://github.com/ekristen/cast/releases/tag/v1.0.4/cast-v1.0.4-linux-amd64.deb;
    
    echo '--- Installing CAST ---';
    sudo dpkg -i cast-v1.0.4-linux-amd64.deb || sudo apt-get install -f -y;
    
    echo '--- Starting SIFT Installation (Server Mode) ---';
    # Server mode is recommended for USB drives to save space and improve performance
    sudo cast install teamdfir/sift-saltstack --mode=server;
"

echo "--- Setup Finished! ---"
echo "To enter your forensic lab, run: distrobox enter $CONTAINER_NAME"
