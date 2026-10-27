//! The calculator: parse an address, compute the facts, explain each one and
//! build a prompt for the visitor's own AI agent. Pure std, so `cargo test`
//! runs it natively.

use serde_json::{Value, json};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

pub const MAX_INPUT: usize = 64;

/// Parse `addr[/prefix]` and explain it. The error is safe to show the user
/// (it never echoes the input).
pub fn calc(input: &str) -> Result<Value, String> {
    let s = input.trim();
    if s.is_empty() {
        return Err("Enter an IPv4 or IPv6 address, optionally with a /prefix.".into());
    }
    if s.len() > MAX_INPUT {
        return Err(format!("Input is longer than {MAX_INPUT} characters."));
    }
    let (addr, prefix) = match s.split_once('/') {
        Some((a, p)) => (a, Some(p)),
        None => (s, None),
    };
    // fe80::1%eth0: std does not parse zone IDs; drop it and say why.
    let (addr, zone) = match addr.split_once('%') {
        Some((a, _)) => (a, true),
        None => (addr, false),
    };
    let ip: IpAddr = addr
        .parse()
        .map_err(|_| "Not a valid IPv4 or IPv6 address.".to_string())?;
    let max = if ip.is_ipv4() { 32 } else { 128 };
    let p = match prefix {
        None => max,
        Some(p) => match p.parse::<u8>() {
            Ok(n) if n <= max => n,
            _ => return Err(format!("Prefix must be a number from 0 to {max}.")),
        },
    };
    let mut out = match ip {
        IpAddr::V4(v4) => calc_v4(v4, p, prefix.is_some()),
        IpAddr::V6(v6) => calc_v6(v6, p, prefix.is_some()),
    };
    if zone {
        out.facts.insert(
            1,
            fact(
                "Zone ID",
                "removed",
                "The %name after an address picks the interface. Link-local addresses need it: every interface has its own fe80::/64, so fe80::1 alone is ambiguous.",
            ),
        );
    }
    Ok(out.into_json())
}

struct Out {
    family: &'static str,
    canonical: String,
    kind: &'static str,
    facts: Vec<Value>,
}

impl Out {
    fn into_json(self) -> Value {
        let prompt = prompt(&self.facts, self.kind);
        json!({
            "family": self.family,
            "canonical": self.canonical,
            "kind": self.kind,
            "facts": self.facts,
            "prompt": prompt,
        })
    }
}

fn fact(label: &str, value: impl Into<String>, why: impl Into<String>) -> Value {
    json!({"label": label, "value": value.into(), "why": why.into()})
}

struct Range {
    net: u128,
    len: u8,
    kind: &'static str,
    name: &'static str,
    why: &'static str,
    twin: &'static str,
}

#[allow(clippy::too_many_arguments)]
const fn r4(a: u8, b: u8, c: u8, d: u8, len: u8, kind: &'static str, name: &'static str, why: &'static str, twin: &'static str) -> Range {
    Range { net: u32::from_be_bytes([a, b, c, d]) as u128, len, kind, name, why, twin }
}

