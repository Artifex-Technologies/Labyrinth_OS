"""The cases of ipconfig (loaded by compare.py). Each run is in a world of its own (compare.NS): its host name is parity-host,
/sys/class/net, /proc/sys/net/ipv4/ip_forward and /etc/resolv.conf are the world's; ip, nmcli, udevadm and resolvectl are
stand-ins (nothing is disconnected or flushed). The addresses are the ones set aside for documentation (192.0.2.0/24,
198.51.100.0/24, 2001:db8::/32, the MAC block 00-00-5E-00-53-xx)."""
import json
import os

import __main__ as P

W = P.WORLD
FS = os.path.join(W, "fs")

IP = r"""#!/bin/sh
line=ip
for a in "$@"; do line="$line [$a]"; done
printf '%s\n' "$line" >> "$WORLD/calls"
case "$*" in
  "-j addr") printf '%s' "${IP_ADDR:-}" ;;
  "-j route show default") printf '%s' "${IP_ROUTE4:-}" ;;
  "-j -6 route show default") printf '%s' "${IP_ROUTE6:-}" ;;
esac
exit ${RC_ip:-0}
"""
NMCLI = r"""#!/bin/sh
line=nmcli
for a in "$@"; do line="$line [$a]"; done
printf '%s\n' "$line" >> "$WORLD/calls"
case "$1 $2 $3" in
  "-t -e no") eval "printf '%b' \"\${NM_$(printf '%s' "$6" | tr -c 'A-Za-z0-9' '_'):-}\"" ;;
  "-t -f DEVICE,TYPE,STATE") printf '%b' "${NM_DEVICES:-}" ;;
  device*) echo "Device '$3' successfully ${2}ed." ;;
esac
exit ${RC_nmcli:-0}
"""
UDEVADM = r"""#!/bin/sh
line=udevadm
for a in "$@"; do line="$line [$a]"; done
printf '%s\n' "$line" >> "$WORLD/calls"
eval "printf '%b' \"\${UDEV_$(printf '%s' "${5##*/}" | tr -c 'A-Za-z0-9' '_'):-}\""
"""

LO = {"ifindex": 1, "ifname": "lo", "operstate": "UNKNOWN", "link_type": "loopback", "address": "00:00:00:00:00:00",
      "addr_info": [{"family": "inet", "local": "127.0.0.1", "prefixlen": 8, "scope": "host"}]}
ETH = {"ifindex": 2, "ifname": "eth0", "operstate": "UP", "link_type": "ether", "address": "00:00:5e:00:53:01",
       "addr_info": [{"family": "inet", "local": "192.0.2.10", "prefixlen": 24, "scope": "global", "dynamic": True, "valid_life_time": 86399},
                     {"family": "inet6", "local": "2001:db8::10", "prefixlen": 64, "scope": "global", "dynamic": True, "valid_life_time": 4294967295},
                     {"family": "inet6", "local": "fe80::200:5eff:fe00:5301", "prefixlen": 64, "scope": "link"}]}
WLAN = {"ifindex": 3, "ifname": "wlan0", "operstate": "DOWN", "link_type": "ether", "address": "00:00:5e:00:53:02", "addr_info": []}
TUN = {"ifindex": 4, "ifname": "tun0", "operstate": "UNKNOWN", "link_type": "none",
       "addr_info": [{"family": "inet", "local": "198.51.100.7", "prefixlen": 32}]}
ROUTE4 = [{"dst": "default", "gateway": "192.0.2.1", "dev": "eth0"}]
ROUTE6 = [{"dst": "default", "gateway": "fe80::1", "dev": "eth0"}, {"dst": "default", "dev": "tun0"}]
NM_ETH = "GENERAL.DEVICE:eth0\nGENERAL.CONNECTION:Wired connection 1\nIP4.ADDRESS[1]:192.0.2.10/24\nIP4.DNS[1]:192.0.2.53\nIP4.DNS[2]:192.0.2.54\nIP6.DNS[1]:2001:db8::53\nIP4.DOMAIN[1]:example.test\nIP4.DOMAIN[2]:lan.test\nIP6.GATEWAY:\n"
NM_WLAN = "GENERAL.DEVICE:wlan0\nGENERAL.CONNECTION:\n"
BASE_ENV = {"IP_ADDR": json.dumps([LO, ETH, WLAN, TUN]), "IP_ROUTE4": json.dumps(ROUTE4), "IP_ROUTE6": json.dumps(ROUTE6), "NM_eth0": NM_ETH,
            "NM_wlan0": NM_WLAN, "NM_tun0": "GENERAL.CONNECTION:tun0\n",
            "UDEV_eth0": "ID_VENDOR_FROM_DATABASE=Example Corp\\nID_MODEL_FROM_DATABASE=Ethernet Controller X\\nINTERFACE=eth0\\n",
            "NM_DEVICES": "eth0:ethernet:connected\\nwlan0:wifi:disconnected\\ntun0:tun:connected (externally)\\np2p-dev-wlan0:wifi-p2p:disconnected\\neth1:ethernet:unavailable\\nlo:loopback:unmanaged\\n"}


