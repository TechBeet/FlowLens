# wpdlib

This directory holds the MSVC import libraries (`wpcap.lib`, `Packet.lib`)
required to build the `flowlens-capture` crate's real packet-capture engine
against the Npcap SDK on Windows.

- They are committed so the repository builds on a plain Windows host without
  having to install the full Npcap SDK (only the Npcap runtime is needed).
- `regenerate.ps1` documents how to regenerate them from an installed
  `wpcap.dll` / `Packet.dll` pair using MSVC's `dumpbin.exe` + `lib.exe`.
- On Linux/ macOS the `pcap` crate links against `libpcap` directly and these
  files are unused.

The `.cargo/config.toml` picks them up when building on Windows.
