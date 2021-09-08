//
// Copyright (c) 2017, 2020 ADLINK Technology Inc.
//
// This program and the accompanying materials are made available under the
// terms of the Eclipse Public License 2.0 which is available at
// http://www.eclipse.org/legal/epl-2.0, or the Apache License, Version 2.0
// which is available at https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: EPL-2.0 OR Apache-2.0
//
// Contributors:
//   ADLINK zenoh team, <zenoh@adlink-labs.tech>
//
pub mod rname;

use http_types::Mime;
use std::borrow::Cow;
use std::convert::From;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::sync::atomic::AtomicU64;
pub use uhlc::Timestamp;
use zenoh_util::core::{ZError, ZErrorKind, ZResult};
use zenoh_util::zerror;

/// The unique Id of the [`HLC`](uhlc::HLC) that generated the concerned [`Timestamp`].
pub type TimestampId = uhlc::ID;

/// A zenoh integer.
pub type ZInt = u64;
pub type ZiInt = i64;
pub type AtomicZInt = AtomicU64;
pub const ZINT_MAX_BYTES: usize = 10;

zconfigurable! {
    static ref CONGESTION_CONTROL_DEFAULT: CongestionControl = CongestionControl::Drop;
}

// WhatAmI values
pub type WhatAmI = whatami::Type;
/// Constants and helpers for zenoh `whatami`falgs.
pub mod whatami {
    use super::ZInt;

    pub type Type = ZInt;

    pub const ROUTER: Type = 1; // 0x01
    pub const PEER: Type = 1 << 1; // 0x02
    pub const CLIENT: Type = 1 << 2; // 0x04
                                     // b4-b13: Reserved

    pub fn to_string(w: Type) -> String {
        match w {
            ROUTER => "Router".to_string(),
            PEER => "Peer".to_string(),
            CLIENT => "Client".to_string(),
            i => i.to_string(),
        }
    }
}

/// A numerical Id mapped to a resource name with [`register_resource`](crate::Session::register_resource).
pub type ResourceId = ZInt;

pub const NO_RESOURCE_ID: ResourceId = 0;

/// A resource key.
//  7 6 5 4 3 2 1 0
// +-+-+-+-+-+-+-+-+
// ~      id       — if ResName{name} : id=0
// +-+-+-+-+-+-+-+-+
// ~  name/suffix  ~ if flag C!=1 in Message's header
// +---------------+
//
#[derive(PartialEq, Eq, Hash, Clone)]
pub enum ResKey<'a> {
    RName(Cow<'a, str>),
    RId(ResourceId),
    RIdWithSuffix(ResourceId, Cow<'a, str>),
}
use ResKey::*;

impl ResKey<'_> {
    #[inline(always)]
    pub fn rid(&self) -> ResourceId {
        match self {
            RName(_) => NO_RESOURCE_ID,
            RId(rid) | RIdWithSuffix(rid, _) => *rid,
        }
    }

    #[inline(always)]
    pub fn is_numerical(&self) -> bool {
        matches!(self, RId(_))
    }

    pub fn to_owned(&self) -> ResKey<'static> {
        match self {
            Self::RId(id) => ResKey::RId(*id),
            Self::RName(s) => ResKey::RName(s.to_string().into()),
            Self::RIdWithSuffix(id, s) => ResKey::RIdWithSuffix(*id, s.to_string().into()),
        }
    }
}

impl fmt::Debug for ResKey<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RName(name) => write!(f, "{}", name),
            RId(rid) => write!(f, "{}", rid),
            RIdWithSuffix(rid, suffix) => write!(f, "{}, {}", rid, suffix),
        }
    }
}

impl fmt::Display for ResKey<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl<'a> From<&ResKey<'a>> for ResKey<'a> {
    #[inline]
    fn from(key: &ResKey<'a>) -> ResKey<'a> {
        key.clone()
    }
}

impl From<ResourceId> for ResKey<'_> {
    #[inline]
    fn from(rid: ResourceId) -> ResKey<'static> {
        RId(rid)
    }
}

impl<'a> From<&'a str> for ResKey<'a> {
    #[inline]
    fn from(name: &'a str) -> ResKey<'a> {
        RName(name.into())
    }
}

impl From<String> for ResKey<'_> {
    #[inline]
    fn from(name: String) -> ResKey<'static> {
        RName(name.into())
    }
}

impl<'a> From<&'a String> for ResKey<'a> {
    #[inline]
    fn from(name: &'a String) -> ResKey<'a> {
        RName(name.as_str().into())
    }
}

