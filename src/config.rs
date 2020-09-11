use crate::errors::Error;

use askama::Template;
use log::debug;
use std::io::Write;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub enum ReaderMessageType {
    Alert,
    Dns,
    Flow,
    Http,
    Smtp,
    Stats,
    Tls,
}

pub struct UdsListener {
    pub listener: std::os::unix::net::UnixListener,
    pub path: std::path::PathBuf,
}

pub enum Listener {
    External,
    Redis,
    Uds(UdsListener),
}

pub struct Reader {
    eve: EveConfiguration,
    pub message: ReaderMessageType,
    pub listener: Listener,
}

impl Reader {
    pub fn message(&self) -> &ReaderMessageType {
        &self.message
    }
}

pub struct InternalIps(Vec<String>);

impl InternalIps {
    pub fn new(ips: Vec<String>) -> Self {
        InternalIps(ips)
    }
}

impl std::fmt::Display for InternalIps {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        let ips = &self.0;
        write!(fmt, "{}", ips.join(","))?;
        Ok(())
    }
}

pub struct ConfigReader {
    eve: EveConfiguration,
    message: ReaderMessageType,
}

#[derive(Template)]
#[template(path = "suricata.yaml.in", escape = "none")]
struct ConfigTemplate<'a> {
    rules: &'a str,
    readers: Vec<ConfigReader>,
    community_id: &'a str,
    suricata_config_path: &'a str,
    internal_ips: &'a InternalIps,
    max_pending_packets: &'a str,
}

/// Configuration options for redis output
#[derive(Clone)]
pub struct Redis {
    pub server: String,
    pub port: u16,
}

impl Default for Redis {
    fn default() -> Self {
        Self {
            server: "redis".into(),
            port: 6379,
        }
    }
}

/// Configuration options for Alert socket
#[derive(Clone)]
pub struct Uds {
    pub path: PathBuf,
    pub external_listener: bool,
}

impl Default for Uds {
    fn default() -> Self {
        Self {
            path: PathBuf::from("/tmp/suricata"),
            external_listener: false,
        }
    }
}

/// Eve configuration
#[derive(Clone)]
pub enum EveConfiguration {
    Redis(Redis),
    Uds(Uds),
}

impl EveConfiguration {
    pub fn uds(path: PathBuf) -> Self {
        Self::Uds(Uds {
            path: path,
            external_listener: false,
        })
    }
}

impl Default for EveConfiguration {
    fn default() -> Self {
        Self::Uds(Uds::default())
    }
}

/// Configuration options for suricata
pub struct Config {
    /// Whether statistics should be enabled (output) for suricata, defaults to true
    pub enable_stats: bool,
    /// Whether flows should be enabled (output) for suricata, defaults to true
    pub enable_flows: bool,
    /// Whether http should be enabled (output) for suricata, defaults to false
    pub enable_http: bool,
    /// Whether dns should be enabled (output) for suricata, defaults to false
    pub enable_dns: bool,
    /// Whether smtp should be enabled (output) for suricata, defaults to false
    pub enable_smtp: bool,
    /// Whether tls should be enabled (output) for suricata, defaults to false
    pub enable_tls: bool,
    /// Whether community id should be enabled, defaults to true
    pub enable_community_id: bool,
    /// Path where config will be materialized to
    pub materialize_config_to: PathBuf,
    /// Path where the suricata executable lives
    pub exe_path: PathBuf,
    /// Configuration for eve
    pub eve: EveConfiguration,
    /// Path where the rules reside at
    pub rule_path: PathBuf,
    /// Path where suricata config resides at (e.g. threshold config)
    pub suriata_config_path: PathBuf,
    /// Internal ips to use for HOME_NET
    pub internal_ips: InternalIps,
    /// Max pending packets before suricata will block on incoming packets
    pub max_pending_packets: u16,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            enable_stats: true,
            enable_flows: true,
            enable_dns: false,
            enable_smtp: false,
            enable_http: false,
            enable_tls: false,
            enable_community_id: true,
            materialize_config_to: PathBuf::from("/etc/suricata/suricata-rs.yaml"),
            exe_path: {
                if let Some(e) = std::env::var_os("SURICATA_EXE").map(|s| PathBuf::from(s)) {
                    e
                } else {
                    PathBuf::from("/usr/local/bin/suricata")
                }
            },
            eve: EveConfiguration::default(),
            rule_path: PathBuf::from("/etc/suricata/custom.rules"),
            suriata_config_path: {
                if let Some(e) = std::env::var_os("SURICATA_CONFIG_DIR").map(|s| PathBuf::from(s)) {
                    e
                } else {
                    PathBuf::from("/etc/suricata")
                }
            },
            internal_ips: InternalIps(vec![
                String::from("10.0.0.0/8,172.16.0.0/12"),
                String::from("e80:0:0:0:0:0:0:0/64"),
                String::from("127.0.0.1/32"),
                String::from("fc00:0:0:0:0:0:0:0/7"),
                String::from("192.168.0.0/16"),
                String::from("169.254.0.0/16"),
            ]),
            max_pending_packets: 800,
        }
    }
}