// IANA IPv4 Special-Purpose Address Registry, most specific first.
const V4_RANGES: &[Range] = &[
    r4(255, 255, 255, 255, 32, "broadcast", "Limited broadcast (RFC 919)", "Sent to every host on the local link, never routed.", "IPv6 has no broadcast. The closest is the all-nodes multicast group ff02::1."),
    r4(192, 0, 0, 0, 24, "special", "IETF protocol assignments (RFC 6890)", "Reserved for protocol use, for example DS-Lite (192.0.0.0/29).", "IPv6 equivalent block: 2001::/23."),
    r4(192, 0, 2, 0, 24, "documentation", "Documentation, TEST-NET-1 (RFC 5737)", "Only for examples in docs. Never routed.", "IPv6 documentation prefixes: 2001:db8::/32 (RFC 3849) and 3fff::/20 (RFC 9637)."),
    r4(198, 51, 100, 0, 24, "documentation", "Documentation, TEST-NET-2 (RFC 5737)", "Only for examples in docs. Never routed.", "IPv6 documentation prefixes: 2001:db8::/32 (RFC 3849) and 3fff::/20 (RFC 9637)."),
    r4(203, 0, 113, 0, 24, "documentation", "Documentation, TEST-NET-3 (RFC 5737)", "Only for examples in docs. Never routed.", "IPv6 documentation prefixes: 2001:db8::/32 (RFC 3849) and 3fff::/20 (RFC 9637)."),
    r4(192, 88, 99, 0, 24, "special", "6to4 relay anycast, deprecated (RFC 7526)", "Was used to tunnel IPv6 over IPv4. Deprecated.", "Paired with the IPv6 6to4 prefix 2002::/16."),
    r4(169, 254, 0, 0, 16, "link-local", "Link-local (RFC 3927)", "Self-assigned when DHCP fails. Works only on the local link.", "IPv6 link-local fe80::/10. Unlike IPv4, every IPv6 interface always has one, and NDP and routing protocols rely on it."),
    r4(192, 168, 0, 0, 16, "private", "Private (RFC 1918)", "Not routed on the internet. Reaches it through NAT.", "IPv6 has ULA fd00::/8 (RFC 4193), but most IPv6 LANs use global addresses with a firewall instead of private addresses with NAT."),
    r4(198, 18, 0, 0, 15, "benchmarking", "Benchmarking (RFC 2544)", "For network device testing labs.", "IPv6 benchmarking prefix: 2001:2::/48 (RFC 5180)."),
    r4(172, 16, 0, 0, 12, "private", "Private (RFC 1918)", "Not routed on the internet. Reaches it through NAT.", "IPv6 has ULA fd00::/8 (RFC 4193), but most IPv6 LANs use global addresses with a firewall instead of private addresses with NAT."),
    r4(100, 64, 0, 0, 10, "cgnat", "Shared address space, carrier-grade NAT (RFC 6598)", "Your ISP puts you behind a second NAT because it ran out of public IPv4 addresses.", "No IPv6 equivalent: IPv6 has enough addresses that ISPs give every customer a whole prefix, so no CGNAT."),
    r4(0, 0, 0, 0, 8, "special", "This network (RFC 791)", "0.0.0.0 means 'no address yet' or 'all interfaces' when a server binds to it.", "IPv6 unspecified address :: (same meaning)."),
    r4(10, 0, 0, 0, 8, "private", "Private (RFC 1918)", "Not routed on the internet. Reaches it through NAT.", "IPv6 has ULA fd00::/8 (RFC 4193), but most IPv6 LANs use global addresses with a firewall instead of private addresses with NAT."),
    r4(127, 0, 0, 0, 8, "loopback", "Loopback (RFC 1122)", "Traffic never leaves the host.", "IPv6 has a single loopback address ::1, not a whole /8."),
    r4(224, 0, 0, 0, 4, "multicast", "Multicast (RFC 5771)", "One sender, a group of receivers.", "IPv6 multicast ff00::/8. IPv6 relies on it far more: it replaces broadcast and ARP."),
    r4(240, 0, 0, 0, 4, "reserved", "Reserved, former class E (RFC 1112)", "Never usable on the internet.", "No IPv6 equivalent."),
];

const V4_PUBLIC: Range = Range {
    net: 0,
    len: 0,
    kind: "public",
    name: "Public unicast",
    why: "Globally routed. Homes usually have only one, shared by every device through NAT.",
    twin: "IPv6 global unicast 2000::/3. Every device gets its own global address, and the router's firewall (not NAT) blocks unsolicited inbound traffic.",
};

fn classify<'a>(ranges: &'a [Range], a: u128, bits: u8, fallback: &'a Range) -> &'a Range {
    ranges
        .iter()
        .find(|r| r.len == 0 || a >> (bits - r.len) == r.net >> (bits - r.len))
        .unwrap_or(fallback)
}

