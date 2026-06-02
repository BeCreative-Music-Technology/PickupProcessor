use std::net::TcpListener;
use crate::error::Error;
use crate::external_connection::Connection;
use crate::routing_director::RoutingDirector;

pub struct VcsgpConnection {
  listener: TcpListener,
}

impl Connection for VcsgpConnection {
  ///
  /// Opens a new VCSGP (Very Cool and Shitty Guitar Protocol) listener using a TCP/IP socket.
  ///
  /// `connections_str` takes a connection string with the following info:
  /// `[ip_address]:[port_number]`.
  ///
  fn new(connection_str: &str) -> Result<Self, Error>
  where
      Self: Sized
  {
    let listener = match TcpListener::bind(connection_str) {
      Ok(listener) => listener,
      Err(_) => return Err(Error::new("Could not create TCP socket"))
    };

    Ok(Self { listener })
  }

  fn start(&mut self, routing_director: RoutingDirector) {
    todo!()
  }
}




// impl VcsgpConnection {
//   fn listen(&self, callback: Box<dyn Fn(&str)>) {
//     for stream in self.listener.incoming() {
//       let Ok(mut stream) = stream else {
//         continue;
//       };
//
//       let mut buffer = Vec::new();
//       if stream.read_to_end(&mut buffer).is_ok() && !buffer.is_empty() {
//         let message = String::from_utf8_lossy(&buffer);
//         callback(&message);
//       }
//     }
//   }
// }