def world(resolv="nameserver 192.0.2.99\n# comment\nnameserver\nnameserverx 192.0.2.98 extra\noptions edns0\n", forward="1\n", no_resolv=False):
    def go(paths):
        os.makedirs(os.path.join(W, "pybin"), exist_ok=True)
        for t in ("python3", "basename", "tr", "cat"):
            os.symlink("/usr/bin/" + t, os.path.join(W, "pybin", t))
        P.stubs(["resolvectl"])
        P.write(os.path.join(W, "bin", "ip"), IP, 0o755)
        P.write(os.path.join(W, "bin", "nmcli"), NMCLI, 0o755)
        P.write(os.path.join(W, "bin", "udevadm"), UDEVADM, 0o755)
        net = os.path.join(FS, "net")
        os.makedirs(os.path.join(net, ".drivers", "e1000e"))
        for dev in ("eth0", "wlan0", "tun0", "lo", "eth9"):
            os.makedirs(os.path.join(net, dev))
        os.makedirs(os.path.join(net, "wlan0", "wireless"))
        os.makedirs(os.path.join(net, "wlan0", "device"))
        os.symlink("/sys/class/net/.drivers/iwlwifi", os.path.join(net, "wlan0", "device", "driver"))  # a driver link that leads nowhere
        os.makedirs(os.path.join(net, "eth9", "device"))
        os.symlink("../../.drivers/e1000e", os.path.join(net, "eth9", "device", "driver"))
        P.write(os.path.join(FS, "ip_forward"), forward)
        mounts = [("hostname", "parity-host"), ("bind", net, "/sys/class/net"), ("bind", os.path.join(FS, "ip_forward"), "/proc/sys/net/ipv4/ip_forward")]
        if no_resolv:
            mounts += [("whiteout", "/etc", "resolv.conf"), ("overlay", "/etc")]
        else:
            P.write(os.path.join(FS, "resolv.conf"), resolv)
            mounts.append(("bind", os.path.join(FS, "resolv.conf"), "/etc/resolv.conf"))
        P.world_of_its_own(mounts)
    return go


ENV = {"PATH": os.path.join(W, "bin") + ":" + os.path.join(W, "pybin")}


def case(name, steps=None, env=None, **world_args):
    e = dict(ENV)
    e.update(BASE_ENV)
    e.update(env or {})
    steps = steps or [{"args": []}, {"args": ["/all"]}]
    for s in steps:
        s["wrap"] = P.NS_WRAP
    P.case("ipconfig", name, steps, setup=world(**world_args), env=e)


def links(*ls):
    return {"IP_ADDR": json.dumps(list(ls))}


case("ethernet up, Wi-Fi down, a tunnel, loopback left out", steps=[{"args": []}, {"args": ["/all"]}, {"args": ["-all"]}, {"args": ["--ALL"]}, {"args": ["/All", "x"]}])
case("two gateways and three DNS servers line up (fixed)", env={"IP_ROUTE4": json.dumps(ROUTE4 + [{"gateway": "192.0.2.2", "dev": "eth0"}])})
case("no DNS from NetworkManager: resolv.conf's", env={"NM_eth0": "GENERAL.CONNECTION:eth0\n"})
case("no DNS anywhere", env={"NM_eth0": ""}, resolv="# nothing\n")
case("no resolv.conf", env={"NM_eth0": ""}, no_resolv=True)
case("a resolv.conf that is not UTF-8: the script stopped", env={"NM_eth0": ""}, resolv=b"nameserver \xff\n")
case("routing on and off", steps=[{"args": ["/all"]}], forward=" 0\n")
case("ip is missing", env={"PATH": os.path.join(W, "pybin")})
case("ip fails", env={"RC_ip": "1"})
case("ip answers garbage", env={"IP_ADDR": "garbage", "IP_ROUTE4": "{", "IP_ROUTE6": ""})
for label, value in [("an empty object", "{}"), ("an empty text", '""'), ("an object: the script stopped", '{"a": 1}'), ("a number: the script stopped", "5"),
                     ("a list of numbers: the script stopped", "[1]")]:
    case("ip addr answers " + label, env={"IP_ADDR": value})
for label, value in [("an object: the script stopped", '{"a": 1}'), ("a list of text: the script stopped", '["x"]'), ("null: the script stopped", "null")]:
    case("ip route answers " + label, env={"IP_ROUTE6": value})
case("gateways of odd kinds", env={"IP_ROUTE4": json.dumps([{"gateway": 5, "dev": "eth0"}, {"gateway": "", "dev": "eth0"}, {"gateway": "192.0.2.3", "dev": None},
                                                              {"gateway": "192.0.2.4", "dev": 3}, {"gateway": "192.0.2.5"}]), "IP_ROUTE6": "[]"})
