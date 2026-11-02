# Machines and power

The Machines area shows the hub's own machine and every node (a machine
whose collector reports to the hub): uptime, CPU, memory, disks, battery.
From there a node's screen can be locked, and the node suspended and
woken again over the LAN, so the phone can lock a forgotten desk, put it
to sleep and bring it back.

1. [What a machine reports](#1-what-a-machine-reports)
2. [Lock](#2-lock)
3. [Suspend](#3-suspend)
4. [Wake](#4-wake)
5. [Asleep, waking, offline](#5-asleep-waking-offline)
6. [Configuration](#6-configuration)

## 1. What a machine reports

A collector reads its machine every 30 s, and once more right after a
resume. The hub reads its own machine on the same schedule. Everything
comes from `/proc`, `/sys` and `getifaddrs`, without privileges.

| Field       | Source                                                                                     |
| ----------- | ------------------------------------------------------------------------------------------ |
| up, slept   | `CLOCK_BOOTTIME` (sleep included), minus `CLOCK_MONOTONIC` (sleep excluded) for time slept |
| awake since | the last resume the collector saw: the gap between the two clocks grew                     |
| CPU         | busy share since the last reading (`/proc/stat`), load average, cores                      |
| temperature | `k10temp` Tctl, `zenpower` Tdie, `coretemp` package, `cpu_thermal`, the first found        |
| memory      | total minus available, swap used (`/proc/meminfo`)                                         |
| disks       | `statvfs` of `/` and `$HOME` (the hub: `/` and its database's directory), one per device   |
| battery     | the first `power_supply` of type Battery that is not a device's (a mouse), mains online    |
| system      | `PRETTY_NAME`, the kernel, reboot pending (NixOS: the booted and current systems differ)   |
| network     | physical wired cards: MAC, IPv4 addresses, whether `power/wakeup` is on (wake-on-LAN)      |
| screen      | logind's `LockedHint` of the user's display session, which the screen locker sets          |
| can suspend | the collector's `power.suspend`, `power.lock`                                              |

The hub keeps every node's last report and status in its database. After
a restart, Machines still lists every node it has seen, offline ones too,
and still knows how to wake them. **Forget** on an offline node's page
drops it until its collector connects again.

## 2. Lock

Lock is opt-in per machine (`power.lock`). A node whose screen is
unlocked says so on its card, in amber, with a **Lock** button; its page
has **Lock** too. Locking asks nothing first: it only protects.

The collector finds the user's graphical session (logind's `Display`) and
runs `power.lock_command`, by default `loginctl lock-session <id>`. Any
screen locker that listens to logind answers it; DankMaterialShell does
with its loginctl integration on, which is its default. The report that
follows a moment later says whether the screen is locked now. A machine
without a graphical session refuses with that reason.

## 3. Suspend

Suspend is opt-in per machine (`power.suspend`). On a node's page,
**Suspend…** asks first. The dialog says whether the hub can wake the
machine again and names the sessions mid-turn that would pause.

- First the hub checks it may: it refuses (409) when the machine is
  offline, has not opted in, is already on its way to sleep, or, unless
  forced, when a session there is working or waits on a prompt. The
  panel's dialog forces it after naming those sessions.
- The collector lets the state sync hand the hub what is pending (up to
  12 s), then runs `power.suspend_command` (`systemctl suspend`), which
  returns once the sleep is on its way.
- It answers before the freeze. A refusal (an inhibitor, logind's policy)
  comes back to the panel word for word. Otherwise the hub marks the
  machine asleep. If the link drops before the answer, the freeze came
  first and the hub marks it asleep all the same.

logind lets the user of an active seat session suspend without a
password, and that includes a process of the user's systemd manager (the
collector's service). No polkit rule is needed.

## 4. Wake

The hub sends the magic packet itself: six `0xff` bytes and the MAC
sixteen times, as UDP to port 9, three times 300 ms apart. It needs
neither a `wakeonlan` binary nor router credentials.

It learns where to send from the reports, so no MAC is ever typed:

- From the node's last report it takes the wired cards with wake-on-LAN
  on. A card that lost its addresses on the way to sleep keeps the ones
  it had.
- If the hub has an address on one of those cards' networks, the packet
  goes to that network's broadcast address, for example `10.10.0.255`.
- Otherwise it goes to `wake.via.<host>`, if that is set: an address the
  hub routes to.
- Otherwise the node cannot be woken from the hub. Its page says why, for
  example "the hub has no address on 192.168.31.0/24".

A magic packet must arrive as a frame on the sleeping machine's own
network. A tunnel to the machine itself (its WireGuard peer) sleeps with
its kernel. For a machine on another network, the router in front of it
has to put the routed packet onto that network, in one of two ways:

- **Unicast to the machine's address**, with a permanent ARP entry for it
  on that router (a sleeping machine answers no ARP). Set
  `wake.via.<host> = "<its address>"`.
- **A directed broadcast** (`<its network>.255`), which the router
  forwards onto that network. Linux: `net.ipv4.conf.<if>.bc_forwarding`.
  RouterOS: `/ip settings` and the firewall. Set
  `wake.via.<host> = "<network broadcast>"`.

Either way the hub's packets need a route to that address, and the
firewalls on the way must let UDP to port 9 through.

## 5. Asleep, waking, offline

| State     | Shown as                                    | Ends                                                     |
| --------- | ------------------------------------------- | -------------------------------------------------------- |
| connected | green, "connected for …"                    | the link drops                                           |
| asleep    | amber, "asleep since …" (suspended here)    | the machine connects again, or reports again 15 s later  |
| waking    | amber, "waking for …"                       | it connects; after 3 min the panel offers **Wake again** |
| offline   | red, "offline for …", the last report faded | it connects                                              |

A machine suspended by hand shows as offline until it resumes. Its page
then tells how long it slept and when it woke. After a sleep of 30 s or
more, the collector reconnects at once instead of waiting for its next
ping to fail. The hub forgets an unanswered wake after 10 minutes.

## 6. Configuration

| Where        | Key                                     | Default                                     | Meaning                                                       |
| ------------ | --------------------------------------- | ------------------------------------------- | ------------------------------------------------------------- |
| collector    | `power.lock`                            | `false`                                     | carry out the panel's Lock                                    |
| collector    | `power.lock_command`                    | `["loginctl", "lock-session", "{session}"]` | locks; `{session}` is the display session's id                |
| collector    | `power.suspend`                         | `false`                                     | carry out the panel's Suspend                                 |
| collector    | `power.suspend_command`                 | `["systemctl", "suspend"]`                  | suspends; must return once the sleep is on its way            |
| hub          | `wake.via.<host>`                       | —                                           | `ip` or `ip:port` the packets go to when no network is shared |
| Home Manager | `services.ccdash.power.lock`            | `false`                                     | sets `power.lock`                                             |
| Home Manager | `services.ccdash.power.suspend`         | `false`                                     | sets `power.suspend`                                          |
| NixOS (hub)  | `services.ccdash-hub.settings.wake.via` | `{ }`                                       | sets `wake.via`                                               |

The hub's service allows `AF_NETLINK` so that `getifaddrs` can list its
own networks. Wake-on-LAN itself is the machine's setup: on NixOS,
`networking.interfaces.<if>.wakeOnLan.enable` with the `magic` policy and
the firmware's wake setting.