fn calc_v4(ip: Ipv4Addr, p: u8, explicit: bool) -> Out {
    let a = u32::from(ip);
    let mask = if p == 0 { 0 } else { u32::MAX << (32 - p) };
    let net = a & mask;
    let bcast = net | !mask;
    let total = 1u64 << (32 - p);
    let range = classify(V4_RANGES, a as u128, 32, &V4_PUBLIC);
    let v4 = |n: u32| Ipv4Addr::from(n).to_string();

    let mut f = vec![
        fact("Address", ip.to_string(), "32 bits, written as 4 decimal numbers (octets) from 0 to 255."),
        fact(
            "Prefix",
            format!("/{p}"),
            if explicit {
                format!("The first {p} bits name the network; the other {} bits number the hosts in it.", 32 - p)
            } else {
                "No prefix given, so /32: this one address.".into()
            },
        ),
        fact("Netmask", v4(mask), "The prefix written as a mask (1-bits = network part). IPv6 never uses masks, only /prefix."),
        fact("Network", format!("{}/{p}", v4(net)), "The address with all host bits set to 0."),
    ];
    match p {
        32 => f.push(fact("Hosts", "1", "A /32 is one address, used for host routes and loopbacks. IPv6 twin: /128.")),
        31 => f.push(fact(
            "Hosts",
            format!("{} - {} (2)", v4(net), v4(bcast)),
            "A /31 has no network or broadcast address: both are usable on a point-to-point link (RFC 3021). IPv6 twin: /127 (RFC 6164).",
        )),
        _ => {
            f.push(fact(
                "Broadcast",
                v4(bcast),
                "All host bits set to 1: reaches every host on the subnet. IPv6 has no broadcast; it uses multicast (ff02::1 = all nodes).",
            ));
            f.push(fact(
                "Usable hosts",
                format!("{} - {} ({})", v4(net + 1), v4(bcast - 1), group((total - 2) as u128)),
                "Total minus the network and broadcast addresses.",
            ));
        }
    }
    f.push(fact("Addresses", group(total as u128), format!("2^{} addresses in a /{p}.", 32 - p)));
    f.push(fact("Type", range.name, range.why));
    f.push(fact("IPv6 counterpart", range.twin, "Where this kind of address lives in IPv6."));
    f.push(fact(
        "IPv4-mapped IPv6",
        format!("::ffff:{ip}"),
        "How a dual-stack server sees an IPv4 client on an IPv6 socket (RFC 4291). It is never sent on the wire.",
    ));
    if range.kind == "public" {
        f.push(fact(
            "NAT64 address",
            format!("64:ff9b::{ip}"),
            "An IPv6-only client reaches this IPv4 server through a NAT64 gateway at this address (RFC 6052). DNS64 builds it automatically.",
        ));
    }
    f.push(fact(
        "Size in IPv6 terms",
        "a LAN is always /64",
        format!(
            "This /{p} holds {} addresses. One IPv6 LAN (/64) holds 18,446,744,073,709,551,616, so in IPv6 you stop counting hosts and count /64 subnets.",
            group(total as u128)
        ),
    ));
    Out { family: "ipv4", canonical: format!("{}/{p}", ip), kind: range.kind, facts: f }
}

const fn r6(net: u128, len: u8, kind: &'static str, name: &'static str, why: &'static str, twin: &'static str) -> Range {
    Range { net, len, kind, name, why, twin }
}

