use crate::routing;
use libcommon::SysinspectError;
use libmodcore::{
    response::ModResponse,
    runtime::{self, ModRequest},
};
use nix::{
    ifaddrs::{InterfaceAddress, getifaddrs},
    net::if_::InterfaceFlags,
    sys::socket::{AddressFamily, SockaddrLike},
};
use serde_json::json;
use std::collections::{HashMap, hash_map::Entry};

struct NetInfo {
    ifaces: Vec<InterfaceAddress>,
    if_filter: Vec<String>,
}

impl NetInfo {
    /// NetInfo instance
    fn new() -> Result<Self, SysinspectError> {
        if let Ok(itr) = getifaddrs() {
            let mut ifaces: Vec<InterfaceAddress> = Vec::default();
            for iface in itr {
                ifaces.push(iface);
            }
            Ok(NetInfo { ifaces, if_filter: Vec::default() })
        } else {
            Err(SysinspectError::ModuleError("Unable to retrieve interfaces data".to_string()))
        }
    }

    fn format_mac(mac: &[u8]) -> String {
        mac.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(":")
    }

    /// Checks if interface is accepted
    fn itf_accepted(&self, name: &String) -> bool {
        self.if_filter.is_empty() || self.if_filter.contains(name)
    }

    /// Get interfaces
    fn interfaces(&self) -> HashMap<String, Vec<HashMap<String, serde_json::Value>>> {
        let mut out: HashMap<String, Vec<HashMap<String, serde_json::Value>>> = HashMap::default();
        for iface in &self.ifaces {
            if !iface.flags.contains(InterfaceFlags::IFF_UP) {
                continue;
            }

            let mut item: HashMap<String, serde_json::Value> = HashMap::default();

            if let Some(addr) = iface.address
                && let Some(family) = addr.family()
            {
                if let (AddressFamily::Inet, Some(ip)) = (family, addr.as_sockaddr_in()) {
                    item.insert("port".to_string(), json!(ip.port()));
                    item.insert("IPv4".to_string(), json!(ip.ip()));
                } else if let (AddressFamily::Inet6, Some(ip)) = (family, addr.as_sockaddr_in6()) {
                    item.insert("port".to_string(), json!(ip.port()));
                    item.insert("IPv6".to_string(), json!(ip.ip()));
                    item.insert("scope".to_string(), json!(ip.scope_id()));
                } else if let (AddressFamily::Packet, Some(link)) = (family, addr.as_link_addr())
                    && let Some(mac) = link.addr()
                {
                    item.insert("mac".to_string(), json!(Self::format_mac(&mac).to_ascii_uppercase().to_string()));
                }
            }

            let ifn = iface.interface_name.to_string();
            if self.itf_accepted(&ifn) && !item.is_empty() {
                if let Entry::Vacant(e) = out.entry(ifn.to_owned()) {
                    e.insert(vec![item]);
                } else {
                    out.get_mut(&ifn).unwrap().push(item);
                }
            }
        }

        out
    }

    /// Set interfaces those needs to be examined, ignoring all others.
    fn set_if_filter(&mut self, if_filter: Vec<String>) {
        self.if_filter.extend(if_filter);
    }

    /// Get filter interfaces
    fn get_if_filter(&self) -> &Vec<String> {
        &self.if_filter
    }
}

/// Get data
fn get_data(rt: &ModRequest, netinfo: &mut NetInfo) -> Result<HashMap<String, serde_json::Value>, SysinspectError> {
    let mut data: HashMap<String, serde_json::Value> = HashMap::default();

    netinfo
        .set_if_filter(runtime::get_arg(rt, "if-list").split(",").map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect::<Vec<String>>());

    // Include running interfaces
    if runtime::get_opt(rt, "if-up") {
        let ifs = netinfo.interfaces();
        let ifs_ids = ifs.keys().map(|s| s.to_string()).collect::<Vec<String>>();

        // Reconcile found/requested interfaces
        let mut missing: Vec<String> = vec![];
        for rq_if in netinfo.get_if_filter() {
            if !ifs_ids.contains(rq_if) {
                missing.push(rq_if.to_owned());
            }
        }
        if !missing.is_empty() {
            return Err(SysinspectError::ModuleError(format!("missing network interfaces: {}", missing.join(", "))));
        }

        data.insert("if-up".to_string(), json!(ifs));
    }

    if runtime::get_opt(rt, "route-table")
        && let Ok(rt_data) = routing::ip_route()
    {
        let mut rtable: Vec<HashMap<String, String>> = Vec::default();
        for entry in &rt_data {
            // Skip an interface, which wasn't requested
            if !entry.iface.is_empty() && !netinfo.itf_accepted(&entry.iface) {
                continue;
            }
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

    Ok(data)
}

/// Run sys.net
pub fn run(rt: &ModRequest) -> ModResponse {
    let mut res = runtime::new_call_response();

    match NetInfo::new() {
        Ok(mut netinfo) => match get_data(rt, &mut netinfo) {
            Ok(ret) => {
                if let Err(err) = res.set_data(json!(ret)) {
                    res.set_retcode(1);
                    res.add_warning(&format!("{err}"));
                } else {
                    res.set_message("Network data obtained");
                }
            }
            Err(err) => {
                res.set_retcode(1);
                res.set_message(&format!("{err}"));
            }
        },
        Err(err) => {
            res.set_message(&format!("Error obtaining networking data: {err}"));
        }
    }

    res
}
