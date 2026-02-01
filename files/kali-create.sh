#!/bin/bash

# 1. Definizione nome del container (Attenzione alla Maiuscola!)
CONTAINER_NAME="Kali"

echo "🛠️  Preparing your ROOT hacking environment ($CONTAINER_NAME)..."

# 2. Creazione del container con privilegi ROOT
# Fondamentale per far funzionare Nmap, OpenVPN e Wireshark
distrobox create --image docker.io/kalilinux/kali-rolling --name $CONTAINER_NAME --root -Y

echo "🚀 Installing Kali Headless (Metapackages)..."
echo "⚠️  ATTENZIONE: Questo pacchetto scarica ~3GB di dati. Assicurati di avere spazio!"

# 3. Installazione dei pacchetti
# Usiamo 'bash -e -c' per fermarci se c'è un errore.
# Aggiunto 'sudo' davanti ai comandi apt perché entri come utente normale.
distrobox enter --root $CONTAINER_NAME -- bash -e -c "
    echo '--- Aggiornamento lista pacchetti ---'
    sudo apt-get update
    
    echo '--- Installazione Kali Headless ---'
    # Ti chiederà la password la prima volta
    sudo apt-get install -y kali-linux-headless
    
    echo '--- Pulizia cache ---'
    sudo apt-get clean
"

echo "✅ Done! To enter your Kali environment, type: distrobox enter --root $CONTAINER_NAME"
