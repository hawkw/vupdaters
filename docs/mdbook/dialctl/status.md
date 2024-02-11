# querying dial status

The `dialctl status` command can be used to query the detailed status of a
single dial. The dial can be selected either by index, with `--index <INDEX>`,
or by UID, with `--dial <UID>`.

## examples

```bash
# selecting the dial by index
dialctl --key $VU_SERVER_API_KEY status --index 0
DIAL: 630032000650564139323920
├─name: CPU Load
├─value: 0
├─index: 0
├─rgbw: [Value(50), Value(50), Value(50), Value(0)]
├─image file: img_blank
├─DIAL EASING:
│ ├─dial step: 2
│ └─dial period: 50
├─BACKLIGHT EASING:
│ ├─backlight step: 5
│ └─backlight period: 100
├─VERSION:
│ ├─firmware hash: ?
│ ├─firmware version: ?
│ ├─hardware version: ?
│ └─protocol version: V1
├─BACKLIGHT:
│ ├─red: 50
│ ├─green: 50
│ └─blue: 50
├─STATUS:
│ ├─value_changed: false
│ ├─backlight_changed: false
│ └─image_changed: false
└─update deadline: 1707604486.3434525
```

```bash
# selecting the dial by UID
dialctl --key $VU_SERVER_API_KEY status --dial 630032000650564139323920
DIAL: 630032000650564139323920
├─name: CPU Load
├─value: 0
├─index: 0
├─rgbw: [Value(50), Value(50), Value(50), Value(0)]
├─image file: img_blank
├─DIAL EASING:
│ ├─dial step: 2
│ └─dial period: 50
├─BACKLIGHT EASING:
│ ├─backlight step: 5
│ └─backlight period: 100
├─VERSION:
│ ├─firmware hash: ?
│ ├─firmware version: ?
│ ├─hardware version: ?
│ └─protocol version: V1
├─BACKLIGHT:
│ ├─red: 50
│ ├─green: 50
│ └─blue: 50
├─STATUS:
│ ├─value_changed: false
│ ├─backlight_changed: false
│ └─image_changed: false
└─update deadline: 1707604486.3434525
```