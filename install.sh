#!/bin/bash

mkdir /var/unicom
mkdir /var/unicom/apps
chmod 777 /var/unicom/apps
mkdir /var/unicom/templates
chmod 777 /var/unicom/templates
mkdir /var/unicom/framwork
mkdir /etc/unicom

cp ./config/config.toml /etc/unicom

cp ./target/release/unicom-daemon /bin/

cp ./config/unicom-daemon.service  /etc/systemd/system/

cp ./config/unicom-app-update /bin/

#systemctl daemon-reload

#systemctl enable unicom-daemon

#systemctl restart unicom-daemon



