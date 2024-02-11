# Using `dialctl`

`dialctl` is a command-line tool for interacting with the [VU-Server HTTP API].
You can use `dialctl` to list all VU-1 dials connected to your system, to show
detailed status information about the dials, and to set a dial's value,
backlight color, background image, and easing configuration.

All `dialctl` commands require a [VU-Server API key]. The API key is either read
from the `$VU_SERVER_API_KEY`, or from the `--key` CLI option. The address of
the VU-Server instance can be configured by the `--server` CLI option.

For more detailed usage information, use `dialctl help`.

[VU-Server HTTP API]: https://docs.vudials.com/api_messaging/
[VU-Server API key]: https://docs.vudials.com/webui/manage_keys/