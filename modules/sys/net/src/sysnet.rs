use crate::routing;
use libcommon::SysinspectError;
use libmodcore::{
    response::ModResponse,
    runtime::{self, ModRequest},
};
use serde_json::json;
use std::collections::{HashMap, hash_map::Entry};

#[cfg(target_os = "linux")]
use nix::{
    ifaddrs::{InterfaceAddress, getifaddrs},
    net::if_::InterfaceFlags,
    sys::socket::{AddressFamily, SockaddrLike},
};
#[cfg(target_os = "freebsd")]
use std::process::Command;

#[cfg(target_os = "linux")]
struct NetInfo {
    ifaces: Vec<InterfaceAddress>,
    if_filter: Vec<String>,
}

#[cfg(target_os = "freebsd")]
struct NetInfo {
    if_filter: Vec<String>,
}

impl NetInfo {
    fn itf_accepted(&self, name: &String) -> bool {
        self.if_filter.is_empty() || self.if_filter.contains(name)
    }

    fn set_if_filter(&mut self, if_filter: Vec<String>) {
        self.if_filter.extend(if_filter);
    }

    fn get_if_filter(&self) -> &Vec<String> {
        &self.if_filter
    }
}

#[cfg(target_os = "linux")]
impl NetInfo {
    fn new() -> Result<Self, SysinspectError> {
        getifaddrs().map_or_else(
            |_| Err(SysinspectError::ModuleError("Unable to retrieve interfaces data".to_string())),
            |interfaces| Ok(Self { ifaces: interfaces.collect(), if_filter: Vec::default() }),
        )
    }

    fn format_mac(mac: &[u8]) -> String {
        mac.iter().map(|byte| format!("{byte:02x}")).collect::<Vec<_>>().join(":")
    }

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
                    item.insert("mac".to_string(), json!(Self::format_mac(&mac).to_ascii_uppercase()));
                }
            }

            if self.itf_accepted(&iface.interface_name) && !item.is_empty() {
                if let Entry::Vacant(entry) = out.entry(iface.interface_name.to_string()) {
                    entry.insert(vec![item]);
                } else if let Some(items) = out.get_mut(&iface.interface_name) {
                    items.push(item);
                }
            }
        }

        out
    }
}

#[cfg(target_os = "freebsd")]
impl NetInfo {
    fn new() -> Result<Self, SysinspectError> {
        Ok(Self { if_filter: Vec::default() })
    }

    fn interfaces(&self) -> HashMap<String, Vec<HashMap<String, serde_json::Value>>> {
        Command::new("ifconfig")
            .arg("-a")
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
            .map(|stdout| {
                let mut out: HashMap<String, Vec<HashMap<String, serde_json::Value>>> = HashMap::default();
                let mut current = None::<String>;
                let mut up = false;

                for line in stdout.lines() {
                    if !line.starts_with('\t') && !line.starts_with(' ') {
                        line.split_once(':').map(|(name, rest)| {
                            current = Some(name.to_string());
                            up = rest.contains("UP");
                        });
                        continue;
                    }

                    current.as_ref().filter(|name| up && self.itf_accepted(name)).map(|name| {
                        let mut item: HashMap<String, serde_json::Value> = HashMap::default();
                        let trimmed = line.trim();
                        if trimmed.starts_with("ether ") {
                            item.insert("mac".to_string(), json!(trimmed.trim_start_matches("ether ").trim().to_ascii_uppercase()));
                        } else if trimmed.starts_with("inet6 ") {
                            let fields = trimmed.split_whitespace().collect::<Vec<_>>();
                            fields.get(1).map(|ip| item.insert("IPv6".to_string(), json!(*ip)));
                            fields
                                .iter()
                                .position(|field| *field == "scopeid")
                                .and_then(|idx| fields.get(idx + 1))
                                .map(|scope| item.insert("scope".to_string(), json!(*scope)));
                            item.insert("port".to_string(), json!(0));
                        } else if trimmed.starts_with("inet ") {
                            trimmed.split_whitespace().nth(1).map(|ip| item.insert("IPv4".to_string(), json!(ip)));
                            item.insert("port".to_string(), json!(0));
                        }

                        (!item.is_empty()).then(|| {
                            if let Entry::Vacant(entry) = out.entry(name.to_string()) {
                                entry.insert(vec![item]);
                            } else if let Some(items) = out.get_mut(name) {
                                items.push(item);
                            }
                        });
                    });
                }

                out
            })
            .unwrap_or_default()
    }
}

fn get_data(rt: &ModRequest, netinfo: &mut NetInfo) -> Result<HashMap<String, serde_json::Value>, SysinspectError> {
    let mut data: HashMap<String, serde_json::Value> = HashMap::default();

    netinfo.set_if_filter(runtime::get_arg(rt, "if-list").split(',').map(|part| part.trim().to_string()).filter(|part| !part.is_empty()).collect());

    if runtime::get_opt(rt, "if-up") {
        let interfaces = netinfo.interfaces();
        let interface_ids = interfaces.keys().cloned().collect::<Vec<_>>();
        let missing = netinfo.get_if_filter().iter().filter(|name| !interface_ids.contains(*name)).cloned().collect::<Vec<_>>();

        if !missing.is_empty() {
            return Err(SysinspectError::ModuleError(format!("missing network interfaces: {}", missing.join(", "))));
        }

        data.insert("if-up".to_string(), json!(interfaces));
    }

    if runtime::get_opt(rt, "route-table")
        && let Ok(route_data) = routing::ip_route()
    {
        let route_table = route_data
            .iter()
            .filter(|entry| entry.iface.is_empty() || netinfo.itf_accepted(&entry.iface))
            .map(|entry| {
                let mut row: HashMap<String, String> = HashMap::default();
                entry.gw.map(|gw| row.insert("gateway".to_string(), gw.to_string()));
                entry.src.map(|src| row.insert("src".to_string(), src.to_string()));
                entry.dst.map(|dst| row.insert("dst".to_string(), dst.to_string()));
                entry.proto.as_ref().map(|proto| row.insert("proto".to_string(), proto.to_string()));
                entry.scope.as_ref().map(|scope| row.insert("scope".to_string(), scope.to_string()));
                (!entry.iface.is_empty()).then(|| row.insert("if".to_string(), entry.iface.to_owned()));
                row.insert("mask".to_string(), entry.dst_len.to_string());
                row
            })
            .collect::<Vec<_>>();

        if !route_table.is_empty() {
            data.insert("route-table".to_string(), json!(route_table));
        }
    }

    Ok(data)
}

pub fn run(rt: &ModRequest) -> ModResponse {
    let mut response = runtime::new_call_response();

    match NetInfo::new() {
        Ok(mut netinfo) => match get_data(rt, &mut netinfo) {
            Ok(ret) => {
                if let Err(err) = response.set_data(json!(ret)) {
                    response.set_retcode(1);
                    response.add_warning(&format!("{err}"));
                } else {
                    response.set_message("Network data obtained");
                }
            }
            Err(err) => {
                response.set_retcode(1);
                response.set_message(&format!("{err}"));
            }
        },
        Err(err) => {
            response.set_message(&format!("Error obtaining networking data: {err}"));
        }
    }

    response
}