fn uds_to_reader(uds: &Uds, mt: ReaderMessageType) -> Result<Reader, Error> {
    let path = uds.path.join(format!("{:?}.socket", mt));
    let listener = if !uds.external_listener {
        if path.exists() {
            std::fs::remove_file(&path).map_err(Error::from)?;
        }
        let listener = std::os::unix::net::UnixListener::bind(path.clone()).map_err(Error::from)?;
        Listener::Uds(UdsListener {
            listener: listener,
            path: path.clone(),
        })
    } else {
        Listener::External
    };
    let mut uds = uds.clone();
    uds.path = path;
    Ok(Reader {
        eve: EveConfiguration::Uds(uds),
        listener: listener,
        message: mt,
    })
}

impl Config {
    pub fn readers(&self) -> Result<Vec<Reader>, Error> {
        let mut message_types = vec![];
        if self.enable_dns {
            message_types.push(ReaderMessageType::Dns);
        }
        if self.enable_flows {
            message_types.push(ReaderMessageType::Flow);
        }
        if self.enable_http {
            message_types.push(ReaderMessageType::Http);
        }
        if self.enable_smtp {
            message_types.push(ReaderMessageType::Smtp);
        }
        if self.enable_stats {
            message_types.push(ReaderMessageType::Stats);
        }
        if self.enable_tls {
            message_types.push(ReaderMessageType::Tls);
        }

        let res: Result<Vec<_>, Error> = message_types
            .into_iter()
            .map(|mt| {
                if let EveConfiguration::Uds(uds) = &self.eve {
                    uds_to_reader(uds, mt)
                } else {
                    Ok(Reader {
                        eve: self.eve.clone(),
                        message: mt,
                        listener: Listener::Redis,
                    })
                }
            })
            .collect();

        res
    }

    pub fn materialize<'a, T>(&'a self, readers: T) -> Result<(), Error>
    where
        T: Iterator<Item = &'a Reader> + 'a,
    {
        let rules = self.rule_path.to_string_lossy().to_owned();
        let suricata_config_path = self.suriata_config_path.to_string_lossy().to_owned();
        let internal_ips = &self.internal_ips;
        let community_id = if self.enable_community_id {
            "yes"
        } else {
            "no"
        };
        let max_pending_packets = format!("{}", self.max_pending_packets);
        let readers = readers
            .map(|r| ConfigReader {
                eve: r.eve.clone(),
                message: r.message.clone(),
            })
            .collect();
        let template = ConfigTemplate {
            rules: &rules,
            readers: readers,
            community_id: &community_id,
            suricata_config_path: &suricata_config_path,
            internal_ips: internal_ips,
            max_pending_packets: &max_pending_packets,
        };
        debug!("Attempting to render");
        let rendered = template.render().map_err(Error::from)?;
        debug!("Writing output.yaml to {:?}", self.materialize_config_to);
        let mut f = std::fs::File::create(&self.materialize_config_to).map_err(Error::Io)?;
        f.write(rendered.as_bytes()).map_err(Error::from)?;
        debug!("Output file written");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::config::InternalIps;

    #[test]
    fn test_internal_ip_display() {
        let internal_ips = InternalIps(vec![
            "169.254.0.0/16".to_owned(),
            "192.168.0.0/16".to_owned(),
            "fc00:0:0:0:0:0:0:0/7".to_owned(),
            "127.0.0.1/32".to_owned(),
            "10.0.0.0/8".to_owned(),
            "172.16.0.0/12".to_owned(),
        ]);
        assert_eq!(format!("{}", internal_ips), "169.254.0.0/16,192.168.0.0/16,fc00:0:0:0:0:0:0:0/7,127.0.0.1/32,10.0.0.0/8,172.16.0.0/12");
    }
}
