//! Containment check and route summarization, for both families.

use crate::calc::{addr, bits, fact, group, mask, parse, render_prompt, width};
use serde_json::{Value, json};

/// Most entries `summarize` accepts.
pub const MAX_ITEMS: usize = 64;

/// Is `ip` (an address or a subnet) inside `net`?
pub fn contains(net: &str, ip: &str) -> Result<Value, String> {
    let n = parse(net)?;
    let q = parse(ip)?;
    let w = width(n.ip);
    let net_bits = bits(n.ip) & mask(n.prefix, w);
    let net_str = format!("{}/{}", addr(net_bits, w), n.prefix);
    let q_str = if q.explicit { format!("{}/{}", q.ip, q.prefix) } else { q.ip.to_string() };

    let mut f = vec![fact(
        "Network",
        &net_str,
        if net_bits == bits(n.ip) {
            "The network you gave.".to_string()
        } else {
            format!("Host bits cleared: {} is an address inside {net_str}, not the network itself.", n.ip)
        },
    )];
    f.push(fact("Checked", &q_str, if q.explicit { "A subnet: inside only if all of it is." } else { "A single address." }));

    let qs = if width(q.ip) != w {
        f.push(fact(
            "Result",
            "outside",
            "IPv4 and IPv6 are separate address spaces. An IPv4 address appears in IPv6 only as ::ffff:a.b.c.d (IPv4-mapped, inside a host) or behind NAT64 (64:ff9b::a.b.c.d).",
        ));
        [
            "How do IPv4 and IPv6 coexist on one network (dual stack)?",
            "How do IPv6-only hosts reach IPv4-only servers (NAT64 and DNS64)?",
            "What are IPv4-mapped addresses and when do I see them?",
        ]
    } else {
        let q_masked = bits(q.ip) & mask(n.prefix, w);
        let inside = q.prefix >= n.prefix && q_masked == net_bits;
        let last = net_bits | (!mask(n.prefix, w) & mask(w, w));
        f.push(fact(
            "Network range",
            format!("{} - {}", addr(net_bits, w), addr(last, w)),
            "Every address whose first bits match the network.",
        ));
        f.push(fact(
            &format!("Checked, masked to /{}", n.prefix),
            format!("{}/{}", addr(q_masked, w), n.prefix),
            "What a router does: AND the address with the prefix mask and compare the result with the network. Equal means inside. This works the same in IPv4 and IPv6.",
        ));
        let why = if q.prefix < n.prefix && q_masked == net_bits {
            format!("The first {} bits match, but /{} is bigger than /{}: it contains the network, not the other way round.", n.prefix, q.prefix, n.prefix)
        } else if inside {
            format!("The first {} bits match.", n.prefix)
        } else {
            format!("The first {} bits differ.", n.prefix)
        };
        f.push(fact("Result", if inside { "inside" } else { "outside" }, why));
        [
            "How does a router use longest-prefix match to pick a route for this address?",
            "How does a host decide whether a destination is on its own link or behind the router, in IPv4 (netmask) and in IPv6 (router advertisements)?",
            "Where do containment checks show up in practice (ACLs, firewall rules, route filters)?",
        ]
    };
    let prompt = render_prompt(&f, "", &qs);
    Ok(json!({"title": format!("Is {q_str} in {net_str}?"), "facts": f, "prompt": prompt}))
}

/// Merge a list of prefixes: the exact minimal set, and the single supernet.
pub fn summarize(input: &str) -> Result<Value, String> {
    let items: Vec<&str> = input.split(|c: char| c == ',' || c.is_whitespace()).filter(|s| !s.is_empty()).collect();
    if items.is_empty() {
        return Err("Enter two or more subnets, separated by commas, spaces or new lines.".into());
    }
    if items.len() > MAX_ITEMS {
        return Err(format!("At most {MAX_ITEMS} subnets."));
    }
    let (mut v4, mut v6) = (vec![], vec![]);
    for it in &items {
        let p = parse(it)?;
        let w = width(p.ip);
        let start = bits(p.ip) & mask(p.prefix, w);
        let end = start | (!mask(p.prefix, w) & mask(w, w));
        if w == 32 { v4.push((start, end)) } else { v6.push((start, end)) }
    }
    let both = !v4.is_empty() && !v6.is_empty();
    let mut f = vec![];
    for (w, ranges) in [(32u8, v4), (128u8, v6)] {
        if !ranges.is_empty() {
            f.extend(summarize_family(w, ranges, if both { if w == 32 { "IPv4 " } else { "IPv6 " } } else { "" }));
        }
    }
    let qs = [
        "When should I advertise the exact summary, and when the single supernet? What goes wrong with the extra addresses?",
        "How does route aggregation work in IPv6 (nibble boundaries, a /48 per site)?",
        "How should I allocate subnets so they summarize cleanly later?",
    ];
    let prompt = render_prompt(&f, "", &qs);
    Ok(json!({"title": format!("Summary of {} prefixes", items.len()), "facts": f, "prompt": prompt}))
}

