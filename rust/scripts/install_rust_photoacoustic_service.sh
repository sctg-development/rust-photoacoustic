#!/usr/bin/env bash
set -euo pipefail

SERVICE_PATH=/etc/systemd/system/rust-photoacoustic.service

cat <<'EOF' | sudo tee "$SERVICE_PATH" > /dev/null
[Unit]
Description=Rust Photoacoustic service
After=network.target

[Service]
Type=simple
User=lasersmart
Group=lasersmart
WorkingDirectory=/home/lasersmart
ExecStart=/usr/local/bin/rust-photoacoustic --config /home/lasersmart/config.yaml
Restart=on-failure
RestartSec=5s
TimeoutStopSec=5s
KillMode=process
SendSIGKILL=yes
Environment=RUST_LOG=info
StandardOutput=journal
StandardError=journal
LimitNOFILE=4096

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable rust-photoacoustic.service
sudo systemctl start rust-photoacoustic.service
sudo systemctl status rust-photoacoustic.service --no-pager

echo "Service rust-photoacoustic installed and started. Logs: journalctl -u rust-photoacoustic.service -f"
