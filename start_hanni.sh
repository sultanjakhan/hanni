#!/bin/bash
cd ~/hanni
source .venv/bin/activate

# Убедись что LM Studio запущен на порту 8000
echo "Checking LM Studio..."
until curl -s http://localhost:8000/v1/models > /dev/null 2>&1; do
    echo "Waiting for LM Studio..."
    sleep 5
done

echo "LM Studio ready, starting Hanni..."
python bot_autonomous.py
