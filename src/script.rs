use anyhow::{anyhow, Result};
use chrono::{DateTime, Duration, FixedOffset, Local};
use csv::{ReaderBuilder, Trim};
use glob::glob;
use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer,
};
use std::{
    cmp::Ordering,
    fmt,
    fs::File,
    io::{BufReader, Read},
    path::{Path, PathBuf},
    str::FromStr,
    time::SystemTime,
};

fn now() -> DateTime<FixedOffset> {
    DateTime::from(DateTime::<Local>::from(SystemTime::now()))
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Timestamp {
    Absolute(DateTime<FixedOffset>),
    Relative(Duration),
}

impl FromStr for Timestamp {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match DateTime::parse_from_rfc2822(s) {
            Ok(t) => {
                return Ok(Timestamp::Absolute(t));
            }
            Err(e) => {
                log::debug! {"Failed to parse \"{}\" as RFC2822 timestamp: {}", s, e};
            }
        }

        match DateTime::parse_from_rfc3339(s) {
            Ok(t) => {
                return Ok(Timestamp::Absolute(t));
            }
            Err(e) => {
                log::debug! {"Failed to parse \"{}\" as RFC3339 timestamp: {}", s, e};
            }
        }

        match s.parse() {
            Ok(t) => {
                return Ok(Timestamp::Relative(Duration::seconds(t)));
            }
            Err(e) => {
                log::debug! {"Failed to parse \"{}\" as offset in seconds: {}", s, e};
            }
        }

        Err(anyhow! {"Could not determine a timestamp from \"{}\"", s})
    }
}

struct TimestampVisitor;

impl<'de> Visitor<'de> for TimestampVisitor {
    type Value = Timestamp;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(
            "an absolute timestamp in RFC2822 or RFC3339 format or a relative time in seconds",
        )
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        match Timestamp::from_str(value) {
            Ok(t) => Ok(t),
            Err(e) => Err(de::Error::custom(e)),
        }
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D>(deserializer: D) -> Result<Timestamp, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(TimestampVisitor)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(crate) struct Message {
    pub timestamp: Timestamp,
    pub topic: String,
    pub message: String,
}

fn load_messages<R: Read>(delimiter: u8, reader: R) -> Vec<Message> {
    let mut offset_time = now();

    ReaderBuilder::new()
        .has_headers(false)
        .trim(Trim::All)
        .delimiter(delimiter)
        .from_reader(reader)
        .deserialize()
        .filter_map(|m| match m {
            Ok(m) => Some(m),
            Err(e) => {
                log::warn! {"Failed to parse message script: {}", e};
                None
            }
        })
        .map(|mut m: Message| match m.timestamp {
            Timestamp::Absolute(msg_time) => {
                offset_time = msg_time;
                m
            }
            Timestamp::Relative(msg_time) => {
                let msg_time = offset_time + msg_time;
                m.timestamp = Timestamp::Absolute(msg_time);
                offset_time = msg_time;
                m
            }
        })
        .collect()
}

#[derive(Clone, Debug)]
pub(crate) struct Script {
    source_dir: PathBuf,
    source_file_delimiter: u8,

    messages: Vec<Message>,

    last_poll_time: DateTime<FixedOffset>,
}

impl Script {
    pub(crate) fn new(dir: &Path, delimiter: u8) -> Result<Self> {
        let mut s = Script {
            source_dir: dir.to_path_buf(),
            source_file_delimiter: delimiter,
            messages: Vec::new(),
            last_poll_time: now(),
        };

        if let Err(e) = s.reload() {
            log::error! {"Initial script loading failed: {}", e};
        }

        Ok(s)
    }