impl<'a> From<(ResourceId, &'a str)> for ResKey<'a> {
    #[inline]
    fn from(tuple: (ResourceId, &'a str)) -> ResKey<'a> {
        if tuple.1.is_empty() {
            RId(tuple.0)
        } else if tuple.0 == NO_RESOURCE_ID {
            RName(tuple.1.into())
        } else {
            RIdWithSuffix(tuple.0, tuple.1.into())
        }
    }
}

impl From<(ResourceId, String)> for ResKey<'_> {
    #[inline]
    fn from(tuple: (ResourceId, String)) -> ResKey<'static> {
        if tuple.1.is_empty() {
            RId(tuple.0)
        } else if tuple.0 == NO_RESOURCE_ID {
            RName(tuple.1.into())
        } else {
            RIdWithSuffix(tuple.0, tuple.1.into())
        }
    }
}

impl<'a> From<&'a ResKey<'a>> for (ResourceId, &'a str) {
    #[inline]
    fn from(key: &'a ResKey<'a>) -> (ResourceId, &'a str) {
        match key {
            RId(rid) => (*rid, ""),
            RName(name) => (NO_RESOURCE_ID, &name[..]), //(&(0 as u64)
            RIdWithSuffix(rid, suffix) => (*rid, &suffix[..]),
        }
    }
}

/// A zenoh [`Value`](crate::Value) encoding.
///
/// A zenoh encoding is a [`Mime`](http_types::Mime) type represented, for wire efficeincy,
/// as an integer prefix (that maps to a string) and a string suffix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Encoding {
    pub prefix: ZInt,
    pub suffix: Cow<'static, str>,
}

impl Encoding {
    /// Converts the given encoding to [`Mime`](http_types::Mime).
    pub fn to_mime(&self) -> ZResult<Mime> {
        if self.prefix == 0 {
            Mime::from_str(self.suffix.as_ref()).map_err(|e| {
                ZError::new(
                    ZErrorKind::Other {
                        descr: e.to_string(),
                    },
                    file!(),
                    line!(),
                    None,
                )
            })
        } else if self.prefix <= encoding::MIMES.len() as ZInt {
            Mime::from_str(&format!(
                "{}{}",
                &encoding::MIMES[self.prefix as usize],
                self.suffix
            ))
            .map_err(|e| {
                ZError::new(
                    ZErrorKind::Other {
                        descr: e.to_string(),
                    },
                    file!(),
                    line!(),
                    None,
                )
            })
        } else {
            zerror!(ZErrorKind::Other {
                descr: format!("Unknown encoding prefix {}", self.prefix)
            })
        }
    }

    /// Sets the suffix of this encoding.
    pub fn with_suffix<IntoCowStr>(mut self, suffix: IntoCowStr) -> Self
    where
        IntoCowStr: Into<Cow<'static, str>>,
    {
        self.suffix = suffix.into();
        self
    }

    /// Returns `true`if the string representation of this encoding starts with
    /// the string representation of ther given encoding.
    pub fn starts_with(&self, encoding: &Encoding) -> bool {
        (self.prefix == encoding.prefix && self.suffix.starts_with(encoding.suffix.as_ref()))
            || self.to_string().starts_with(&encoding.to_string())
    }
}

impl fmt::Display for Encoding {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.prefix > 0 && self.prefix < encoding::MIMES.len() as ZInt {
            write!(
                f,
                "{}{}",
                &encoding::MIMES[self.prefix as usize],
                self.suffix
            )
        } else {
            write!(f, "{}", self.suffix)
        }
    }
}

impl From<&'static str> for Encoding {
    fn from(s: &'static str) -> Self {
        for (i, v) in encoding::MIMES.iter().enumerate() {
            if i != 0 && s.starts_with(v) {
                return Encoding {
                    prefix: i as u64,
                    suffix: s.split_at(v.len()).1.into(),
                };
            }
        }
        Encoding {
            prefix: 0,
            suffix: s.into(),
        }
    }
}

impl<'a> From<String> for Encoding {
    fn from(s: String) -> Self {
        for (i, v) in encoding::MIMES.iter().enumerate() {
            if i != 0 && s.starts_with(v) {
                return Encoding {
                    prefix: i as u64,
                    suffix: s.split_at(v.len()).1.to_string().into(),
                };
            }
        }
        Encoding {
            prefix: 0,
            suffix: s.into(),
        }
    }
}

impl<'a> From<Mime> for Encoding {
    fn from(m: Mime) -> Self {
        Encoding::from(&m)
    }
}

impl<'a> From<&Mime> for Encoding {
    fn from(m: &Mime) -> Self {
        Encoding::from(m.essence().to_string())
    }
}

impl From<u64> for Encoding {
    fn from(i: u64) -> Self {
        Encoding {
            prefix: i,
            suffix: "".into(),
        }
    }
}

impl Default for Encoding {
    fn default() -> Self {
        encoding::EMPTY
    }
}

