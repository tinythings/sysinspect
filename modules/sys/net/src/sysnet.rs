use crate::routing;
use libsysinspect::{
    modlib::{
        response::ModResponse,
        runtime::{self, ModRequest},
    },
    SysinspectError,
};
use nix::{
    ifaddrs::{getifaddrs, InterfaceAddress},
    net::if_::InterfaceFlags,
    sys::socket::{AddressFamily, SockaddrLike},
};
use serde_json::json;
use std::collections::{hash_map::Entry, HashMap};

struct NetInfo {
    ifaces: Vec<InterfaceAddress>,
}

impl NetInfo {
    /// NetInfo instance
    fn new() -> Result<Self, SysinspectError> {
        if let Ok(itr) = getifaddrs() {
            let mut ifaces: Vec<InterfaceAddress> = Vec::default();
            for iface in itr {
                ifaces.push(iface);
            }
            Ok(NetInfo { ifaces })
        } else {
            Err(SysinspectError::ModuleError("Unable to retrieve interfaces data".to_string()))
        }
    }

    fn format_mac(mac: &[u8]) -> String {
        mac.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(":")
    }

    /// Get interfaces
    fn interfaces(&self) -> HashMap<String, Vec<HashMap<String, serde_json::Value>>> {
        let mut out: HashMap<String, Vec<HashMap<String, serde_json::Value>>> = HashMap::default();
        for iface in &self.ifaces {
            if !iface.flags.contains(InterfaceFlags::IFF_UP) {
                continue;
            }

            let mut item: HashMap<String, serde_json::Value> = HashMap::default();

            if let Some(addr) = iface.address {
                if let Some(family) = addr.family() {
                    if let (AddressFamily::Inet, Some(ip)) = (family, addr.as_sockaddr_in()) {
                        item.insert("port".to_string(), json!(ip.port()));
                        item.insert("IPv4".to_string(), json!(ip.ip()));
                    } else if let (AddressFamily::Inet6, Some(ip)) = (family, addr.as_sockaddr_in6()) {
                        item.insert("port".to_string(), json!(ip.port()));
                        item.insert("IPv6".to_string(), json!(ip.ip()));
                        item.insert("scope".to_string(), json!(ip.scope_id()));
                    } else if let (AddressFamily::Packet, Some(link)) = (family, addr.as_link_addr()) {
                        if let Some(mac) = link.addr() {
                            item.insert("mac".to_string(), json!(Self::format_mac(&mac).to_ascii_uppercase().to_string()));
                        }
                    }
                }
            }

            if !item.is_empty() {
                let ifn = iface.interface_name.to_string();
                if let Entry::Vacant(e) = out.entry(ifn.to_owned()) {
                    e.insert(vec![item]);
                } else {
                    out.get_mut(&ifn).unwrap().push(item);
                }
            }
        }

        out
    }
}

/// Get data
fn get_data(rt: &ModRequest, netinfo: &NetInfo) -> HashMap<String, serde_json::Value> {
    let mut data: HashMap<String, serde_json::Value> = HashMap::default();

    // Include running interfaces
    if runtime::get_opt(rt, "if-up") {
        data.insert("if-up".to_string(), json!(netinfo.interfaces()));
    }

    if runtime::get_opt(rt, "route-table") {
        if let Ok(rt_data) = routing::ip_route() {
            let mut rtable: Vec<HashMap<String, String>> = Vec::default();
            for entry in &rt_data {
                let mut rec: HashMap<String, String> = HashMap::default();
                if let Some(gw) = entry.gw {
                    rec.insert("gateway".to_string(), gw.to_string());
                }
                if let Some(src) = entry.src {
                    rec.insert("src".to_string(), src.to_string());
                }
                if let Some(dst) = entry.dst {
                    rec.insert("dst".to_string(), dst.to_string());
                }
                if let Some(proto) = entry.proto {
                    rec.insert(
                        "proto".to_string(),
                        match proto {
                            neli::consts::rtnl::Rtprot::Unspec => "unspecified",
                            neli::consts::rtnl::Rtprot::Redirect => "redirect",
                            neli::consts::rtnl::Rtprot::Kernel => "kernel",
                            neli::consts::rtnl::Rtprot::Boot => "boot",
                            neli::consts::rtnl::Rtprot::Static => "static",
                            _ => "",
                        }
                        .to_string(),
                    );
                }
                if let Some(scope) = entry.scope {
                    rec.insert(
                        "scope".to_string(),
                        match scope {
                            neli::consts::rtnl::RtScope::Universe => "universe",
                            neli::consts::rtnl::RtScope::Site => "site",
                            neli::consts::rtnl::RtScope::Link => "link",
                            neli::consts::rtnl::RtScope::Host => "host",
                            neli::consts::rtnl::RtScope::Nowhere => "nowhere",
                            _ => "",
                        }
                        .to_string(),
                    );
                }
                if !entry.iface.is_empty() {
                    rec.insert("if".to_string(), entry.iface.to_owned());
                }
                rec.insert("mask".to_string(), entry.dst_len.to_string());

                rtable.push(rec);
            }

            if !rtable.is_empty() {
                data.insert("route-table".to_string(), json!(rtable));
            }
        }
    }

    data
}

/// Run sys.net
pub fn run(rt: &ModRequest) -> ModResponse {
    let mut res = runtime::new_call_response();

    match NetInfo::new() {
        Ok(netinfo) => {
            if let Err(err) = res.set_data(json!(get_data(rt, &netinfo))) {
                res.set_retcode(1);
                res.add_warning(&format!("{}", err));
            } else {
                res.set_message("Network data obtained");
            }
        }
        Err(err) => {
            res.set_message(&format!("Error obtaining networking data: {}", err));
        }
    }

    res
}
