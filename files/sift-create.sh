#!/bin/bash

CONTAINER_NAME="sift-custom"

echo "--- Creating Custom Forensic Container ---"
# AGGIUNTO --root: Fondamentale per mount e accessi raw
distrobox create --name $CONTAINER_NAME --image ubuntu:22.04 --root --yes

echo "--- Avvio installazione interna ---"

# Rimosso 'sudo' perché con --root siamo già root dentro
distrobox enter --root $CONTAINER_NAME -- bash -e -c "
    echo '1. Aggiornamento e tool base...'
    apt-get update && apt-get install -y wget curl gnupg2 ca-certificates lsb-release

    echo '2. Download CAST v0.10.6 (Versione stabile per 22.04)...'
    # Nota: a volte le versioni nuove di cast rompono le dipendenze, controlla sempre
    wget -O /tmp/cast.deb https://github.com/ekristen/cast/releases/download/v0.10.6/cast_0.10.6_linux_amd64.deb

    echo '3. Installazione CAST...'
    dpkg -i /tmp/cast.deb || apt-get install -f -y

    echo '4. Installazione SIFT (Modalità Server - Più leggera)...'
    # Questo passaggio impiegherà MOLTO tempo su USB
    cast install teamdfir/sift-saltstack --mode=server --user=root
"

echo "--- Setup Finished! ---"
echo "To enter run: distrobox enter --root $CONTAINER_NAME"
