use std::{error::Error, net::IpAddr};

#[cfg(target_os = "linux")]
use std::collections::HashMap;
#[cfg(target_os = "freebsd")]
use std::process::Command;

#[derive(Debug)]
pub struct RtRec {
    pub gw: Option<IpAddr>,
    pub src: Option<IpAddr>,
    pub dst: Option<IpAddr>,
    pub proto: Option<String>,
    pub scope: Option<String>,
    pub dst_len: u8,
    pub iface: String,
}

#[cfg(target_os = "linux")]
use neli::{
    consts::{
        nl::{NlTypeWrapper, NlmF, NlmFFlags},
        rtnl::{Ifa, IfaFFlags, RtAddrFamily, RtScope, RtTable, Rta, Rtm, RtmFFlags, Rtn, Rtprot},
        socket::NlFamily,
    },
    err::NlError,
    nl::{NlPayload, Nlmsghdr},
    rtnl::{Ifaddrmsg, Rtmsg},
    socket::NlSocketHandle,
    types::RtBuffer,
};
#[cfg(target_os = "linux")]
use std::{convert::TryFrom, io::Read, net::{Ipv4Addr, Ipv6Addr}};

#[cfg(target_os = "linux")]
fn to_addr(buff: &[u8]) -> Option<IpAddr> {
    match buff.len() {
        0x4 => Some(IpAddr::from(<[u8; 4]>::try_from(buff).ok()?)),
        0x10 => Some(IpAddr::from(<[u8; 16]>::try_from(buff).ok()?)),
        _ => None,
    }
}

#[cfg(target_os = "linux")]
fn proto_name(proto: Rtprot) -> Option<String> {
    Some(
        match proto {
            Rtprot::Unspec => "unspecified",
            Rtprot::Redirect => "redirect",
            Rtprot::Kernel => "kernel",
            Rtprot::Boot => "boot",
            Rtprot::Static => "static",
            _ => return None,
        }
        .to_string(),
    )
}

#[cfg(target_os = "linux")]
fn scope_name(scope: RtScope) -> Option<String> {
    Some(
        match scope {
            RtScope::Universe => "universe",
            RtScope::Site => "site",
            RtScope::Link => "link",
            RtScope::Host => "host",
            RtScope::Nowhere => "nowhere",
            _ => return None,
        }
        .to_string(),
    )
}

#[cfg(target_os = "linux")]
fn to_record(ifs: &HashMap<IpAddr, String>, rtm: Nlmsghdr<NlTypeWrapper, Rtmsg>) -> Result<Option<RtRec>, NlError> {
    let payload = rtm.get_payload()?;
    if payload.rtm_table != RtTable::Main {
        return Ok(None);
    }

    let (mut src, mut dst, mut gw) = (None, None, None);
    for attr in payload.rtattrs.iter() {
        match attr.rta_type {
            Rta::Dst => dst = to_addr(attr.rta_payload.as_ref()),
            Rta::Prefsrc => src = to_addr(attr.rta_payload.as_ref()),
            Rta::Gateway => gw = to_addr(attr.rta_payload.as_ref()),
            _ => (),
        }
    }

    Ok(Some(RtRec {
        gw,
        src,
        dst,
        proto: (payload.rtm_scope != RtScope::Universe).then(|| proto_name(payload.rtm_protocol)).flatten(),
        scope: (payload.rtm_scope != RtScope::Universe).then(|| scope_name(payload.rtm_scope)).flatten(),
        dst_len: payload.rtm_dst_len,
        iface: src.and_then(|addr| ifs.get(&addr).cloned()).unwrap_or_default(),
    }))
}