case("a second gateway that is not text: the script stopped", env={"IP_ROUTE4": json.dumps([{"gateway": "192.0.2.1", "dev": "eth0"}, {"gateway": 7, "dev": "eth0"}])})
case("a gateway on a list of devices: the script stopped", env={"IP_ROUTE4": json.dumps([{"gateway": "192.0.2.1", "dev": ["eth0"]}])})
case("a link without a name: the script stopped", env=links(ETH, {"operstate": "UP"}))
case("a link named by a number: the script stopped", env=links(ETH, {"ifname": 5}))
case("a link name with a NUL: the script stopped", env=links(ETH, {"ifname": "a\u0000b"}))
case("odd links", env=links({"ifname": "eth9", "operstate": "UNKNOWN", "addr_info": {}}, {"ifname": "x0", "link_type": None, "address": "aa:bb"},
                            {"ifname": "eth9", "operstate": "UP", "link_type": "ether", "addr_info": [{"family": "inet", "local": "192.0.2.20", "prefixlen": "24"}]}))
case("a MAC that is not text: the script stopped", env=links({"ifname": "eth0", "address": None}))
case("addresses as an object (one key): /all stops", env=links({"ifname": "eth0", "operstate": "UP", "addr_info": {"a": 1}}))
case("DHCP from the first dynamic address on, the rest not looked at", env=links({"ifname": "eth0", "operstate": "DOWN", "addr_info": [{"dynamic": 1}, 5]}))
case("an address without a family: the script stopped", env=links({"ifname": "eth0", "operstate": "UP", "addr_info": [{"local": "x"}]}))
case("an IPv6 address without local: the script stopped", env=links({"ifname": "eth0", "operstate": "UP", "addr_info": [{"family": "inet6"}]}))
case("an IPv4 local that is not text: the script stopped", env=links({"ifname": "eth0", "operstate": "UP", "addr_info": [{"family": "inet", "local": 5, "prefixlen": 24}]}))
for label, p in [("24 as text", "24"), ("a netmask", "255.255.255.0"), ("a hostmask", "0.0.255.255"), ("0", 0), ("32", 32), ("true", True)]:
    case("prefix length " + label, env=links({"ifname": "eth0", "operstate": "UP", "addr_info": [{"family": "inet", "local": "192.0.2.10", "prefixlen": p}]}),
         steps=[{"args": []}])
for label, p in [("33", 33), ("24.0", 24.0), ("null", None), ("a/b", "1/2"), ("leading zeros", "255.255.255.00"), ("not a mask", "255.0.255.0"), ("missing", "-")]:
    a = {"family": "inet", "local": "192.0.2.10"}
    if p != "-":
        a["prefixlen"] = p
    case("prefix length %s: the script stopped" % label, env=links({"ifname": "eth0", "operstate": "UP", "addr_info": [a]}), steps=[{"args": []}])
for label, t in [("days", 3 * 86400 + 5), ("under a day", 59), ("forever", 4294967295), ("negative", -5), ("true", True), ("a float", 1.5), ("text", "5"), ("none", "-")]:
    a = {"family": "inet", "local": "192.0.2.10", "prefixlen": 24, "dynamic": True}
    if t != "-":
        a["valid_life_time"] = t
    case("lease " + label, env=links({"ifname": "eth0", "operstate": "UP", "addr_info": [a]}), steps=[{"args": ["/all"]}])
case("the adapter's name from its driver, or virtual", env=links({"ifname": "eth9", "operstate": "DOWN"}, {"ifname": "wlan0", "operstate": "DOWN"}, {"ifname": "tun0"}),
     steps=[{"args": ["/all"]}])
case("udevadm with only a model", env={"UDEV_eth0": "ID_MODEL_FROM_DATABASE=Only Model\\nID_VENDOR_FROM_DATABASE=\\n"}, steps=[{"args": ["/all"]}])
case("release and renew", steps=[{"args": ["/release"]}, {"args": ["-RENEW"]}, {"args": ["release"]}])
case("release with nmcli failing", steps=[{"args": ["/release"]}], env={"RC_nmcli": "1"})
case("release with nmcli missing", steps=[{"args": ["/release"]}, {"args": ["/renew"]}], env={"PATH": os.path.join(W, "pybin")})
case("flushdns", steps=[{"args": ["/flushdns"]}, {"args": ["/flushdns"], "env": {"RC_resolvectl": "1", "OUT_resolvectl": "out", "ERR_resolvectl": "err"}}])
case("flushdns without resolvectl: the script stopped", steps=[{"args": ["/flushdns"]}], env={"PATH": os.path.join(W, "pybin")})
case("help and errors", steps=[{"args": [a]} for a in ("/?", "-h", "--help", "/HELP", "/bogus", "", "///", "/all/")])
