# vupdaters

Tools for controlling [Streacom VU-1 dials], written in Rust.

This repository contains the following crates:

- [`vupdaters`]: Contains the following application binaries:
  - `vupdated`: A daemon process that runs in the background and periodically
    updates the VU-1 dials, based on a configuration file.
  - `dialctl`: A command-line tool for getting the current state of the dials
    and updating them manually.
- [`vu-api`]: A Rust library providing API bindings for the [VU-Server HTTP
      API].

[Streacom VU-1 dials]: https://streacom.com/products/vu1-dynamic-analogue-dials/
[VU-Server HTTP API]: https://docs.vudials.com/api_messaging/
[`vupdaters`]: https://github.com/hawkw/vupdaters/tree/main/vupdaters
[`vu-api`]: https://github.com/hawkw/vupdaters/tree/main/api