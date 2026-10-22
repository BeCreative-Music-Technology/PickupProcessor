# Changelog

## Unreleased: panel integration and laser strings

### Panel communication (VCSGP)
- The listener no longer stops when a client disconnects abruptly or sends invalid data; each client gets its own thread.
- New optional fields, all already sent by the QUTI panel:
  - `main_volume`: volume applied after every bus.
  - `enabled` per effect: disabled effects are left out of the chain.
  - `input_control_id` may be empty or missing for unmapped parameters (no more "control input [] not found" errors).
- A parameter may appear more than once with different `input_control_id`s, so several controls can drive one parameter.
- Effects are built before the routing lock is taken, and a bus's chain is swapped in one step, so sending a new setup no longer interrupts audio.
- `api_contract.json` now shows every effect type and is used by the tests.

### Laser strings (from `feature/laser-strings`, fixed)
- Each string is its own control input: `laser_0`, `laser_1`, `laser_2`. Before, all three shared `laser_0`, and the panel's ids were never found.
- Fixed the distance conversion. Whole-number math made every value 0, and readings under 50 mm crashed debug builds.
- Readings are cached per ToF sensor. Before, "not ready yet" (most polls) was treated as 0 mm.
- Out-of-range readings (~8190 mm) are ignored.
- GPIO/I2C setup errors are logged instead of silently ending the laser thread.
- The loop sleeps 1 ms and only logs touch/release.
- The rotary and the lasers now run together.

### Audio performance
- The routing loop and the bus threads no longer spin at 100% CPU and lock once per sample; they process in chunks and sleep when idle.
- Ring buffers are 4 periods long, so a late thread no longer drops a whole period.
- Fixed a race where re-enabling a bus could leave its thread parked forever.
- Filter, reverb and delay use JACK's sample rate instead of fundsp's default 44.1 kHz.
- Low-pass coefficients and gain are only recalculated when their value changes.
- Fixed a leak where every new setup left old control observers registered.

### Other
- JACK client setup returns errors instead of panicking; JACK clients are kept active without parking threads.
- An invalid buffer-length argument prints a clear message.
- `start.sh` no longer has a BOM before the shebang.
- Clippy warnings went from 27 to 6 (only unused API remains).
