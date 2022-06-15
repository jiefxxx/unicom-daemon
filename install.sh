#!/bin/bash

mkdir /var/unicom
mkdir /var/unicom/apps
mkdir /var/unicom/templates
mkdir /etc/unicom

cp ./config/config.toml /etc/unicom

cp ./target/release/unicom-daemon /bin/

cp ./config/unicom-daemon.service  /etc/systemd/system/

#systemctl daemon-reload

#systemctl enable unicom-daemon

#systemctl restart unicom-daemon



