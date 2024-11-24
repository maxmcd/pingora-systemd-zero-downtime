#! /usr/bin/env bash

set -euxo pipefail

cargo build

sudo cp psdzd.service /etc/systemd/system/psdzd.service
sudo cp target/debug/psdzd /opt/psdzd/psdzd-next
sudo mv /opt/psdzd/psdzd-next /opt/psdzd/psdzd
sudo systemctl reload psdzd
