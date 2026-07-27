# ICL Light Stage Server monorepo

This repo contains:

- [kinetrs](kinetrs/)
  - a Philips KiNET library
  - published on [crates.io][cratesio-kinetrs]
- [lsserver](lsserver/)
  - Source code for the `lsserver` binary, to control the light stage.
  - For precompiled binaries, see [releases][releases] in the side panel.
- [lightstagepi](lightstagepi/)
  - provisioning scripts and utils for running `lsserver` on a RPi.
- [kinet.lua](kinet.lua)
  - wireshark dissector for the KiNET protocol

For more information, see the [wiki][wiki].

[wiki]: https://github.com/lightstageurop/lightstage-server-rs/wiki
[cratesio-kinetrs]: https://crates.io/crates/kinetrs
[releases]: https://github.com/lightstageurop/lightstage-server-rs/releases
