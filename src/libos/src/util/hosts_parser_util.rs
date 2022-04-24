use super::*;
use regex::Regex;
use std::net::IpAddr;
use std::path::Path;
use std::str;
use std::str::FromStr;

lazy_static! {
    // The hostname regex is compliant with RFC1123
    static ref HOSTNAME_RE: Regex = Regex::new(r"^(([a-zA-Z0-9]|[a-zA-Z0-9][a-zA-Z0-9\-]*[a-zA-Z0-9])\.)*([A-Za-z0-9]|[A-Za-z0-9][A-Za-z0-9\-]*[A-Za-z0-9])$").unwrap();
}

#[derive(Debug, Default, Clone)]
pub struct HostEntry {
    ip: String,
    hostname: Vec<String>,
}

#[derive(Debug, Default, Clone)]
pub struct Hosts {
    pub entries: Vec<HostEntry>,
}

impl FromStr for HostEntry {
    type Err = error::Error;
    fn from_str(line: &str) -> Result<Self> {
        let slice: Vec<&str> = line.split_whitespace().collect();

        // check IP:
        let ip = match slice.first() {
            Some(ip) => ip,
            None => {
                return_errno!(EINVAL, "malformated ip in /etc/hosts file");
            }
        };

        let _ip_addr: IpAddr = match ip.parse() {
            Ok(ip) => ip,
            Err(_) => {
                return_errno!(EINVAL, "malformated ip in /etc/hosts file");
            }
        };

        let mut hostname: Vec<String> = Vec::new();
        if !slice.iter().skip(1).all(|&s| {
            let is_match = HOSTNAME_RE.is_match(s);
            if is_match {
                hostname.push(s.to_string());
            }
            is_match
        }) {
            return_errno!(EINVAL, "malformated hostname in /etc/hosts file");
        }

        if hostname.is_empty() {
            return_errno!(EINVAL, "malformated hostname in /etc/hosts file");
        }
        Ok(HostEntry {
            ip: ip.to_string(),
            hostname,
        })
    }
}

pub fn parse_hosts_buffer(bytes: &[u8]) -> Result<Hosts> {
    let mut hosts: Hosts = Default::default();
    for (_, line) in bytes.split(|&x| x == b'\n').enumerate() {
        let line = str::from_utf8(line).unwrap();
        let line = line.trim_start();
        match line.chars().next() {
            // comment
            Some('#') => continue,
            // empty line
            None => continue,
            // valid line
            Some(_) => {}
        }
        hosts.entries.push(line.parse()?);
    }
    Ok(hosts)
}

pub fn parse_hostname_buffer(bytes: &[u8]) -> Result<String> {
    let mut hostname: Vec<String> = Vec::new();
    for (_, line) in bytes.split(|&x| x == b'\n').enumerate() {
        let line = str::from_utf8(line).unwrap();
        let line = line.trim_start();
        match line.chars().next() {
            // comment
            Some('#') => continue,
            // empty line
            None => continue,
            // valid line
            Some(_) => {}
        }
        if (!HOSTNAME_RE.is_match(&line)) | (line.len() > 64) {
            return_errno!(EINVAL, "malformated hostname in /etc/hostname file");
        }
        hostname.push(line.to_owned());
    }

    if hostname.len() != 1 {
        return_errno!(EINVAL, "malformated hostname in /etc/hostname file");
    }
    Ok(hostname[0].clone())
}
