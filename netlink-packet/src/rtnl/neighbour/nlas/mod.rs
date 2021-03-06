mod cache_info;
pub use self::cache_info::*;

use std::mem::size_of;

use byteorder::{ByteOrder, NativeEndian};
use failure::ResultExt;

use crate::constants::*;
use crate::utils::{parse_u16, parse_u32};
use crate::{DecodeError, DefaultNla, Emitable, Nla, NlaBuffer, Parseable};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum NeighbourNla {
    Unspec(Vec<u8>),
    Destination(Vec<u8>),
    LinkLocalAddress(Vec<u8>),
    CacheInfo(NeighbourCacheInfo),
    Probes(Vec<u8>),
    Vlan(u16),
    Port(Vec<u8>),
    Vni(u32),
    IfIndex(u32),
    Master(Vec<u8>),
    LinkNetNsId(Vec<u8>),
    SourceVni(u32),
    Other(DefaultNla),
}

impl Nla for NeighbourNla {
    #[rustfmt::skip]
    fn value_len(&self) -> usize {
        use self::NeighbourNla::*;
        match *self {
            Unspec(ref bytes)
            | Destination(ref bytes)
            | LinkLocalAddress(ref bytes)
            | Probes(ref bytes)
            | Port(ref bytes)
            | Master(ref bytes)
            | LinkNetNsId(ref bytes) => bytes.len(),
            CacheInfo(_) => NEIGHBOUR_CACHE_INFO_LEN,
            Vlan(_) => size_of::<u16>(),
            Vni(_)
            | IfIndex(_)
            | SourceVni(_) => size_of::<u32>(),
            Other(ref attr) => attr.value_len(),
        }
    }

    #[rustfmt::skip]
    fn emit_value(&self, buffer: &mut [u8]) {
        use self::NeighbourNla::*;
        match *self {
            Unspec(ref bytes)
            | Destination(ref bytes)
            | LinkLocalAddress(ref bytes)
            | Probes(ref bytes)
            | Port(ref bytes)
            | Master(ref bytes)
            | LinkNetNsId(ref bytes) => buffer.copy_from_slice(bytes.as_slice()),
            CacheInfo(ref cacheinfo) => cacheinfo.emit(buffer),
            Vlan(ref value) => NativeEndian::write_u16(buffer, *value),
            Vni(ref value)
            | IfIndex(ref value)
            | SourceVni(ref value) => NativeEndian::write_u32(buffer, *value),
            Other(ref attr) => attr.emit_value(buffer),
        }
    }

    fn kind(&self) -> u16 {
        use self::NeighbourNla::*;
        match *self {
            Unspec(_) => NDA_UNSPEC,
            Destination(_) => NDA_DST,
            LinkLocalAddress(_) => NDA_LLADDR,
            CacheInfo(_) => NDA_CACHEINFO,
            Probes(_) => NDA_PROBES,
            Vlan(_) => NDA_VLAN,
            Port(_) => NDA_PORT,
            Vni(_) => NDA_VNI,
            IfIndex(_) => NDA_IFINDEX,
            Master(_) => NDA_MASTER,
            LinkNetNsId(_) => NDA_LINK_NETNSID,
            SourceVni(_) => NDA_SRC_VNI,
            Other(ref nla) => nla.kind(),
        }
    }
}

impl<'buffer, T: AsRef<[u8]> + ?Sized> Parseable<NeighbourNla> for NlaBuffer<&'buffer T> {
    fn parse(&self) -> Result<NeighbourNla, DecodeError> {
        use self::NeighbourNla::*;
        let payload = self.value();
        Ok(match self.kind() {
            NDA_UNSPEC => Unspec(payload.to_vec()),
            NDA_DST => Destination(payload.to_vec()),
            NDA_LLADDR => LinkLocalAddress(payload.to_vec()),
            NDA_CACHEINFO => CacheInfo(
                NeighbourCacheInfoBuffer::new(payload)
                    .parse()
                    .context("invalid NDA_CACHEINFO value")?,
            ),
            NDA_PROBES => Probes(payload.to_vec()),
            NDA_VLAN => Vlan(parse_u16(payload)?),
            NDA_PORT => Port(payload.to_vec()),
            NDA_VNI => Vni(parse_u32(payload)?),
            NDA_IFINDEX => IfIndex(parse_u32(payload)?),
            NDA_MASTER => Master(payload.to_vec()),
            NDA_LINK_NETNSID => LinkNetNsId(payload.to_vec()),
            NDA_SRC_VNI => SourceVni(parse_u32(payload)?),
            _ => Other(
                <Self as Parseable<DefaultNla>>::parse(self)
                    .context("invalid link NLA value (unknown type)")?,
            ),
        })
    }
}