    pub(crate) fn reload(&mut self) -> Result<()> {
        log::debug! {"Building script from \"{}\"", &self.source_dir.display()};

        self.messages = glob(&format!("{}/**/*.txt", self.source_dir.display()))?
            .filter_map(|path| match path {
                Ok(path) => Some(load_messages(
                    self.source_file_delimiter,
                    BufReader::new(File::open(path).ok()?),
                )),
                Err(_) => None,
            })
            .flatten()
            .collect();

        // This sort is not strictly necessary, the core functionality will work mostly the same
        // without it. It is just here to provide logical ordering for logging and ensures messages
        // falling within the same poll() time window are delivered in timestamp order.
        self.messages.sort_by(|a, b| {
            if let Timestamp::Absolute(a) = a.timestamp {
                if let Timestamp::Absolute(b) = b.timestamp {
                    a.cmp(&b)
                } else {
                    Ordering::Equal
                }
            } else {
                Ordering::Equal
            }
        });

        log::info! {"Loaded messages:"};
        for m in &self.messages {
            log::info! {"{:?}", m};
        }

        Ok(())
    }

    pub(crate) fn poll(&mut self) -> Vec<Message> {
        let start = self.last_poll_time;
        let end = now();

        let msgs = self
            .messages
            .iter()
            .filter_map(|m| {
                if let Timestamp::Absolute(t) = m.timestamp {
                    if t > start && t <= end {
                        Some(m.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        self.last_poll_time = end;

        msgs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::io::Cursor;

    #[test]
    fn timestamp_parse_absolute() {
        assert_eq!(
            Timestamp::from_str("2022-03-28T10:23:33+00:00").unwrap(),
            Timestamp::Absolute(FixedOffset::east(0).ymd(2022, 3, 28).and_hms(10, 23, 33))
        );

        assert_eq!(
            Timestamp::from_str("Mon, 28 Mar 2022 10:23:33 GMT").unwrap(),
            Timestamp::Absolute(FixedOffset::east(0).ymd(2022, 3, 28).and_hms(10, 23, 33))
        );
    }

    #[test]
    fn timestamp_parse_relative() {
        assert_eq!(
            Timestamp::from_str("25").unwrap(),
            Timestamp::Relative(Duration::seconds(25))
        );
    }

    #[test]
    fn messages_with_absolute_and_relative() {
        let data = r##"
Mon, 28 Mar 2022 00:00:00 GMT | root/user-1 | msg 1
10                            | root/user-2 | msg 2
20                            | root/user-1 | msg 3
"##;
        let c = Cursor::new(data);
        let msgs = load_messages(b'|', c);
        assert_eq!(msgs.len(), 3);
    }

    #[test]
    fn messages_with_only_relative() {
        let data = r##"
 5 | root/user-1 | msg 1
10 | root/user-2 | msg 2
20 | root/user-1 | msg 3
"##;
        let c = Cursor::new(data);
        let msgs = load_messages(b'|', c);
        assert_eq!(msgs.len(), 3);
    }

    #[test]
    fn script_poll() {
        let data = r##"
1 | test_topic | msg 1
1 | test_topic | msg 2
1 | test_topic | msg 3
1 | test_topic | msg 4
1 | test_topic | msg 5
"##;

        let mut s = Script {
            source_dir: PathBuf::new(),
            source_file_delimiter: b'|',
            messages: load_messages(b'|', Cursor::new(data)),
            last_poll_time: now(),
        };

        // t =   10
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert_eq!(s.poll(), vec![]);

        // t = 1010
        std::thread::sleep(std::time::Duration::from_secs(1));
        assert_eq!(s.poll(), vec![s.messages[0].clone()]);

        // t = 3010
        std::thread::sleep(std::time::Duration::from_secs(2));
        assert_eq!(s.poll(), vec![s.messages[1].clone(), s.messages[2].clone()]);

        // t = 3510
        std::thread::sleep(std::time::Duration::from_millis(500));
        assert_eq!(s.poll(), vec![]);

        // t = 4010
        std::thread::sleep(std::time::Duration::from_millis(500));
        assert_eq!(s.poll(), vec![s.messages[3].clone()]);

        // t = 6010
        std::thread::sleep(std::time::Duration::from_secs(2));
        assert_eq!(s.poll(), vec![s.messages[4].clone()]);
    }
}
