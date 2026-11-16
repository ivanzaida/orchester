#![allow(unsafe_code)]
//! The system's IPv4 TCP listeners with each one's owning pid (`GetExtendedTcpTable`), which has no safe wrapper.

use std::{io, net::Ipv4Addr};

use windows_sys::Win32::{
  Foundation::{ERROR_INSUFFICIENT_BUFFER, NO_ERROR},
  NetworkManagement::IpHelper::{
    GetExtendedTcpTable, MIB_TCPROW_OWNER_PID, MIB_TCPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_LISTENER,
  },
};

/// `AF_INET`: the IPv4 table, which `127.0.0.1` is in.
const IPV4: u32 = 2;
/// How often the table is asked for again when it grew between asking its size and reading it.
const ATTEMPTS: usize = 4;

pub(super) fn loopback_owner(port: u16) -> io::Result<Option<u32>> {
  Ok(
    listeners()?
      .into_iter()
      .find(|row| local_port(row) == Some(port) && accepts_loopback(row))
      .map(|row| row.dwOwningPid),
  )
}

/// The port of `row`, kept in network byte order in the low 16 bits of its field.
fn local_port(row: &MIB_TCPROW_OWNER_PID) -> Option<u16> {
  u16::try_from(row.dwLocalPort & 0xFFFF).ok().map(u16::from_be)
}

/// Whether `row` listens on `127.0.0.1`: on a loopback address or on every address.
fn accepts_loopback(row: &MIB_TCPROW_OWNER_PID) -> bool {
  // The address is in network byte order: its bytes in memory are the address's in order.
  let address = Ipv4Addr::from(row.dwLocalAddr.to_ne_bytes());
  address.is_loopback() || address.is_unspecified()
}

fn listeners() -> io::Result<Vec<MIB_TCPROW_OWNER_PID>> {
  let mut size = 0_usize;
  for _ in 0..ATTEMPTS {
    // Whole u32 words, so the table's fields are aligned.
    let mut buffer = vec![0_u32; size.div_ceil(4).max(1)];
    let mut given = u32::try_from(buffer.len() * 4).map_err(io::Error::other)?;
    // SAFETY: `buffer` is writable for `given` bytes and aligned for the table, whose fields are u32; the call writes
    // at most `given` bytes and says how many it needs when that is too few.
    let result = unsafe {
      GetExtendedTcpTable(
        buffer.as_mut_ptr().cast(),
        &raw mut given,
        0,
        IPV4,
        TCP_TABLE_OWNER_PID_LISTENER,
        0,
      )
    };
    match result {
      NO_ERROR => return Ok(rows(&buffer)),
      ERROR_INSUFFICIENT_BUFFER => size = given as usize,
      error => {
        return Err(i32::try_from(error).map_or_else(
          |_| io::Error::other(format!("the TCP table cannot be read: error {error}")),
          io::Error::from_raw_os_error,
        ));
      }
    }
  }
  Err(io::Error::other("the TCP table kept growing while it was read"))
}

/// The rows of the table `GetExtendedTcpTable` wrote into `buffer`.
fn rows(buffer: &[u32]) -> Vec<MIB_TCPROW_OWNER_PID> {
  let table = buffer.as_ptr().cast::<MIB_TCPTABLE_OWNER_PID>();
  // SAFETY: the call succeeded, so `buffer` holds a table: its count, then that many rows, all within the buffer.
  unsafe {
    let count = (*table).dwNumEntries as usize;
    let first = (&raw const (*table).table).cast::<MIB_TCPROW_OWNER_PID>();
    std::slice::from_raw_parts(first, count).to_vec()
  }
}
