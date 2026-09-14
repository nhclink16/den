# Home network investigation

2026-09-12. Initial tests 19:05–19:12 EDT; updated with Nicholas's 19:54–19:55 Xfinity app screenshots and a fresh public IPv4 check. Host `codexbox`, Debian 12.

**Verdict: port forwarding possible.** The gateway WAN IPv4 matches the independently measured public IPv4, ruling out CGNAT on this connection. The app exposes manual port forwarding, consistent with [Xfinity's instructions](https://www.xfinity.com/support/articles/xfi-port-forwarding). A successful forwarded inbound connection remains unverified; this is a capability verdict, not a completed hosting acceptance test.

1. **IPv4 and router.** `curl -4` to `api.ipify.org` and `ifconfig.me/ip` returned `73.20.188.137`; STUN agreed. The app now confirms WAN IPv4 `73.20.188.137`, DOCSIS, WAN IP translation `NONE`, and a Technicolor XB8, revision 2.5, firmware `CGM4981COM_8.5p10s1_PROD_sey`. Default route is gateway `10.0.0.1`, Ethernet source `10.0.0.235`. SSDP received no gateway response; NAT-PMP discovery timed out. Existing UPnP/forwarding rules remain unread. `100.116.27.23` is Tailscale, not the router WAN.

2. **IPv6.** Ethernet has global addresses in `2601:100:8c01:ed0::/64`, including `2601:100:8c01:ed0::562`. A default IPv6 route exists, and `curl -6 https://api64.ipify.org` succeeded using `2601:100:8c01:ed0:892f:cf61:4200:f70b`. This proves outbound IPv6. Unsolicited inbound IPv6 was not tested, so “possible via IPv6 only” is unsupported.

3. **Link and upload.** `enp5s0` is Ethernet, 1,000 Mbps full duplex from sysfs. Wi-Fi is also connected but has route metric 600 versus Ethernet's 10. A 10.03-second series of 8 MiB curl uploads to `speed.cloudflare.com/__up` sent 40 MiB, approximately **33.4 Mbps**. Four requests completed with HTTP 200; the final request hit the time limit. Counting only completed uploads gives 26.8 Mbps. This is an approximate application throughput measurement.

4. **Minecraft/playit.** Both services are currently `inactive/dead`, `disabled`; no Minecraft listener exists. `/etc/playit/playit.toml` contains the agent credential. A read-only `/v1/agents/rundata` query confirms Minecraft Java **TCP** `monument-swaddling.tun.ply.gg` → `127.0.0.1:25565`, plus **UDP** `sands-thanked.tun.ply.gg:6307` → `127.0.0.1:24454`. Rotated logs record two running tunnels on September 11, followed by DNS errors. Playit provides the configured public Minecraft route, but it is not currently making this stopped server reachable.

5. **Inbound test.** During a verified 20-second `nc -l -p 25565 -s 10.0.0.235` listener, [CanYouSeeMe](https://canyouseeme.org/) reported timeout and [Portchecker](https://portchecker.co/) reported closed for `73.20.188.137:25565`. No connections arrived; closure was verified with `ss`. Host INPUT policies accept this traffic. No forwarding rule was added or verified. This failure does not establish an ISP block; missing forwarding or gateway filtering remain possible.

Den's current configuration targets Tailscale. Home hosting is viable in principle, with HTTPS, forwarded [LiveKit media ports](https://docs.livekit.io/transport/self-hosting/ports-firewall/), and TURN configuration. Next proof requires an authorized forwarding test and an external voice call. A VPS avoids dependence on home upload capacity and gateway configuration. No router/firewall settings changed or commits made.