fn summarize_family(w: u8, mut ranges: Vec<(u128, u128)>, label: &str) -> Vec<Value> {
    let fmt = |(s, p): (u128, u8)| format!("{}/{p}", addr(s, w));
    let inputs: Vec<String> = ranges.iter().map(|&(s, e)| fmt((s, prefix_of(s, e, w)))).collect();

    ranges.sort();
    let mut merged: Vec<(u128, u128)> = vec![];
    for (s, e) in ranges {
        match merged.last_mut() {
            // overlapping or adjacent
            Some(last) if s <= last.1.saturating_add(1) => last.1 = last.1.max(e),
            _ => merged.push((s, e)),
        }
    }
    let exact: Vec<String> = merged.iter().flat_map(|&(s, e)| to_cidrs(s, e, w)).map(fmt).collect();

    let (lo, hi) = (merged[0].0, merged[merged.len() - 1].1);
    let p = ((lo ^ hi).leading_zeros() - (128 - w as u32)) as u8;
    let supernet = fmt((lo & mask(p, w), p));
    let covered = merged.iter().try_fold(0u128, |acc, &(s, e)| (e - s).checked_add(1).and_then(|n| acc.checked_add(n)));
    let size = if w - p == 128 { None } else { Some(1u128 << (w - p)) };
    let extra = match (size, covered) {
        (Some(n), Some(c)) => Some(n - c),
        (None, Some(c)) => Some(u128::MAX - c + 1),
        _ => Some(0),
    };

    vec![
        fact(
            &format!("{label}Inputs"),
            inputs.join(", "),
            format!("{} prefixes, host bits cleared. Duplicates and overlaps are merged.", inputs.len()),
        ),
        fact(
            &format!("{label}Exact summary"),
            exact.join(", "),
            format!(
                "The fewest prefixes ({}) that cover exactly these addresses and nothing more. Safe to advertise or to put in an ACL.",
                exact.len()
            ),
        ),
        fact(
            &format!("{label}Single supernet"),
            &supernet,
            "The smallest single prefix that covers everything: one route instead of several.",
        ),
        fact(
            &format!("{label}Extra addresses in the supernet"),
            count(extra),
            if extra == Some(0) {
                "None: the inputs fill the supernet exactly, so the exact summary is this one prefix.".to_string()
            } else {
                "Addresses you did not list. Advertising the supernet attracts traffic for them too: make sure you own them, or send them to a discard route (Null0).".to_string()
            },
        ),
    ]
}

/// The prefix length of an aligned block [s, e].
fn prefix_of(s: u128, e: u128, w: u8) -> u8 {
    w - (e - s).count_ones() as u8
}

/// The minimal list of aligned prefixes covering exactly [start, end].
fn to_cidrs(start: u128, end: u128, w: u8) -> Vec<(u128, u8)> {
    let block_end = |s: u128, b: u8| if b == 128 { u128::MAX } else { s | ((1u128 << b) - 1) };
    let mut out = vec![];
    let mut s = start;
    loop {
        let mut b = if s == 0 { w } else { (s.trailing_zeros() as u8).min(w) };
        while block_end(s, b) > end {
            b -= 1;
        }
        out.push((s, w - b));
        let be = block_end(s, b);
        if be >= end {
            return out;
        }
        s = be + 1;
    }
}