/// Constants and helpers for zenoh [`Encoding`].
pub mod encoding {
    use super::Encoding;

    lazy_static! {
        pub(super) static ref MIMES: [&'static str; 21] = [
            /*  0 */ "",
            /*  1 */ "application/octet-stream",
            /*  2 */ "application/custom", // non iana standard
            /*  3 */ "text/plain",
            /*  4 */ "application/properties", // non iana standard
            /*  5 */ "application/json", // if not readable from casual users
            /*  6 */ "application/sql",
            /*  7 */ "application/integer", // non iana standard
            /*  8 */ "application/float", // non iana standard
            /*  9 */ "application/xml", // if not readable from casual users (RFC 3023, section 3)
            /* 10 */ "application/xhtml+xml",
            /* 11 */ "application/x-www-form-urlencoded",
            /* 12 */ "text/json", // non iana standard - if readable from casual users
            /* 13 */ "text/html",
            /* 14 */ "text/xml", // if readable from casual users (RFC 3023, section 3)
            /* 15 */ "text/css",
            /* 16 */ "text/csv",
            /* 17 */ "text/javascript",
            /* 18 */ "image/jpeg",
            /* 19 */ "image/png",
            /* 20 */ "image/gif",
        ];
    }

    pub const EMPTY: Encoding = Encoding {
        prefix: 0,
        suffix: std::borrow::Cow::Borrowed(""),
    };
    pub const APP_OCTET_STREAM: Encoding = Encoding {
        prefix: 1,
        suffix: std::borrow::Cow::Borrowed(""),
    };
    pub const APP_CUSTOM: Encoding = Encoding {
        prefix: 2,
        suffix: std::borrow::Cow::Borrowed(""),
    };
    pub const TEXT_PLAIN: Encoding = Encoding {
        prefix: 3,
        suffix: std::borrow::Cow::Borrowed(""),
    };
    pub const STRING: Encoding = TEXT_PLAIN;
    pub const APP_PROPERTIES: Encoding = Encoding {
        prefix: 4,
        suffix: std::borrow::Cow::Borrowed(""),
    };
    pub const APP_JSON: Encoding = Encoding {
        prefix: 5,
        suffix: std::borrow::Cow::Borrowed(""),
    };
    pub const APP_SQL: Encoding = Encoding {
        prefix: 6,
        suffix: std::borrow::Cow::Borrowed(""),
    };
    pub const APP_INTEGER: Encoding = Encoding {
        prefix: 7,
        suffix: std::borrow::Cow::Borrowed(""),
    };
    pub const APP_FLOAT: Encoding = Encoding {
        prefix: 8,
        suffix: std::borrow::Cow::Borrowed(""),
    };
    pub const APP_XML: Encoding = Encoding {
        prefix: 9,
        suffix: std::borrow::Cow::Borrowed(""),
    };
    pub const APP_XHTML_XML: Encoding = Encoding {
        prefix: 10,
        suffix: std::borrow::Cow::Borrowed(""),
    };
    pub const APP_X_WWW_FORM_URLENCODED: Encoding = Encoding {
        prefix: 11,
        suffix: std::borrow::Cow::Borrowed(""),
    };
    pub const TEXT_JSON: Encoding = Encoding {
        prefix: 12,
        suffix: std::borrow::Cow::Borrowed(""),
    };
    pub const TEXT_HTML: Encoding = Encoding {
        prefix: 13,
        suffix: std::borrow::Cow::Borrowed(""),
    };
    pub const TEXT_XML: Encoding = Encoding {
        prefix: 14,
        suffix: std::borrow::Cow::Borrowed(""),
    };
    pub const TEXT_CSS: Encoding = Encoding {
        prefix: 15,
        suffix: std::borrow::Cow::Borrowed(""),
    };
    pub const TEXT_CSV: Encoding = Encoding {
        prefix: 16,
        suffix: std::borrow::Cow::Borrowed(""),
    };
    pub const TEXT_JAVASCRIPT: Encoding = Encoding {
        prefix: 17,
        suffix: std::borrow::Cow::Borrowed(""),
    };
    pub const IMG_JPG: Encoding = Encoding {
        prefix: 18,
        suffix: std::borrow::Cow::Borrowed(""),
    };
    pub const IMG_PNG: Encoding = Encoding {
        prefix: 19,
        suffix: std::borrow::Cow::Borrowed(""),
    };
    pub const IMG_GIF: Encoding = Encoding {
        prefix: 20,
        suffix: std::borrow::Cow::Borrowed(""),
    };
}

#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    pub key: ZInt,
    pub value: Vec<u8>,
}

/// The global unique id of a zenoh peer.
#[derive(Clone, Eq)]
pub struct PeerId {
    size: usize,
    id: [u8; PeerId::MAX_SIZE],
}

