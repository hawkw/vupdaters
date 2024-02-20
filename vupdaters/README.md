# vupdaters

[![CI]](https://github.com/hawkw/vupdaters/actions/workflows/ci.yml)
[![GitHub License]](https://github.com/hawkw/vupdaters/LICENSE)
[![FlakeHub]](https://flakehub.com/flake/mycoliza/vu-server)
[![GitHub Downloads]](https://www.elizas.website/vupdaters/artifacts/)
[![GitHub Sponsors]](https://github.com/sponsors/hawkw/)


[CI]: https://github.com/hawkw/vupdaters/actions/workflows/ci.yml/badge.svg
[GitHub License]: https://img.shields.io/github/license/hawkw/vupdaters?style=flat
[FlakeHub]:
    https://img.shields.io/endpoint?url=https://flakehub.com/f/mycoliza/vupdaters/badge
[GitHub Downloads]:
    https://img.shields.io/github/downloads/hawkw/vupdaters/total?logo=github
[GitHub Sponsors]: https://img.shields.io/github/sponsors/hawkw?logo=github

Tools for controlling [Streacom VU-1 dials], written in Rust.

This package provides two binaries:

 - `vupdated`: A daemon process that runs in the background and periodically
   updates the VU-1 dials, with system statistics, based on a configuration
   file.
 - `dialctl`: A command-line tool for getting the current state of the dials
   and updating them manually.

Both `dialctl` and `vupdated` depend on a running instance of the [VU-Server]
application. See [the VU-1 dials documentation](https://vudials.com/) for details.

[Streacom VU-1 dials]: https://streacom.com/products/vu1-dynamic-analogue-dials/
[VU-Server]: https://github.com/SasaKaranovic/VU-Server
