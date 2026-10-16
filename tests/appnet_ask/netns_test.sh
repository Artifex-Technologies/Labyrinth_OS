# Real nftables behaviour of the Ask rules, inside a throw-away user+network namespace (no root, nothing on your system is touched).
# Run: unshare -Urn bash tests/appnet_ask/netns_test.sh   Expected: no rules CONNECTED; allowed .2 CONNECTED; .3 BLOCKED, then CONNECTED once added.
set -e
cd "$(dirname "$0")"
python3 gen_ruleset.py 10.77.0.2 > /tmp/appnet-ask-allow.nft
python3 gen_ruleset.py > /tmp/appnet-ask-none.nft
ip link set lo up
unshare -n sleep 20 & CH=$!
sleep 0.5
ip link add veth0 type veth peer name veth1 netns $CH
ip addr add 10.77.0.1/24 dev veth0; ip link set veth0 up
nsenter -t $CH -n bash -c 'ip link set lo up; ip addr add 10.77.0.2/24 dev veth1; ip link set veth1 up; ip addr add 10.77.0.3/24 dev veth1' 
nsenter -t $CH -n python3 -c "
import socket,threading
def serve(a):
    s=socket.socket(); s.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1); s.bind((a,8080)); s.listen(5)
    while True:
        c,_=s.accept(); c.close()
for a in ('10.77.0.2','10.77.0.3'): threading.Thread(target=serve,args=(a,),daemon=True).start()
import time; time.sleep(15)" &
sleep 1
try() { python3 -c "
import socket,sys
s=socket.socket(); s.settimeout(1.5)
try: s.connect((sys.argv[1],8080)); print(sys.argv[1],'CONNECTED')
except Exception as e: print(sys.argv[1],'BLOCKED',type(e).__name__)" $1; }
echo "--- no rules"; try 10.77.0.2
nft -f /tmp/appnet-ask-allow.nft
echo "--- ask mode, 10.77.0.2 allowed"; try 10.77.0.2; try 10.77.0.3
nft add element inet labyrinth_apps $(nft list sets | grep -o 'ask_1000_web_[0-9a-f]*_4' | head -1) { 10.77.0.3 }
echo "--- after adding .3 at run time"; try 10.77.0.3
kill $CH 2>/dev/null; true
