# vupdaters

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
