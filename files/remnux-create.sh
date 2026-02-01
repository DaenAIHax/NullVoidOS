#!/bin/bash
# 1. Configurazione cartella log sulla USB per Thug
LOG_DIR="$HOME/remnux_logs/thug"
mkdir -p "$LOG_DIR"
chmod a+xwr "$LOG_DIR"

# 2. Controllo e creazione del contenitore Distrobox
# Usiamo l'immagine specifica di quay.io che è molto stabile per REMnux
if ! distrobox list | grep -q "remnux-lab"; then
    echo "Creazione del laboratorio REMnux in corso..."
    distrobox create --image quay.io/remnux/remnux-distrobox:latest --name remnux-lab --yes
fi

echo "--- Avvio REMnux Lab ---"
echo "I log di Thug saranno in: $LOG_DIR"

# 3. Entra nel laboratorio (il nome deve coincidere con quello sopra)
distrobox enter remnux-lab
