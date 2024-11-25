#! /usr/bin/env bash

set -euxo pipefail

cargo build

sudo cp psdzd.service /etc/systemd/system/psdzd.service
sudo systemctl daemon-reload
sudo cp target/debug/psdzd /opt/psdzd/psdzd-next
sudo mv /opt/psdzd/psdzd-next /opt/psdzd/psdzd
if systemctl is-active psdzd > /dev/null 2>&1; then
    sudo systemctl reload psdzd
else
    sudo systemctl start psdzd
fi
