use std::sync::{Arc, Mutex};
use crate::error::Error;
use crate::routing_director::RoutingDirector;

pub trait ExternalConnection {
  fn new(connection_str: &str) -> Result<Self, Error> where Self: Sized;
  fn start(&mut self, routing_director: Arc<Mutex<RoutingDirector>>);
}
