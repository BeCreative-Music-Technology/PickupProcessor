use crate::error::Error;
use crate::routing_director::RoutingDirector;

pub trait Connection {
  fn new(connection_str: &str) -> Result<Self, Error> where Self: Sized;
  fn start(&mut self, routing_director: RoutingDirector);
}
