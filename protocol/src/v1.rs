#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum HubCommand {
    // === set values ===
    SetDialRawSingle = 0x01,
    SetDialRawMultiple = 0x02,
    SetDialPercSingle = 0x03,
    SetDialPercMultiple = 0x04,
    SetDialCalibrateMax = 0x05,
    SetDialCalibrateHalf = 0x06,

    // hub commands ===
    GetDevicesMap = 0x07,
    ProvisionDevice = 0x08,
    ResetAllDevices = 0x09,
    DialPower = 0x0A,
    GetDeviceUid = 0x0B,
    RescanBus = 0x0C,

    // === display commands ===
    DisplayClear = 0x0D,
    DisplayGotoXy = 0x0E,
    DisplayImgData = 0x0F,
    DisplayShowImg = 0x10,
    RxBufferSize = 0x11,
    ResetCfg = 0x12,
    SetRgbBacklight = 0x13,

    // === easing ===
    SetDialEasingStep = 0x14,
    SetDialEasingPeriod = 0x15,
    SetBacklightEasingStep = 0x16,
    SetBacklightEasingPeriod = 0x17,
    GetEasingConfig = 0x18,

    // === get metadata ===
    GetBuildInfo = 0x19,
    GetFwInfo = 0x20,
    GetHwInfo = 0x21,
    GetProtocolInfo = 0x22,
    DebugI2cScan = 0xF3,

    // === bootloader commands ===
    HubBtlJumpToBootloader = 0xF4,
    DialBtlJumpToBootloader = 0xF5,
    DialBtlGetInfo = 0xF6,
    DialBtlGetCrc = 0xF7,
    DialBtlEraseApp = 0xF8,
    DialBtlFwupSendPackage = 0xF9,
    DialBtlFwupFinished = 0xFA,
    DialBtlExit = 0xFB,
    DialBtlRestartFwupload = 0xFC,
    DialBtlReadLastStatus = 0xFD,
}