// IANA IPv6 Special-Purpose Address Registry plus the big allocations,
// most specific first.
const V6_RANGES: &[Range] = &[
    r6(0, 128, "unspecified", "Unspecified (RFC 4291)", "Means 'no address yet', or 'all interfaces' when a server binds to [::].", "IPv4 0.0.0.0."),
    r6(1, 128, "loopback", "Loopback (RFC 4291)", "Traffic never leaves the host.", "IPv4 127.0.0.1 (IPv4 reserves all of 127.0.0.0/8)."),
    r6(0xffff_0000_0000, 96, "mapped", "IPv4-mapped (RFC 4291)", "An IPv4 address seen through an IPv6 socket on a dual-stack host. Never on the wire.", "The embedded IPv4 address."),
    r6(0x0064_ff9b_0000_0000_0000_0000_0000_0000, 96, "nat64", "NAT64 well-known prefix (RFC 6052)", "An IPv6-only client reaches an IPv4 server through a NAT64 gateway; the last 32 bits are the IPv4 address.", "The embedded IPv4 address."),
    r6(0x0064_ff9b_0001_0000_0000_0000_0000_0000, 48, "nat64", "NAT64 local-use prefix (RFC 8215)", "A NAT64 prefix for one network's own translators.", "The embedded IPv4 address."),
    r6(0x0100_0000_0000_0000_0000_0000_0000_0000, 64, "special", "Discard-only (RFC 6666)", "Routed to a black hole on purpose, to drop attack traffic.", "No IPv4 equivalent."),
    r6(0x2001_0002_0000_0000_0000_0000_0000_0000, 48, "benchmarking", "Benchmarking (RFC 5180)", "For network device testing labs.", "IPv4 198.18.0.0/15."),
    r6(0x2001_0000_0000_0000_0000_0000_0000_0000, 32, "teredo", "Teredo tunneling (RFC 4380)", "Old way to tunnel IPv6 through IPv4 NAT. Bits 32-63 hold the Teredo server's IPv4 address.", "The Teredo server's IPv4 address is embedded."),
    r6(0x2001_0db8_0000_0000_0000_0000_0000_0000, 32, "documentation", "Documentation (RFC 3849)", "Only for examples in docs. Never routed.", "IPv4 192.0.2.0/24, 198.51.100.0/24 and 203.0.113.0/24 (RFC 5737)."),
    r6(0x2002_0000_0000_0000_0000_0000_0000_0000, 16, "6to4", "6to4, deprecated (RFC 3056)", "Old automatic tunnel: bits 16-47 hold the site's public IPv4 address.", "The embedded public IPv4 address."),
    r6(0x3fff_0000_0000_0000_0000_0000_0000_0000, 20, "documentation", "Documentation (RFC 9637)", "Only for examples in docs, sized for whole-network examples. Never routed.", "IPv4 192.0.2.0/24, 198.51.100.0/24 and 203.0.113.0/24 (RFC 5737)."),
    r6(0xfc00_0000_0000_0000_0000_0000_0000_0000, 7, "ula", "Unique local, ULA (RFC 4193)", "Private addresses: not routed on the internet. fd00::/8 is the usable half; the 40 bits after fd are a random Global ID so two networks rarely clash when merged.", "IPv4 private 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16 (RFC 1918). Unlike IPv4, hosts usually also have a global address, so no NAT is needed."),
    r6(0xfe80_0000_0000_0000_0000_0000_0000_0000, 10, "link-local", "Link-local (RFC 4291)", "Every IPv6 interface always has one (fe80::/64 in practice). Used for NDP, router advertisements and routing protocols; never forwarded by routers.", "IPv4 169.254.0.0/16, which only appears when DHCP fails."),
    r6(0xff00_0000_0000_0000_0000_0000_0000_0000, 8, "multicast", "Multicast (RFC 4291)", "One sender, a group of receivers. IPv6 uses multicast where IPv4 used broadcast and ARP.", "IPv4 224.0.0.0/4."),
    r6(0x2000_0000_0000_0000_0000_0000_0000_0000, 3, "global", "Global unicast (RFC 4291)", "A public, routable address.", "A public IPv4 address. Difference: IPv4 homes share one public address through NAT; in IPv6 every device has its own global address and the router's firewall blocks unsolicited inbound traffic."),
];

const V6_RESERVED: Range = Range {
    net: 0,
    len: 0,
    kind: "reserved",
    name: "Reserved by the IETF (RFC 4291)",
    why: "Not allocated for use yet.",
    twin: "IPv4 240.0.0.0/4 (also reserved).",
};

fn expanded(a: u128) -> String {
    groups(a).iter().map(|g| format!("{g:04x}")).collect::<Vec<_>>().join(":")
}

fn groups(a: u128) -> [u16; 8] {
    Ipv6Addr::from(a).segments()
}

/// Longest run of 2+ zero groups (leftmost on a tie), as (start, len): the
/// part RFC 5952 replaces with `::`.
fn zero_run(g: &[u16; 8]) -> Option<(usize, usize)> {
    let mut best: Option<(usize, usize)> = None;
    let mut i = 0;
    while i < 8 {
        if g[i] == 0 {
            let start = i;
            while i < 8 && g[i] == 0 {
                i += 1;
            }
            let len = i - start;
            if len >= 2 && best.is_none_or(|(_, l)| len > l) {
                best = Some((start, len));
            }
        } else {
            i += 1;
        }
    }
    best
}