impl PeerId {
    pub const MAX_SIZE: usize = 16;

    pub fn new(size: usize, id: [u8; PeerId::MAX_SIZE]) -> PeerId {
        PeerId { size, id }
    }

    #[inline]
    pub fn size(&self) -> usize {
        self.size
    }

    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.id[..self.size]
    }
}

impl From<uuid::Uuid> for PeerId {
    #[inline]
    fn from(uuid: uuid::Uuid) -> Self {
        PeerId {
            size: 16,
            id: *uuid.as_bytes(),
        }
    }
}

impl PartialEq for PeerId {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.size == other.size && self.as_slice() == other.as_slice()
    }
}

impl Hash for PeerId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state);
    }
}

impl fmt::Debug for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", hex::encode_upper(self.as_slice()))
    }
}

impl fmt::Display for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

// A PeerID can be converted into a Timestamp's ID
impl From<&PeerId> for uhlc::ID {
    fn from(pid: &PeerId) -> Self {
        uhlc::ID::new(pid.size, pid.id)
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Channel {
    BestEffort,
    Reliable,
}

/// The kind of congestion control.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum CongestionControl {
    Block,
    Drop,
}

impl Default for CongestionControl {
    #[inline]
    fn default() -> CongestionControl {
        *CONGESTION_CONTROL_DEFAULT
    }
}

impl FromStr for CongestionControl {
    type Err = ZError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "block" => Ok(CongestionControl::Block),
            "drop" => Ok(CongestionControl::Drop),
            _ => {
                let e = format!(
                    "Invalid CongestionControl: {}. Valid values are: 'block' | 'drop'",
                    s
                );
                log::warn!("{}", e);
                zerror!(ZErrorKind::Other { descr: e })
            }
        }
    }
}

/// The kind of reliability.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Reliability {
    BestEffort,
    Reliable,
}

impl Default for Reliability {
    #[inline]
    fn default() -> Self {
        Reliability::Reliable
    }
}

/// The subscription mode.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum SubMode {
    Push,
    Pull,
}

impl Default for SubMode {
    #[inline]
    fn default() -> Self {
        SubMode::Push
    }
}

/// A time period.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Period {
    pub origin: ZInt,
    pub period: ZInt,
    pub duration: ZInt,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SubInfo {
    pub reliability: Reliability,
    pub mode: SubMode,
    pub period: Option<Period>,
}

impl Default for SubInfo {
    fn default() -> SubInfo {
        SubInfo {
            reliability: Reliability::default(),
            mode: SubMode::default(),
            period: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct QueryableInfo {
    pub complete: ZInt,
    pub distance: ZInt,
}

impl Default for QueryableInfo {
    fn default() -> QueryableInfo {
        QueryableInfo {
            complete: 1,
            distance: 0,
        }
    }
}

pub mod queryable {
    pub const ALL_KINDS: super::ZInt = 0x01;
    pub const STORAGE: super::ZInt = 0x02;
    pub const EVAL: super::ZInt = 0x04;
}

/// The kind of consolidation.
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum ConsolidationMode {
    None,
    Lazy,
    Full,
}

/// The kind of consolidation that should be applied on replies to a [`get`](crate::Session::get)
/// at different stages of the reply process.
#[derive(Debug, Clone, PartialEq)]
pub struct QueryConsolidation {
    pub first_routers: ConsolidationMode,
    pub last_router: ConsolidationMode,
    pub reception: ConsolidationMode,
}

impl QueryConsolidation {
    pub fn none() -> Self {
        Self {
            first_routers: ConsolidationMode::None,
            last_router: ConsolidationMode::None,
            reception: ConsolidationMode::None,
        }
    }
}

impl Default for QueryConsolidation {
    fn default() -> Self {
        Self {
            first_routers: ConsolidationMode::Lazy,
            last_router: ConsolidationMode::Lazy,
            reception: ConsolidationMode::Full,
        }
    }
}

/// The [`Queryable`](crate::Queryable)s that should be target of a [`get`](crate::Session::get).
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    BestMatching,
    All,
    AllComplete,
    None,
    #[cfg(feature = "complete_n")]
    Complete(ZInt),
}

impl Default for Target {
    fn default() -> Self {
        Target::BestMatching
    }
}

/// The [`Queryable`](crate::Queryable)s that should be target of a [`get`](crate::Session::get).
#[derive(Debug, Clone, PartialEq)]
pub struct QueryTarget {
    pub kind: ZInt,
    pub target: Target,
}

impl Default for QueryTarget {
    fn default() -> Self {
        QueryTarget {
            kind: queryable::ALL_KINDS,
            target: Target::default(),
        }
    }
}
