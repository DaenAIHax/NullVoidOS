#!/bin/bash
# Controlla se il contenitore esiste già, altrimenti lo crea
if ! distrobox list | grep -q "remnux-lab"; then
    echo "Creazione del laboratorio REMnux in corso..."
    distrobox create --image remnux/remnux-distrobox:latest --name remnux-lab --yes
fi

# Avvia il laboratorio
distrobox enter remnux