/// An address count; None means 2^128.
fn count(n: Option<u128>) -> String {
    match n {
        None => "2^128".into(),
        Some(n) if n > 1 << 64 && n.is_power_of_two() => format!("2^{}", n.trailing_zeros()),
        Some(n) if n > 1 << 64 => {
            // nearest power of two: round up when the next bit down is set
            let k = 127 - n.leading_zeros();
            format!("about 2^{}", k + ((n >> (k - 1)) & 1) as u32)
        }
        Some(n) => group(n),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get(v: &Value, label: &str) -> String {
        v["facts"].as_array().unwrap().iter().find(|f| f["label"] == label).unwrap_or_else(|| panic!("no {label} in {v}"))["value"]
            .as_str()
            .unwrap()
            .to_string()
    }

    #[test]
    fn contains_v4() {
        let v = contains("10.0.0.0/22", "10.0.1.77").unwrap();
        assert_eq!(get(&v, "Result"), "inside");
        assert_eq!(get(&v, "Network range"), "10.0.0.0 - 10.0.3.255");
        assert_eq!(get(&v, "Checked, masked to /22"), "10.0.0.0/22");
        assert_eq!(get(&contains("10.0.0.0/22", "10.0.4.0").unwrap(), "Result"), "outside");
        assert_eq!(get(&contains("10.0.0.0/22", "10.0.2.0/23").unwrap(), "Result"), "inside");
        assert_eq!(get(&contains("10.0.0.0/22", "10.0.0.0/21").unwrap(), "Result"), "outside");
        assert_eq!(get(&contains("0.0.0.0/0", "203.0.113.9").unwrap(), "Result"), "inside");
        let v = contains("10.0.1.77/22", "10.0.2.1").unwrap();
        assert_eq!(get(&v, "Network"), "10.0.0.0/22");
        assert!(v["prompt"].as_str().unwrap().contains("longest-prefix match"));
        assert_eq!(v["title"], "Is 10.0.2.1 in 10.0.0.0/22?");
    }

    #[test]
    fn contains_v6_and_mixed() {
        assert_eq!(get(&contains("2001:db8:abcd::/48", "2001:db8:abcd:12::1").unwrap(), "Result"), "inside");
        assert_eq!(get(&contains("2001:db8:abcd::/48", "2001:db8:abce::1").unwrap(), "Result"), "outside");
        assert_eq!(get(&contains("::/0", "2001:db8::1").unwrap(), "Result"), "inside");
        assert_eq!(get(&contains("fe80::/10", "fe80::1%eth0").unwrap(), "Result"), "inside");
        let v = contains("10.0.0.0/8", "2001:db8::1").unwrap();
        assert_eq!(get(&v, "Result"), "outside");
        assert!(v["prompt"].as_str().unwrap().contains("dual stack"));
        assert!(contains("nope", "10.0.0.1").is_err());
    }

    #[test]
    fn summarize_v4() {
        let v = summarize("10.0.0.0/24, 10.0.1.0/24,10.0.2.0/24\n10.0.3.0/24").unwrap();
        assert_eq!(get(&v, "Exact summary"), "10.0.0.0/22");
        assert_eq!(get(&v, "Single supernet"), "10.0.0.0/22");
        assert_eq!(get(&v, "Extra addresses in the supernet"), "0");
        let v = summarize("192.168.0.0/24 192.168.1.0/24 192.168.2.0/24").unwrap();
        assert_eq!(get(&v, "Exact summary"), "192.168.0.0/23, 192.168.2.0/24");
        assert_eq!(get(&v, "Single supernet"), "192.168.0.0/22");
        assert_eq!(get(&v, "Extra addresses in the supernet"), "256");
        // duplicates, overlaps, host bits, single addresses
        let v = summarize("10.1.1.5/24, 10.1.1.0/25, 10.1.1.0/24, 10.1.2.7").unwrap();
        assert_eq!(get(&v, "Inputs"), "10.1.1.0/24, 10.1.1.0/25, 10.1.1.0/24, 10.1.2.7/32");
        assert_eq!(get(&v, "Exact summary"), "10.1.1.0/24, 10.1.2.7/32");
        assert_eq!(get(&v, "Single supernet"), "10.1.0.0/22");
        // unaligned range splits into several prefixes
        let v = summarize("10.0.0.1, 10.0.0.2, 10.0.0.3").unwrap();
        assert_eq!(get(&v, "Exact summary"), "10.0.0.1/32, 10.0.0.2/31");
        assert_eq!(get(&summarize("0.0.0.0/1 128.0.0.0/1").unwrap(), "Exact summary"), "0.0.0.0/0");
        assert!(summarize(" , ").is_err());
        assert!(summarize(&"10.0.0.0/24 ".repeat(65)).is_err());
    }

    #[test]
    fn summarize_v6_and_mixed() {
        let v = summarize("2001:db8:0:0::/64, 2001:db8:0:1::/64").unwrap();
        assert_eq!(get(&v, "Exact summary"), "2001:db8::/63");
        let v = summarize("2001:db8::/64 2001:db8:0:ff::/64").unwrap();
        assert_eq!(get(&v, "Single supernet"), "2001:db8::/56");
        assert_eq!(get(&v, "Extra addresses in the supernet"), "about 2^72");
        let v = summarize("::/1 8000::/1").unwrap();
        assert_eq!(get(&v, "Exact summary"), "::/0");
        assert_eq!(get(&v, "Extra addresses in the supernet"), "0");
        let v = summarize("10.0.0.0/24 2001:db8::/48").unwrap();
        assert_eq!(get(&v, "IPv4 Exact summary"), "10.0.0.0/24");
        assert_eq!(get(&v, "IPv6 Exact summary"), "2001:db8::/48");
        assert!(v["prompt"].as_str().unwrap().contains("nibble boundaries"));
    }

    #[test]
    fn cidr_cover() {
        assert_eq!(to_cidrs(0, u32::MAX as u128, 32), vec![(0, 0)]);
        assert_eq!(to_cidrs(0, u128::MAX, 128), vec![(0, 0)]);
        assert_eq!(to_cidrs(1, 6, 32), vec![(1, 32), (2, 31), (4, 31), (6, 32)]);
    }
}