fn calc_v6(ip: Ipv6Addr, p: u8, explicit: bool) -> Out {
    let a = u128::from(ip);
    let mask = if p == 0 { 0 } else { u128::MAX << (128 - p) };
    let net = a & mask;
    let last = net | !mask;
    let range = classify(V6_RANGES, a, 128, &V6_RESERVED);
    let v6 = |n: u128| Ipv6Addr::from(n).to_string();
    let g = groups(a);

    let short_groups = g.iter().map(|x| format!("{x:x}")).collect::<Vec<_>>().join(":");
    let compress_why = match zero_run(&g) {
        Some((start, len)) => format!(
            "Rules (RFC 5952): 1) drop leading zeros in each group: {short_groups}. 2) replace the longest run of zero groups (here groups {}-{}, {len} of them) with ::, only once. 3) lowercase.",
            start + 1,
            start + len
        ),
        None => format!(
            "Rules (RFC 5952): 1) drop leading zeros in each group: {short_groups}. 2) :: replaces a run of 2+ zero groups; there is none here. 3) lowercase."
        ),
    };

    let mut f = vec![
        fact("Address", v6(a), compress_why),
        fact("Expanded", expanded(a), "128 bits, written as 8 groups of 4 hex digits (16 bits each). This is the form to compare or sort."),
        fact(
            "Prefix",
            format!("/{p}"),
            if explicit {
                format!("The first {p} bits name the network; the other {} bits identify the interface or subnets inside it.", 128 - p)
            } else {
                "No prefix given, so /128: this one address.".into()
            },
        ),
        fact("Network", format!("{}/{p}", v6(net)), "The address with all bits after the prefix set to 0."),
        fact("Last address", v6(last), "IPv6 has no broadcast, so the last address is usable too."),
        fact("Addresses", pow2(128 - p), format!("2^{} addresses in a /{p}.", 128 - p)),
    ];
    f.push(match p {
        0..=64 => fact(
            "/64 subnets",
            pow2(64 - p),
            "An IPv6 LAN is always a /64: SLAAC needs 64 bits for the interface ID (RFC 7421). A home ISP typically delegates a /56 (256 LANs); a business a /48 (65,536 LANs).",
        ),
        127 => fact("/64 subnets", "none", "A /127 is a point-to-point link between two routers (RFC 6164), like an IPv4 /31."),
        128 => fact("/64 subnets", "none", "A /128 is one address, used for host routes and loopbacks, like an IPv4 /32."),
        _ => fact("/64 subnets", "none", "Smaller than a /64: SLAAC does not work on it. Use it only for static routes or point-to-point links."),
    });
    f.push(fact("Type", range.name, range.why));
    f.push(fact("IPv4 counterpart", range.twin, "Where this kind of address lives in IPv4."));

    if let Some(v4) = embedded_v4(a, range.kind) {
        f.push(fact("Embedded IPv4", v4.to_string(), "The IPv4 address carried inside this IPv6 address."));
    }

    let unicast = matches!(range.kind, "global" | "ula" | "link-local" | "documentation" | "benchmarking" | "6to4" | "teredo" | "reserved");
    if matches!(range.kind, "global" | "ula" | "documentation") {
        // ULA layout is fixed (fd + 40-bit Global ID = /48); otherwise use the
        // given prefix, or assume the usual /48 site for a single address.
        let r = if range.kind == "ula" || p > 64 { 48 } else { p };
        let rmask = if r == 0 { 0 } else { u128::MAX << (128 - r) };
        let subnet_bits = 64 - r;
        let subnet = if subnet_bits == 0 { 0 } else { (a >> 64) & ((1u128 << subnet_bits) - 1) };
        let why = if range.kind == "ula" {
            "Bits 1-48: fd plus a 40-bit random Global ID. Identifies the site; never routed on the internet.".to_string()
        } else if r == p {
            format!("Bits 1-{r}: identifies the site on the internet. A RIR or ISP assigns it.")
        } else {
            format!("Bits 1-{r} (assuming the usual /48 site): identifies the site on the internet. A RIR or ISP assigns it.")
        };
        f.push(fact("Routing prefix", format!("{}/{r}", v6(a & rmask)), why));
        if subnet_bits > 0 {
            f.push(fact(
                "Subnet ID",
                format!("{subnet:x}"),
                format!("Bits {}-64: {subnet_bits} bits the site uses to number its own /64 LANs.", r + 1),
            ));
        }
    }
    if unicast {
        let iid = a as u64;
        let iid_str = format!("{:x}:{:x}:{:x}:{:x}", iid >> 48, (iid >> 32) & 0xffff, (iid >> 16) & 0xffff, iid & 0xffff);
        let why = if (iid >> 24) & 0xffff == 0xfffe {
            let b = iid.to_be_bytes();
            format!(
                "Built from MAC {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} (EUI-64: ff:fe in the middle, bit 7 flipped). Modern systems avoid this because it tracks a device across networks.",
                b[0] ^ 0x02, b[1], b[2], b[5], b[6], b[7]
            )
        } else if iid < 0x1_0000 {
            "A small number: set by hand or by DHCPv6 (for example ::1 for a router).".into()
        } else {
            "Looks random: a privacy address (RFC 8981, changes over time) or a stable-opaque one (RFC 7217). The host picks it itself with SLAAC.".into()
        };
        f.push(fact("Interface ID", iid_str, format!("The last 64 bits name the interface on its LAN. {why}")));
        let sn = 0xff02_0000_0000_0000_0000_0001_ff00_0000u128 | (a & 0xff_ffff);
        f.push(fact(
            "Solicited-node group",
            v6(sn),
            "NDP replaces ARP: to learn this host's MAC address, a neighbor asks this multicast group (ff02::1:ff + the last 24 bits) instead of broadcasting to everyone.",
        ));
    }
    if range.kind == "multicast" {
        let scope = match (a >> 112) & 0xf {
            0x1 => "interface-local",
            0x2 => "link-local (never leaves the LAN)",
            0x4 => "admin-local",
            0x5 => "site-local",
            0x8 => "organization-local",
            0xe => "global",
            _ => "reserved or unassigned",
        };
        f.push(fact("Multicast scope", scope, "The 4th hex digit (ff0X) says how far the packet may travel. IPv4 multicast uses TTL and address ranges for this."));
        let known = match a {
            0xff02_0000_0000_0000_0000_0000_0000_0001 => Some("All nodes on the link: the closest thing to IPv4 broadcast."),
            0xff02_0000_0000_0000_0000_0000_0000_0002 => Some("All routers on the link: hosts send Router Solicitations here."),
            0xff02_0000_0000_0000_0000_0000_0001_0002 => Some("All DHCPv6 servers and relays on the link."),
            0xff02_0000_0000_0000_0000_0000_0000_00fb => Some("mDNS (Bonjour, .local names); IPv4 uses 224.0.0.251."),
            _ if a & !0xff_ffff == 0xff02_0000_0000_0000_0000_0001_ff00_0000 => {
                Some("A solicited-node group: NDP's replacement for ARP requests.")
            }
            _ => None,
        };
        if let Some(k) = known {
            f.push(fact("Well-known group", v6(a), k));
        }
    }
    Out { family: "ipv6", canonical: format!("{}/{p}", v6(a)), kind: range.kind, facts: f }
}

