#!/bin/bash

# 1. Define the container name
CONTAINER_NAME="sift-lab"

echo "--- Creating Distrobox container: $CONTAINER_NAME ---"
# AGGIUNTO --root: Fondamentale per Forensics (mount, losetup)
distrobox create --name $CONTAINER_NAME --image ubuntu:22.04 --root --yes

echo "--- Avvio installazione interna ---"
# Usiamo bash -c. Nota che la prima volta ti chiederà di creare la password utente.
distrobox enter --root $CONTAINER_NAME -- bash -e -c "
    echo '1. Aggiornamento repository...'
    sudo apt-get update
    
    echo '2. Installazione dipendenze base...'
    sudo apt-get install -y wget curl gnupg2 ca-certificates lsb-release
    
    echo '3. Download CAST v0.10.6 (Versione Stabile)...'
    # Scarichiamo in /tmp per pulizia
    wget -O /tmp/cast.deb https://github.com/ekristen/cast/releases/download/v1.0.4/cast-1.0.4-linux-amd64.deb
    
    echo '4. Installazione CAST...'
    sudo dpkg -i /tmp/cast.deb || sudo apt-get install -f -y
    
    echo '5. Installazione SIFT (Server Mode)...'
    # Nota: --user=root serve a cast per capire che ha i permessi di scrittura
    sudo cast install teamdfir/sift-saltstack --mode=server --user=root
"

echo "--- Setup Finished! ---"
echo "To enter your forensic lab, run: distrobox enter --root $CONTAINER_NAME"
