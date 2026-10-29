use std::collections::HashMap;
use std::io::Write;
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use serde::Serialize;
use crate::control_input::{ControlInput, ControlState};
use crate::logger;

static LOG_ENVIRONMENT: &str = "ControlFeedback";

/// State of one control input as sent to the panel.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct ControlInfo {
  pub id: String,
  pub kind: String,
  pub online: bool,
  pub touched: bool,
  /// Latest position (`0..=65535`), `null` until the control produced one.
  pub value: Option<u16>,
}

/// Messages the engine sends to connected panels, one JSON object per line.
#[derive(Serialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FeedbackMessage {
  /// Sent once when a panel connects: every control input and its current state.
  Hello { controls: Vec<ControlInfo> },
  /// Sent when controls change, containing only the controls that changed.
  ControlState { controls: Vec<ControlInfo> },
}

impl FeedbackMessage {
  pub fn to_line(&self) -> String {
    let mut line = serde_json::to_string(self).unwrap_or_default();
    line.push('\n');
    line
  }
}

///
/// Sends the live state of the control inputs (rotary position, laser touches) to every connected panel,
/// so the panel can show what the hardware is doing.
///
pub struct ControlFeedback {
  control_inputs: Arc<Mutex<Vec<Box<dyn ControlInput>>>>,
  clients: Mutex<Vec<(String, TcpStream)>>,
}

impl ControlFeedback {
  // ~30 updates per second is smooth enough for meters without flooding the network
  const INTERVAL: Duration = Duration::from_millis(33);
  // A panel that stops reading must not stall the updates for everyone else
  const WRITE_TIMEOUT: Duration = Duration::from_millis(100);

  /// Starts sending control states to the clients added with `add_client()`.
  pub fn start(control_inputs: Arc<Mutex<Vec<Box<dyn ControlInput>>>>) -> Arc<Self> {
    let feedback = Arc::new(Self { control_inputs, clients: Mutex::new(Vec::new()) });
    let thread_feedback = feedback.clone();
    thread::spawn(move || thread_feedback.run());
    feedback
  }

  /// Sends the current state of every control to a newly connected panel and keeps it updated from then on.
  pub fn add_client(&self, peer: String, mut stream: TcpStream) {
    if let Err(e) = stream.set_write_timeout(Some(Self::WRITE_TIMEOUT)) {
      logger::warn(LOG_ENVIRONMENT, &format!("could not set write timeout for {}: {}", peer, e));
    }

    let controls = self.snapshot().into_iter().map(|(info, _)| info).collect();
    match stream.write_all(FeedbackMessage::Hello { controls }.to_line().as_bytes()) {
      Ok(()) => self.clients.lock().unwrap().push((peer, stream)),
      Err(e) => logger::warn(LOG_ENVIRONMENT, &format!("could not send control states to {}: {}", peer, e)),
    }
  }

  fn run(&self) {
    let mut sent_revisions: HashMap<String, u32> = HashMap::new();
    loop {
      thread::sleep(Self::INTERVAL);

      let changed = Self::changed_controls(self.snapshot(), &mut sent_revisions);
      if !changed.is_empty() {
        self.broadcast(&FeedbackMessage::ControlState { controls: changed }.to_line());
      }
    }
  }

  fn snapshot(&self) -> Vec<(ControlInfo, u32)> {
    self.control_inputs.lock().unwrap()
      .iter()
      .map(|control_input| {
        let state: ControlState = control_input.observable().state();
        let info = ControlInfo {
          id: control_input.id().to_string(),
          kind: control_input.kind().to_string(),
          online: state.online,
          touched: state.touched,
          value: state.value,
        };
        (info, state.revision)
      })
      .collect()
  }

  /// Controls whose revision differs from the last one sent; remembers the new revisions.
  fn changed_controls(snapshot: Vec<(ControlInfo, u32)>, sent_revisions: &mut HashMap<String, u32>) -> Vec<ControlInfo> {
    snapshot.into_iter()
      .filter(|(info, revision)| sent_revisions.insert(info.id.clone(), *revision) != Some(*revision))
      .map(|(info, _)| info)
      .collect()
  }

  /// Writes a line to every client, dropping clients that disconnected.
  fn broadcast(&self, line: &str) {
    self.clients.lock().unwrap().retain_mut(|(peer, stream)| match stream.write_all(line.as_bytes()) {
      Ok(()) => true,
      Err(e) => {
        logger::info(LOG_ENVIRONMENT, &format!("stopped sending control states to {}: {}", peer, e));
        false
      }
    });
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::io::{BufRead, BufReader};
  use std::net::TcpListener;
  use crate::control_input::{ControlChange, ObservableControlInput};

  struct FakeLaser {
    observable: Arc<ObservableControlInput>,
  }

  impl ControlInput for FakeLaser {
    fn id(&self) -> &str { "laser_0" }
    fn kind(&self) -> &str { "laser" }
    fn observable(&self) -> Arc<ObservableControlInput> { self.observable.clone() }
  }

  fn info(id: &str) -> ControlInfo {
    ControlInfo { id: id.to_string(), kind: "laser".to_string(), online: true, touched: false, value: None }
  }

  #[test]
  fn only_changed_controls_are_sent_again() {
    let mut sent = HashMap::new();
    let first = ControlFeedback::changed_controls(vec![(info("laser_0"), 1), (info("laser_1"), 1)], &mut sent);
    assert_eq!(first.len(), 2, "everything is new the first time");

    let second = ControlFeedback::changed_controls(vec![(info("laser_0"), 1), (info("laser_1"), 2)], &mut sent);
    assert_eq!(second, vec![info("laser_1")]);
  }

  #[test]
  fn messages_have_a_type_and_end_with_a_newline() {
    let line = FeedbackMessage::ControlState { controls: vec![info("laser_0")] }.to_line();
    assert!(line.ends_with('\n'));
    let json: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(json["type"], "control_state");
    assert_eq!(json["controls"][0]["id"], "laser_0");
    assert!(json["controls"][0]["value"].is_null());
  }

  #[test]
  fn connected_panel_receives_hello_and_updates() {
    let observable = Arc::new(ObservableControlInput::new());
    let control_inputs: Arc<Mutex<Vec<Box<dyn ControlInput>>>> =
      Arc::new(Mutex::new(vec![Box::new(FakeLaser { observable: observable.clone() })]));
    let feedback = ControlFeedback::start(control_inputs);

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let panel = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    panel.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let (engine_side, _) = listener.accept().unwrap();
    feedback.add_client("test".to_string(), engine_side);

    let mut lines = BufReader::new(panel).lines();
    let hello: serde_json::Value = serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
    assert_eq!(hello["type"], "hello");
    assert_eq!(hello["controls"][0]["kind"], "laser");

    observable.notify_touch(true);
    observable.notify(&ControlChange { control_id: "laser_0".to_string(), value: 1234 });
    let update: serde_json::Value = loop {
      let message: serde_json::Value = serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
      if message["controls"][0]["value"] == 1234 {
        break message;
      }
    };
    assert_eq!(update["type"], "control_state");
    assert_eq!(update["controls"][0]["touched"], true);
  }
}
