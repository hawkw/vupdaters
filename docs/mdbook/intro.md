# Introduction

`vupdaters` is a set of open-source tools for controlling the [VU-1 USB dials]
from [Streacom](https://streacom.com), written in Rust. Currently, `vupdaters`
consists of two binary applications:

- [`dialctl`](dialctl.md), a command-line tool for querying information about
  the dials connected to the system and manually setting their values,

- [`vupdated`](vupdated.md), a [daemon] that runs in the background and
  continually updates the dials to display system information, based on a
  [config file](vupdated/config.md).

Both of these applications depend on [VU-Server], which provides an HTTP API for
configuring the dials. For more information on how to install and configure
VU-Server, see the official documentation on
[vudials.com](https://vudials.com/).

To install `vupdaters`, see [here](/vupdaters/artifacts/).

[VU-1 USB dials]: https://streacom.com/products/vu1-dynamic-analogue-dials/
[daemon]: https://en.wikipedia.org/wiki/Daemon_(computing)
[VU-Server]: https://github.com/SasaKaranovic/VU-Server