fn embedded_v4(a: u128, kind: &str) -> Option<Ipv4Addr> {
    match kind {
        "mapped" | "nat64" => Some(Ipv4Addr::from(a as u32)),
        "6to4" => Some(Ipv4Addr::from((a >> 80) as u32)),
        "teredo" => Some(Ipv4Addr::from((a >> 64) as u32)),
        _ => None,
    }
}

fn questions(kind: &str) -> [&'static str; 3] {
    match kind {
        "private" | "cgnat" => [
            "What replaces RFC 1918 addresses and NAT in IPv6, and do I still need NAT?",
            "Should my LAN use ULA (fd00::/8), global addresses, or both?",
            "How do I size IPv6 subnets for a network this size?",
        ],
        "public" | "global" => [
            "Without NAT, what protects devices with global IPv6 addresses?",
            "How does a host get this kind of address: SLAAC, DHCPv6, or both?",
            "How much address space does an ISP usually give, and how do I split it?",
        ],
        "link-local" => [
            "Why does every IPv6 interface have a link-local address, even with a global one?",
            "How does NDP use link-local addresses where IPv4 uses ARP?",
            "Why do I need a zone ID (%eth0) to ping a link-local address?",
        ],
        "ula" => [
            "When should I use ULA instead of global addresses?",
            "How do I pick a ULA prefix, and why is the Global ID random?",
            "Can a host have ULA and global addresses at the same time? Which does it use?",
        ],
        "multicast" => [
            "How does IPv6 do without broadcast?",
            "How does NDP use multicast to find neighbors and routers?",
            "What do the multicast scope values mean in practice?",
        ],
        "mapped" | "nat64" | "6to4" | "teredo" => [
            "How do IPv6-only and IPv4-only hosts talk to each other?",
            "What are NAT64 and DNS64, and when would I deploy them?",
            "Which transition mechanisms are still in use, and which are dead?",
        ],
        _ => [
            "How does this address type compare with its IPv4 equivalent?",
            "How do hosts get IPv6 addresses: SLAAC, DHCPv6, or both?",
            "What should I learn first to run a dual-stack network?",
        ],
    }
}

