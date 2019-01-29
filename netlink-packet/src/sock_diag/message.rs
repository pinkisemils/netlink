use std::net::{SocketAddr, SocketAddrV4, SocketAddrV6};
use std::time::Duration;

use failure::ResultExt;
use netlink_sys::constants::{AF_INET, AF_INET6};

use crate::sock_diag::{
    buffer::{
        Extension, InetDiagAttr, InetDiagMsgBuffer, InetDiagReqV2Buffer, TcpStates, UnixDiagBuffer,
    },
    inet_diag::{extension, tcp_state},
    sock_diag::SOCK_DIAG_BY_FAMILY,
};
use crate::{DecodeError, Emitable, Parseable};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum SockDiagMessage {
    InetDiag(InetDiagRequest),
    InetSocks(InetDiagResponse),
    UnixDiag(UnixDiagRequest),
    UnixSocks(UnixDiagResponse),
}

impl Emitable for SockDiagMessage {
    fn buffer_len(&self) -> usize {
        use SockDiagMessage::*;

        match self {
            InetDiag(ref req) => req.buffer_len(),
            UnixDiag(ref req) => req.buffer_len(),
            _ => unimplemented!(),
        }
    }

    fn emit(&self, buf: &mut [u8]) {
        use SockDiagMessage::*;

        match self {
            InetDiag(ref req) => req.emit(buf),
            UnixDiag(ref req) => req.emit(buf),
            _ => unimplemented!(),
        }
    }
}

impl SockDiagMessage {
    pub(crate) fn parse(message_type: u16, buffer: &[u8]) -> Result<Self, DecodeError> {
        match message_type {
            SOCK_DIAG_BY_FAMILY
                if buffer.first() == Some(&(AF_INET as u8))
                    || buffer.first() == Some(&(AF_INET6 as u8)) =>
            {
                Ok(SockDiagMessage::InetSocks(
                    InetDiagMsgBuffer::new_checked(buffer)
                        .context("failed to parse SOCK_DIAG_BY_FAMILY message")?
                        .parse()
                        .context("failed to parse SOCK_DIAG_BY_FAMILY message")?,
                ))
            }
            _ => Err(format!("Unknown message type: {}", message_type).into()),
        }
    }

    pub fn message_type(&self) -> u16 {
        SOCK_DIAG_BY_FAMILY
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SockId {
    pub src: Option<SocketAddr>,
    pub dst: Option<SocketAddr>,
    pub interface: u32,
    pub cookie: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InetDiagRequest {
    pub family: u8,
    pub protocol: u8,
    pub extensions: Extension,
    pub states: TcpStates,
    pub id: SockId,
}

pub fn inet(protocol: u8) -> InetDiagRequest {
    InetDiagRequest::new(AF_INET as u8, protocol)
}

pub fn inet6(protocol: u8) -> InetDiagRequest {
    InetDiagRequest::new(AF_INET6 as u8, protocol)
}

impl InetDiagRequest {
    pub fn new(family: u8, protocol: u8) -> InetDiagRequest {
        InetDiagRequest {
            family,
            protocol,
            extensions: Extension::empty(),
            states: TcpStates::All,
            id: SockId::default(),
        }
    }

    pub fn with_state(mut self, state: tcp_state) -> Self {
        self.states
            .insert(TcpStates::from_bits_truncate(1 << state as usize));
        self
    }

    pub fn without_state(mut self, state: tcp_state) -> Self {
        self.states
            .remove(TcpStates::from_bits_truncate(1 << state as usize));
        self
    }

    pub fn with_extension(mut self, ext: extension) -> Self {
        self.extensions
            .insert(Extension::from_bits_truncate(1 << (ext as usize - 1)));
        self
    }
}

impl Emitable for InetDiagRequest {
    fn buffer_len(&self) -> usize {
        InetDiagReqV2Buffer::<()>::len()
    }

    fn emit(&self, buf: &mut [u8]) {
        let mut req = InetDiagReqV2Buffer::new(buf);

        req.set_family(self.family);
        req.set_protocol(self.protocol);
        req.set_extensions(self.extensions);
        req.set_states(self.states);

        let mut id = req.id_mut();

        if let Some(addr) = self.id.src.as_ref() {
            id.set_src_addr(addr)
        }
        if let Some(addr) = self.id.dst.as_ref() {
            id.set_dst_addr(addr)
        }
        id.set_interface(self.id.interface);
        if let Some(cookie) = self.id.cookie {
            id.set_cookie(cookie)
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct InetDiagResponse {
    pub family: u8,
    pub state: tcp_state,
    pub timer: u8,
    pub retrans: u8,
    pub id: SockId,
    pub expires: Option<Duration>,
    pub rqueue: u32,
    pub wqueue: u32,
    pub uid: u32,
    pub inode: u32,
    pub attrs: Vec<InetDiagAttr>,
}

impl<T: AsRef<[u8]>> Parseable<InetDiagResponse> for InetDiagMsgBuffer<T> {
    fn parse(&self) -> Result<InetDiagResponse, DecodeError> {
        let family = self.family();
        let id = self.id();

        let (src, dst) = match family as u16 {
            AF_INET => {
                let sip = id.src_ipv4();
                let sport = id.sport();
                let dip = id.dst_ipv4();
                let dport = id.dport();

                (
                    if sip.is_unspecified() && sport == 0 {
                        None
                    } else {
                        Some(SocketAddrV4::new(sip, sport).into())
                    },
                    if dip.is_unspecified() && dport == 0 {
                        None
                    } else {
                        Some(SocketAddrV4::new(dip, dport).into())
                    },
                )
            }
            AF_INET6 => {
                let sip = id.src_ipv6();
                let sport = id.sport();
                let dip = id.dst_ipv6();
                let dport = id.dport();

                (
                    if sip.is_unspecified() && sport == 0 {
                        None
                    } else {
                        Some(SocketAddrV6::new(sip, sport, 0, 0).into())
                    },
                    if dip.is_unspecified() && dport == 0 {
                        None
                    } else {
                        Some(SocketAddrV6::new(dip, dport, 0, 0).into())
                    },
                )
            }
            _ => (None, None),
        };

        let attrs = self.attrs().collect::<Result<Vec<_>, DecodeError>>()?;

        Ok(InetDiagResponse {
            family,
            state: self.state(),
            timer: self.timer(),
            retrans: self.retrans(),
            id: SockId {
                src,
                dst,
                interface: id.interface(),
                cookie: id.cookie(),
            },
            expires: self.expires(),
            rqueue: self.rqueue(),
            wqueue: self.wqueue(),
            uid: self.uid(),
            inode: self.inode(),
            attrs,
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct UnixDiagRequest {}

impl Emitable for UnixDiagRequest {
    fn buffer_len(&self) -> usize {
        unimplemented!()
    }

    fn emit(&self, buf: &mut [u8]) {
        unimplemented!()
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct UnixDiagResponse {}

impl<T: AsRef<[u8]>> Parseable<UnixDiagResponse> for UnixDiagBuffer<T> {
    fn parse(&self) -> Result<UnixDiagResponse, DecodeError> {
        unimplemented!()
    }
}
