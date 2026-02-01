#!/bin/bash

# 1. Define the container name
CONTAINER_NAME="sift"

echo "--- Creating Distrobox container: $CONTAINER_NAME ---"
# We use Ubuntu 22.04 as it is the most stable base for SIFT SaltStack
distrobox create --name $CONTAINER_NAME --image ubuntu:22.04 --yes

echo "--- Avvio installazione interna ---"
# Usiamo -e per fermarci al primo errore e vedere cosa succede
distrobox enter $CONTAINER_NAME -- bash -e -c "
    echo '1. Aggiornamento repository...'
    sudo apt-get update
    
    echo '2. Installazione dipendenze base...'
    sudo apt-get install -y wget curl gnupg2 ca-certificates
    
    echo '3. Download CAST v1.0.4...'
    cd /tmp
    wget -c https://github.com/teamdfir/cast/releases/download/v1.0.4/cast-v1.0.4-linux-amd64.deb
    
    echo '4. Installazione CAST...'
    sudo dpkg -i cast-v1.0.4-linux-amd64.deb || sudo apt-get install -f -y
    
    echo '5. Installazione SIFT (Server Mode)...'
    sudo cast install teamdfir/sift-saltstack --mode=server
"

echo "--- Setup Finished! ---"
echo "To enter your forensic lab, run: distrobox enter $CONTAINER_NAME"
