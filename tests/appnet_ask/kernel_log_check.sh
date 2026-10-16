#!/bin/bash
# Needs root (sudo bash tests/appnet_ask/kernel_log_check.sh): checks the real kernel log line of an nft `log ... flags skuid` rule
# against labyrinth-appnet-ask's parser. Uses a temporary table and an unroutable TEST-NET address; the table is removed afterwards.
# Throwaway table: log (with user id) and count one outgoing SYN to a TEST-NET address from this user, then remove it.
nft add table inet labask_test
nft add chain inet labask_test out '{ type filter hook output priority -6; policy accept; }'
nft add rule inet labask_test out meta skuid 1000 ip daddr 198.51.100.7 limit rate 4/second burst 8 packets log prefix '"labask:testapp "' flags skuid
sudo -u zlothra timeout 2 bash -c 'exec 3<>/dev/tcp/198.51.100.7/443' 2>/dev/null
sleep 1
journalctl -k --since "-20 sec" -o cat | grep labask: | tail -2
nft delete table inet labask_test