#[cfg(target_os = "linux")]
pub fn ip_route() -> Result<Vec<RtRec>, Box<dyn Error>> {
    let mut conn = NlSocketHandle::connect(NlFamily::Route, None, &[])?;
    conn.send(Nlmsghdr::new(
        None,
        Rtm::Getaddr,
        NlmFFlags::new(&[NlmF::Request, NlmF::Dump]),
        None,
        None,
        NlPayload::Payload(Ifaddrmsg {
            ifa_family: RtAddrFamily::Unspecified,
            ifa_prefixlen: 0,
            ifa_flags: IfaFFlags::empty(),
            ifa_scope: 0,
            ifa_index: 0,
            rtattrs: RtBuffer::new(),
        }),
    ))?;

    let ifaces = conn
        .iter::<Rtm, _>(false)
        .flatten()
        .filter_map(|addr| if let NlPayload::Payload(payload) = addr.nl_payload { Some(payload) } else { None })
        .try_fold(HashMap::new(), |mut out, payload: Ifaddrmsg| {
            let handle = payload.rtattrs.get_attr_handle();
            let addr = handle.get_attr_payload_as_with_len::<&[u8]>(Ifa::Address).ok().and_then(|mut ipdata| match ipdata.len() {
                0x4 => {
                    let mut buff = [0u8; 4];
                    ipdata.read_exact(&mut buff).ok()?;
                    Some(IpAddr::from(Ipv4Addr::from(u32::from_ne_bytes(buff).to_be())))
                }
                0x10 => {
                    let mut buff = [0u8; 16];
                    ipdata.read_exact(&mut buff).ok()?;
                    Some(IpAddr::from(Ipv6Addr::from(u128::from_ne_bytes(buff).to_be())))
                }
                _ => None,
            });

            if let (Some(address), Some(name)) = (addr, handle.get_attr_payload_as_with_len::<String>(Ifa::Label).ok()) {
                out.insert(address, name);
            }

            Result::<_, NlError>::Ok(out)
        })?;

    conn.send(Nlmsghdr::new(
        None,
        Rtm::Getroute,
        NlmFFlags::new(&[NlmF::Request, NlmF::Dump]),
        None,
        None,
        NlPayload::Payload(Rtmsg {
            rtm_family: RtAddrFamily::Inet,
            rtm_dst_len: 0,
            rtm_src_len: 0,
            rtm_tos: 0,
            rtm_table: RtTable::Unspec,
            rtm_protocol: Rtprot::Unspec,
            rtm_scope: RtScope::Universe,
            rtm_type: Rtn::Unspec,
            rtm_flags: RtmFFlags::empty(),
            rtattrs: RtBuffer::new(),
        }),
    ))?;

    Ok(
        conn.iter(false)
            .flatten()
            .filter(|route| matches!(route.nl_type, NlTypeWrapper::Rtm(_)))
            .filter_map(|route| to_record(&ifaces, route).transpose())
            .flatten()
            .collect(),
    )
}

#[cfg(target_os = "freebsd")]
fn parse_prefix(dst: &str) -> (Option<IpAddr>, u8) {
    if dst.eq_ignore_ascii_case("default") {
        return (None, 0);
    }

    dst.split_once('/').map_or_else(
        || {
            dst.parse::<IpAddr>().map_or((None, 0), |addr| {
                (
                    Some(addr),
                    match addr {
                        IpAddr::V4(_) => 32,
                        IpAddr::V6(_) => 128,
                    },
                )
            })
        },
        |(addr, mask)| {
            addr.parse::<IpAddr>()
                .ok()
                .zip(mask.parse::<u8>().ok())
                .map_or((None, 0), |(parsed, prefix)| (Some(parsed), prefix))
        },
    )
}

#[cfg(target_os = "freebsd")]
fn parse_route_line(line: &str) -> Option<RtRec> {
    let fields = line.split_whitespace().collect::<Vec<_>>();
    (fields.len() >= 4).then_some(fields).and_then(|fields| {
        let (dst, dst_len) = parse_prefix(fields[0]);
        Some(RtRec {
            gw: fields[1].parse::<IpAddr>().ok(),
            src: None,
            dst,
            proto: None,
            scope: None,
            dst_len,
            iface: fields[fields.len() - 1].to_string(),
        })
    })
}

#[cfg(target_os = "freebsd")]
fn routes_for(family: &str) -> Vec<RtRec> {
    Command::new("netstat")
        .args(["-rn", "-f", family])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .map(|stdout| {
            stdout
                .lines()
                .skip_while(|line| !line.trim_start().starts_with("Destination"))
                .skip(1)
                .filter(|line| !line.trim().is_empty())
                .filter_map(parse_route_line)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(target_os = "freebsd")]
pub fn ip_route() -> Result<Vec<RtRec>, Box<dyn Error>> {
    Ok(routes_for("inet").into_iter().chain(routes_for("inet6")).collect())
}
