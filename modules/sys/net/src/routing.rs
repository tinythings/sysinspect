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
use std::convert::TryFrom;
use std::{
    collections::HashMap,
    error::Error,
    io::Read,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
};

#[derive(Debug)]
pub struct RtRec {
    pub gw: Option<IpAddr>,
    pub src: Option<IpAddr>,
    pub dst: Option<IpAddr>,
    pub proto: Option<Rtprot>,
    pub scope: Option<RtScope>,
    pub dst_len: u8,
    pub iface: String,
}

/// Parse IpAddr
fn to_addr(buff: &[u8]) -> Option<IpAddr> {
    match buff.len() {
        0x4 => Some(IpAddr::from(<[u8; 4]>::try_from(buff).ok()?)),
        0x10 => Some(IpAddr::from(<[u8; 16]>::try_from(buff).ok()?)),
        _ => None,
    }
}

/// Get a route message to a route record structure
fn to_record(ifs: &HashMap<IpAddr, String>, rtm: Nlmsghdr<NlTypeWrapper, Rtmsg>) -> Result<Option<RtRec>, NlError> {
    let pl = rtm.get_payload()?;
    if pl.rtm_table != RtTable::Main {
        return Ok(None);
    }

    let (mut src, mut dst, mut gw, mut proto, mut scope) = (None, None, None, None, None);
    for attr in pl.rtattrs.iter() {
        match attr.rta_type {
            Rta::Dst => dst = to_addr(attr.rta_payload.as_ref()),
            Rta::Prefsrc => src = to_addr(attr.rta_payload.as_ref()),
            Rta::Gateway => gw = to_addr(attr.rta_payload.as_ref()),
            _ => (),
        }
    }

    let mut iface = "".to_string();
    if let Some(src) = src {
        iface = ifs.get(&src).unwrap_or(&"".to_string()).to_string();
    }

    if pl.rtm_scope != RtScope::Universe {
        proto = Some(pl.rtm_protocol);
        scope = Some(pl.rtm_scope);
    }

    Ok(Some(RtRec { gw, src, dst, iface, proto, scope, dst_len: pl.rtm_dst_len }))
}

/// Gather main routing table (mimic "ip route")
pub fn ip_route() -> Result<Vec<RtRec>, Box<dyn Error>> {
    let mut conn = NlSocketHandle::connect(NlFamily::Route, None, &[]).unwrap();
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
        .filter_map(|addr| if let NlPayload::Payload(p) = addr.nl_payload { Some(p) } else { None })
        .try_fold(HashMap::new(), |mut out, rtbuff: Ifaddrmsg| {
            let handle = rtbuff.rtattrs.get_attr_handle();
            let ip_addr =
                handle.get_attr_payload_as_with_len::<&[u8]>(Ifa::Address).ok().and_then(|mut ipdata| match ipdata.len() {
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

            if let (Some(addr), Some(name)) = (ip_addr, handle.get_attr_payload_as_with_len::<String>(Ifa::Label).ok()) {
                out.insert(addr, name);
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
    ))
    .unwrap();

    Ok(conn
        .iter(false)
        .flatten()
        .filter(|rtbl| matches!(rtbl.nl_type, NlTypeWrapper::Rtm(_)))
        .filter_map(|rtbl| to_record(&ifaces, rtbl).transpose())
        .flatten()
        .collect::<Vec<RtRec>>())
}
