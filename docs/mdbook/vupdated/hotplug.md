# USB hotplug support

`VU-Server` [does not currently support hotplugging][no-hotplug] of the VU1
dials. When the dials are disconnected, such as by disconnecting a laptop from a
dock, USB hub, or KVM switch, or unplugging the dials directly, the server
process will crash and enter a failure state where subsequent HTTP requests to
update the dials silently do nothing. Unfortunately, because the server does not
handle hotplug events, if the dials are then reconnected to the system, the
server does not recover from the failed state, even though the dials are once
again present. This makes using the VU1 dials with a laptop quite annoying,
since the `VU-Server` process must be manually restarted whenever the dials are
reconnected.

To work around this, `vupdated` implements a brute-force solution to add USB
hotplug support to `VU-Server`. If hotplug support is enabled, `vupdated` will
watch for USB devices being connected or disconnected from the system. When a
device that appears to be the VU-1 dials is connected, `vupdated` will restart
restart VU-Server, if it is running as a systemd service.

[no-hotplug]: https://github.com/SasaKaranovic/VU-Server/issues/10
