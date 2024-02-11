# listing dials

The `dialctl list` command queries the VU-Server to list all connected dials.

## examples

```bash
$ dialctl --key $VU_SERVER_API_KEY list
DIAL: 630032000650564139323920
├─name: CPU Load
├─value: 2
├─BACKLIGHT:
│ ├─red: 50
│ ├─green: 50
│ └─blue: 50
└─image: img_blank

DIAL: 5B0067000650564139323920
├─name: Memory Usage
├─value: 29
├─BACKLIGHT:
│ ├─red: 50
│ ├─green: 50
│ └─blue: 50
└─image: img_blank

DIAL: 320042000650564139323920
├─name: CPU Temperature
├─value: 43
├─BACKLIGHT:
│ ├─red: 50
│ ├─green: 50
│ └─blue: 50
└─image: img_blank

DIAL: 07004D000650564139323920
├─name: Swap Usage
├─value: 3
├─BACKLIGHT:
│ ├─red: 50
│ ├─green: 50
│ └─blue: 50
└─image: img_blank
```