fn prompt(facts: &[Value], kind: &str) -> String {
    let mut s = String::from(
        "I'm an engineer who knows IPv4 well and is learning IPv6.\n\
         Explain this result to me, comparing each point with its IPv4 equivalent.\n\
         Don't recompute the values below; they come from ipcalc.franzramadhan.dev and are already verified.\n\
         Ask me before assuming anything about my network.\n\n",
    );
    for f in facts {
        s.push_str(&format!("{}: {}\n", f["label"].as_str().unwrap_or(""), f["value"].as_str().unwrap_or("")));
    }
    s.push_str("\nMy questions:\n");
    for (i, q) in questions(kind).iter().enumerate() {
        s.push_str(&format!("{}. {q}\n", i + 1));
    }
    s.push_str("4. <your question>\n");
    s
}

/// 2^n, exact with separators up to 2^64, then as a power.
fn pow2(n: u8) -> String {
    if n <= 64 { group(1u128 << n) } else { format!("2^{n}") }
}

fn group(n: u128) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get(v: &Value, label: &str) -> String {
        v["facts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["label"] == label)
            .unwrap_or_else(|| panic!("no fact {label} in {v}"))["value"]
            .as_str()
            .unwrap()
            .to_string()
    }

    #[test]
    fn v4_subnet() {
        let v = calc("192.168.1.77/24").unwrap();
        assert_eq!(v["kind"], "private");
        assert_eq!(get(&v, "Netmask"), "255.255.255.0");
        assert_eq!(get(&v, "Network"), "192.168.1.0/24");
        assert_eq!(get(&v, "Broadcast"), "192.168.1.255");
        assert_eq!(get(&v, "Usable hosts"), "192.168.1.1 - 192.168.1.254 (254)");
        assert_eq!(get(&v, "Addresses"), "256");
    }

    #[test]
    fn v4_edges() {
        let v = calc("10.0.0.1/31").unwrap();
        assert_eq!(get(&v, "Hosts"), "10.0.0.0 - 10.0.0.1 (2)");
        let v = calc("8.8.8.8").unwrap();
        assert_eq!(v["kind"], "public");
        assert_eq!(get(&v, "Hosts"), "1");
        assert_eq!(get(&v, "NAT64 address"), "64:ff9b::8.8.8.8");
        assert_eq!(get(&v, "Addresses"), "1");
        let v = calc("0.0.0.0/0").unwrap();
        assert_eq!(get(&v, "Addresses"), "4,294,967,296");
        assert_eq!(get(&v, "Netmask"), "0.0.0.0");
    }

    #[test]
    fn v4_ranges() {
        for (ip, kind) in [
            ("100.64.0.1", "cgnat"),
            ("100.128.0.1", "public"),
            ("172.31.255.255", "private"),
            ("172.32.0.0", "public"),
            ("169.254.1.1", "link-local"),
            ("127.0.0.1", "loopback"),
            ("198.19.0.1", "benchmarking"),
            ("203.0.113.9", "documentation"),
            ("224.0.0.251", "multicast"),
            ("255.255.255.255", "broadcast"),
            ("250.0.0.1", "reserved"),
        ] {
            assert_eq!(calc(ip).unwrap()["kind"], kind, "{ip}");
        }
    }

    #[test]
    fn v6_compression_rfc5952() {
        // RFC 5952 section 4 examples
        for (input, want) in [
            ("2001:0db8:0000:0000:0000:0000:0002:0001", "2001:db8::2:1"),
            ("2001:db8:0:1:1:1:1:1", "2001:db8:0:1:1:1:1:1"),
            ("2001:db8:0:0:1:0:0:1", "2001:db8::1:0:0:1"),
            ("2001:DB8::1", "2001:db8::1"),
        ] {
            assert_eq!(get(&calc(input).unwrap(), "Address"), want, "{input}");
        }
        let v = calc("2001:db8::2:1").unwrap();
        assert_eq!(get(&v, "Expanded"), "2001:0db8:0000:0000:0000:0000:0002:0001");
    }

    #[test]
    fn zero_run_rules() {
        assert_eq!(zero_run(&[1, 0, 0, 1, 0, 0, 0, 1]), Some((4, 3)));
        assert_eq!(zero_run(&[1, 0, 0, 1, 0, 0, 1, 1]), Some((1, 2))); // tie: leftmost
        assert_eq!(zero_run(&[1, 0, 1, 1, 1, 1, 1, 1]), None); // single zero group stays
    }

    #[test]
    fn v6_subnet() {
        let v = calc("2001:db8:abcd:12::1/64").unwrap();
        assert_eq!(v["kind"], "documentation");
        assert_eq!(get(&v, "Network"), "2001:db8:abcd:12::/64");
        assert_eq!(get(&v, "Last address"), "2001:db8:abcd:12:ffff:ffff:ffff:ffff");
        assert_eq!(get(&v, "Addresses"), "18,446,744,073,709,551,616");
        assert_eq!(get(&v, "/64 subnets"), "1");
        assert_eq!(get(&v, "Routing prefix"), "2001:db8:abcd:12::/64");
        assert_eq!(get(&v, "Interface ID"), "0:0:0:1");
        assert_eq!(get(&v, "Solicited-node group"), "ff02::1:ff00:1");
        let v = calc("2001:db8:abcd::/56").unwrap();
        assert_eq!(get(&v, "/64 subnets"), "256");
        assert_eq!(get(&v, "Addresses"), "2^72");
        let v = calc("2001:db8:abcd:12::1").unwrap();
        assert_eq!(get(&v, "Routing prefix"), "2001:db8:abcd::/48");
        assert_eq!(get(&v, "Subnet ID"), "12");
    }

    #[test]
    fn v6_ranges() {
        for (ip, kind) in [
            ("::", "unspecified"),
            ("::1", "loopback"),
            ("::ffff:192.0.2.1", "mapped"),
            ("64:ff9b::192.0.2.33", "nat64"),
            ("2001::1", "teredo"),
            ("2001:2::1", "benchmarking"),
            ("2002:c000:204::1", "6to4"),
            ("3fff::1", "documentation"),
            ("fd12:3456:789a::1", "ula"),
            ("fe80::1", "link-local"),
            ("ff02::1", "multicast"),
            ("2606:4700::1111", "global"),
            ("4000::1", "reserved"),
        ] {
            assert_eq!(calc(ip).unwrap()["kind"], kind, "{ip}");
        }
    }

    #[test]
    fn v6_embedded_and_eui64() {
        assert_eq!(get(&calc("64:ff9b::192.0.2.33").unwrap(), "Embedded IPv4"), "192.0.2.33");
        assert_eq!(get(&calc("2002:c000:204::1").unwrap(), "Embedded IPv4"), "192.0.2.4");
        let v = calc("fe80::21b:21ff:fe3c:4d5e%eth0").unwrap();
        assert_eq!(get(&v, "Zone ID"), "removed");
        let iid = v["facts"].as_array().unwrap().iter().find(|f| f["label"] == "Interface ID").unwrap();
        assert!(iid["why"].as_str().unwrap().contains("00:1b:21:3c:4d:5e"), "{iid}");
        assert_eq!(get(&calc("ff02::2").unwrap(), "Multicast scope"), "link-local (never leaves the LAN)");
        assert!(calc("ff02::1:ff00:1").unwrap()["facts"].to_string().contains("solicited-node group"));
    }

    #[test]
    fn rejects_bad_input() {
        for bad in ["", "1.2.3", "1.2.3.4/33", "::/129", "2001:db8::/x", "hello", &"1".repeat(65)] {
            assert!(calc(bad).is_err(), "{bad}");
        }
        // errors never echo the input
        assert!(!calc("<script>").unwrap_err().contains("script"));
    }

    #[test]
    fn prompt_has_facts_and_questions() {
        let v = calc("192.168.1.0/24").unwrap();
        let p = v["prompt"].as_str().unwrap();
        assert!(p.contains("Network: 192.168.1.0/24"));
        assert!(p.contains("1. What replaces RFC 1918"));
        assert!(p.contains("4. <your question>"));
    }
}
