#!/bin/bash
# deploy-cluster2.sh — deploy spark-dashboard to the RTX PRO cluster (tron-cluster-2, 192.168.10.103)
# Usage: bash deploy-cluster2.sh
# Preconditions: branch feat/rtx-pro-cluster merged/tested in ~/spark-dashboard-prs/pr46 (Zeus)
#   frontend dist built on Zeus (statics are platform-independent; cargo build runs NATIVE on .103)

set -euo pipefail
SRC="$HOME/spark-dashboard-prs/pr46"
DEST_HOST="cwykel@192.168.10.103"
DEST_DIR="/home/cwykel/spark-dashboard"

echo "== 1. sanity: frontend dist exists =="
test -f "$SRC/frontend/dist/index.html" || { echo "FAIL: run npm run build in $SRC/frontend first"; exit 1; }

echo "== 2. rsync source (no build artifacts) =="
rsync -az --delete \
  --exclude=target --exclude=node_modules --exclude=.git \
  --exclude='.review' --exclude='*.md' --exclude='.bak*' --exclude='.reconcile.py' --exclude='.repair-*' \
  "$SRC/" "$DEST_HOST:$DEST_DIR/"

echo "== 3. ensure dist synced (rust-embed needs it at compile time) =="
rsync -az --delete "$SRC/frontend/dist/" "$DEST_HOST:$DEST_DIR/frontend/dist/"

echo "== 4. native build on .103 (x86_64) =="
ssh "$DEST_HOST" 'source ~/.cargo/env && cd ~/spark-dashboard && cargo build --release 2>&1 | tail -2'

echo "== 5. install binary =="
ssh "$DEST_HOST" 'sudo cp ~/spark-dashboard/target/release/spark-dashboard /usr/local/bin/ && echo binary installed: $(ls -la /usr/local/bin/spark-dashboard | awk "{print \$5}")'

echo "== 6. systemd unit + override (PrivateTmp/ProtectSystem for history DB) =="
ssh "$DEST_HOST" 'sudo tee /etc/systemd/system/spark-dashboard.service > /dev/null << "EOF"
[Unit]
Description=Spark Dashboard (RTX PRO Cluster 2)
After=network.target
[Service]
EnvironmentFile=/etc/spark-dashboard/config.env
ExecStart=/usr/local/bin/spark-dashboard --port 3000
Restart=always
RestartSec=5
User=cwykel
[Install]
WantedBy=multi-user.target
EOF
ssh "$DEST_HOST" 'sudo mkdir -p /etc/systemd/system/spark-dashboard.service.d
sudo tee /etc/systemd/system/spark-dashboard.service.d/override.conf > /dev/null << "EOF"
[Service]
PrivateTmp=false
ProtectSystem=read-only
ReadWritePaths=/var/lib/spark-dashboard
EOF
sudo systemctl daemon-reload'

echo "== 7. config env =="
ssh "$DEST_HOST" 'grep -q TITLE /etc/spark-dashboard/config.env 2>/dev/null || tee /etc/spark-dashboard/config.env > /dev/null << "EOF"
SPARK_DASHBOARD_TITLE=RTX PRO Dashboard
SPARK_DASHBOARD_ENABLE_HISTORY=true
SPARK_DASHBOARD_ENABLE_LOG_VIEWER=true
SPARK_DASHBOARD_HISTORY_DB=/var/lib/spark-dashboard/history.db
EOF
chmod 644 /etc/spark-dashboard/config.env'

echo "== 8. enable history logging (fresh DB ships toggle OFF — silent no-op!) =="
ssh "$DEST_HOST" 'curl -sf -X POST -H "Content-Type: application/json" -d "{\"enabled\":true}" http://localhost:3000/api/history/toggle'

echo "== 9. start + report =="
ssh "$DEST_HOST" 'sudo systemctl enable --now spark-dashboard && sleep 2 && systemctl is-active spark-dashboard'

echo "DONE. Verify: curl http://100.84.37.35:3000/api/meta"
