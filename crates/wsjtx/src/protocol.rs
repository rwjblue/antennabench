use thiserror::Error;

pub const WSJTX_MAGIC: u32 = 0xadbccbda;
pub const MIN_SUPPORTED_SCHEMA: u32 = 2;
pub const MAX_SUPPORTED_SCHEMA: u32 = 3;

const HEARTBEAT_MESSAGE_TYPE: u32 = 0;
const STATUS_MESSAGE_TYPE: u32 = 1;
const CLOSE_MESSAGE_TYPE: u32 = 6;
const WSPR_DECODE_MESSAGE_TYPE: u32 = 10;
const NULL_BYTE_ARRAY_LENGTH: u32 = u32::MAX;
const MILLIS_PER_DAY: u32 = 86_400_000;

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedDatagram {
    pub schema: u32,
    pub message: WsjtxMessage,
    pub trailing_bytes: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WsjtxMessage {
    Heartbeat(HeartbeatMessage),
    Status(StatusMessage),
    WsprDecode(WsprDecodeMessage),
    Close(CloseMessage),
    Unsupported(UnsupportedMessage),
}

impl WsjtxMessage {
    pub fn client_id(&self) -> &str {
        match self {
            Self::Heartbeat(message) => &message.client_id,
            Self::Status(message) => &message.client_id,
            Self::WsprDecode(message) => &message.client_id,
            Self::Close(message) => &message.client_id,
            Self::Unsupported(message) => &message.client_id,
        }
    }

    pub fn message_type(&self) -> u32 {
        match self {
            Self::Heartbeat(_) => HEARTBEAT_MESSAGE_TYPE,
            Self::Status(_) => STATUS_MESSAGE_TYPE,
            Self::WsprDecode(_) => WSPR_DECODE_MESSAGE_TYPE,
            Self::Close(_) => CLOSE_MESSAGE_TYPE,
            Self::Unsupported(message) => message.message_type,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatMessage {
    pub client_id: String,
    pub maximum_schema: u32,
    pub version: String,
    pub revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusMessage {
    pub client_id: String,
    pub dial_frequency_hz: u64,
    pub mode: String,
    pub dx_call: String,
    pub report: String,
    pub tx_mode: String,
    pub tx_enabled: bool,
    pub transmitting: bool,
    pub decoding: bool,
    pub rx_df_hz: u32,
    pub tx_df_hz: u32,
    pub de_call: String,
    pub de_grid: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WsprDecodeMessage {
    pub client_id: String,
    pub is_new: bool,
    pub time_millis: u32,
    pub snr_db: i32,
    pub delta_time_seconds: f64,
    pub frequency_hz: u64,
    pub drift_hz_per_minute: i32,
    pub callsign: String,
    pub grid: String,
    pub power_dbm: i32,
    pub off_air: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloseMessage {
    pub client_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedMessage {
    pub client_id: String,
    pub message_type: u32,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DatagramParseError {
    #[error("invalid WSJT-X magic 0x{actual:08x}")]
    InvalidMagic { actual: u32 },
    #[error("unsupported WSJT-X schema {actual}; supported schemas are 2 and 3")]
    UnsupportedSchema { actual: u32 },
    #[error(
        "datagram ended while reading {field}: needed {needed} bytes, only {remaining} remain"
    )]
    Truncated {
        field: &'static str,
        needed: usize,
        remaining: usize,
    },
    #[error("{field} contains invalid UTF-8")]
    InvalidUtf8 { field: &'static str },
    #[error("{field} has invalid boolean value {actual}")]
    InvalidBool { field: &'static str, actual: u8 },
    #[error("WSPR decode time {actual} is not a valid QTime millisecond value")]
    InvalidTime { actual: u32 },
    #[error("{field} is not a finite floating-point value")]
    NonFiniteFloat { field: &'static str },
}

pub fn parse_wsjtx_datagram(datagram: &[u8]) -> Result<ParsedDatagram, DatagramParseError> {
    let mut cursor = Cursor::new(datagram);
    let magic = cursor.read_u32("magic")?;
    if magic != WSJTX_MAGIC {
        return Err(DatagramParseError::InvalidMagic { actual: magic });
    }

    let schema = cursor.read_u32("schema")?;
    if !(MIN_SUPPORTED_SCHEMA..=MAX_SUPPORTED_SCHEMA).contains(&schema) {
        return Err(DatagramParseError::UnsupportedSchema { actual: schema });
    }

    let message_type = cursor.read_u32("message type")?;
    let client_id = cursor.read_utf8("client id")?;

    let message = match message_type {
        HEARTBEAT_MESSAGE_TYPE => {
            let maximum_schema = if schema >= 3 {
                cursor.read_u32("heartbeat maximum schema")?
            } else {
                2
            };
            WsjtxMessage::Heartbeat(HeartbeatMessage {
                client_id,
                maximum_schema,
                version: cursor.read_utf8("heartbeat version")?,
                revision: cursor.read_utf8("heartbeat revision")?,
            })
        }
        STATUS_MESSAGE_TYPE => WsjtxMessage::Status(StatusMessage {
            client_id,
            dial_frequency_hz: cursor.read_u64("status dial frequency")?,
            mode: cursor.read_utf8("status mode")?,
            dx_call: cursor.read_utf8("status DX call")?,
            report: cursor.read_utf8("status report")?,
            tx_mode: cursor.read_utf8("status TX mode")?,
            tx_enabled: cursor.read_bool("status TX enabled")?,
            transmitting: cursor.read_bool("status transmitting")?,
            decoding: cursor.read_bool("status decoding")?,
            rx_df_hz: cursor.read_u32("status RX DF")?,
            tx_df_hz: cursor.read_u32("status TX DF")?,
            de_call: cursor.read_utf8("status DE call")?,
            de_grid: cursor.read_utf8("status DE grid")?,
        }),
        CLOSE_MESSAGE_TYPE => WsjtxMessage::Close(CloseMessage { client_id }),
        WSPR_DECODE_MESSAGE_TYPE => {
            let is_new = cursor.read_bool("WSPR decode new")?;
            let time_millis = cursor.read_u32("WSPR decode time")?;
            if time_millis >= MILLIS_PER_DAY {
                return Err(DatagramParseError::InvalidTime {
                    actual: time_millis,
                });
            }
            let snr_db = cursor.read_i32("WSPR decode SNR")?;
            let delta_time_seconds = cursor.read_f64("WSPR decode delta time")?;
            if !delta_time_seconds.is_finite() {
                return Err(DatagramParseError::NonFiniteFloat {
                    field: "WSPR decode delta time",
                });
            }
            WsjtxMessage::WsprDecode(WsprDecodeMessage {
                client_id,
                is_new,
                time_millis,
                snr_db,
                delta_time_seconds,
                frequency_hz: cursor.read_u64("WSPR decode frequency")?,
                drift_hz_per_minute: cursor.read_i32("WSPR decode drift")?,
                callsign: cursor.read_utf8("WSPR decode callsign")?,
                grid: cursor.read_utf8("WSPR decode grid")?,
                power_dbm: cursor.read_i32("WSPR decode power")?,
                off_air: cursor.read_bool("WSPR decode off air")?,
            })
        }
        _ => WsjtxMessage::Unsupported(UnsupportedMessage {
            client_id,
            message_type,
        }),
    };

    Ok(ParsedDatagram {
        schema,
        message,
        trailing_bytes: cursor.remaining(),
    })
}

struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.position)
    }

    fn read_exact(
        &mut self,
        length: usize,
        field: &'static str,
    ) -> Result<&'a [u8], DatagramParseError> {
        if self.remaining() < length {
            return Err(DatagramParseError::Truncated {
                field,
                needed: length,
                remaining: self.remaining(),
            });
        }

        let start = self.position;
        self.position += length;
        Ok(&self.bytes[start..self.position])
    }

    fn read_u8(&mut self, field: &'static str) -> Result<u8, DatagramParseError> {
        Ok(self.read_exact(1, field)?[0])
    }

    fn read_bool(&mut self, field: &'static str) -> Result<bool, DatagramParseError> {
        match self.read_u8(field)? {
            0 => Ok(false),
            1 => Ok(true),
            actual => Err(DatagramParseError::InvalidBool { field, actual }),
        }
    }

    fn read_u32(&mut self, field: &'static str) -> Result<u32, DatagramParseError> {
        let bytes = self.read_exact(4, field)?;
        Ok(u32::from_be_bytes(
            bytes.try_into().expect("four bytes read"),
        ))
    }

    fn read_i32(&mut self, field: &'static str) -> Result<i32, DatagramParseError> {
        let bytes = self.read_exact(4, field)?;
        Ok(i32::from_be_bytes(
            bytes.try_into().expect("four bytes read"),
        ))
    }

    fn read_u64(&mut self, field: &'static str) -> Result<u64, DatagramParseError> {
        let bytes = self.read_exact(8, field)?;
        Ok(u64::from_be_bytes(
            bytes.try_into().expect("eight bytes read"),
        ))
    }

    fn read_f64(&mut self, field: &'static str) -> Result<f64, DatagramParseError> {
        Ok(f64::from_bits(self.read_u64(field)?))
    }

    fn read_utf8(&mut self, field: &'static str) -> Result<String, DatagramParseError> {
        let length = self.read_u32(field)?;
        if length == NULL_BYTE_ARRAY_LENGTH {
            return Ok(String::new());
        }
        let bytes = self.read_exact(length as usize, field)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| DatagramParseError::InvalidUtf8 { field })
    }
}
