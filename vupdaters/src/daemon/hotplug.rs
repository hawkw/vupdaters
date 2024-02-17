use super::HotplugSettings;
use futures::stream::StreamExt;
use miette::{Context, IntoDiagnostic};
use std::convert::TryInto;
use tokio::sync::watch;
use tokio_udev::{AsyncMonitorSocket, EventType, MonitorBuilder};
use zbus_systemd::{systemd1, zbus};

const USB_VENDOR_ID: &str = "ID_USB_VENDOR_ID";
const USB_MODEL_ID: &str = "ID_USB_VENDOR_ID";
const DIAL_HUB_USB_VENDOR_ID: &str = "0403"; // FDTI
const DIAL_HUB_USB_MODEL_ID: &str = "6015";

#[tracing::instrument(
    level = tracing::Level::INFO,
    name = "hotplug",
    skip(settings, running),
    fields(hotplug_service = %settings.hotplug_service),
    err(Display),)]
pub(crate) async fn run(
    settings: HotplugSettings,
    running: watch::Sender<bool>,
) -> miette::Result<()> {
    let HotplugSettings {
        enabled,
        hotplug_service,
    } = settings;
    assert!(enabled, "hotplug::run should only be called if enabled");

    let dbus_conn = zbus::Connection::system()
        .await
        .into_diagnostic()
        .context("failed to connect to dbus")?;
    tracing::debug!("connected to dbus");
    let manager = systemd1::ManagerProxy::new(&dbus_conn)
        .await
        .into_diagnostic()
        .context("failed to connect to systemd")?;

    let builder = MonitorBuilder::new()
        .into_diagnostic()
        .context("failed to create `tokio_udev::MonitorBuilder`")?
        .match_subsystem("tty")
        // .match_subsystem_devtype("usb", "usb_device")
        .into_diagnostic()
        .context("failed to add udev filter for usb-serial devices")?;

    let mut monitor: AsyncMonitorSocket = builder
        .listen()
        .into_diagnostic()
        .context("failed to listen to udev events")?
        .try_into()
        .into_diagnostic()
        .context("failed to convert MonitorSocket to async")?;

    tracing::info!("starting hotplug event watcher");

    while let Some(event) = monitor.next().await {
        let event = match event {
            Ok(e) => e,
            Err(error) => {
                tracing::error!(%error, "failed to receive udev event");
                continue;
            }
        };
        let device = event.device();

        let usb_vendor = device.property_value(USB_VENDOR_ID);
        let usb_device = device.property_value(USB_MODEL_ID);
        tracing::debug!(
            event_type = %event.event_type(),
            event.device = %device.syspath().display(),
            device.usb_vendor_id = ?usb_vendor,
            device.usb_device_id = ?usb_device,
            "saw a hotplug event",
        );

        let matches = usb_vendor == Some(DIAL_HUB_USB_VENDOR_ID.as_ref())
            && usb_device != Some(DIAL_HUB_USB_MODEL_ID.as_ref());
        if !matches {
            tracing::debug!(
                "device does not match expected vendor ID ({DIAL_HUB_USB_VENDOR_ID}) \
                and model ID ({DIAL_HUB_USB_MODEL_ID}); ignoring it"
            );
            continue;
        }
        tracing::debug!("USB-serial device matches dial hub");

        let set_running = |run: bool| {
            running
                .send(run)
                .into_diagnostic()
                .context("watch channel dropped")
        };

        match event.event_type() {
            EventType::Remove => {
                tracing::info!(
                    device.syspath = %device.syspath().display(),
                    "USB-serial device removed, pausing updates"
                );
                set_running(false)?;
            }
            EventType::Add | EventType::Change => {
                tracing::info!(
                    device.syspath = %device.syspath().display(),
                    "USB-serial device added, trying to restart VU-Server..."
                );

                manager
                    .restart_unit(hotplug_service.clone(), "replace".to_string())
                    .await
                    .into_diagnostic()
                    .context("failed to restart VU-Server unit")?;

                tracing::info!("VU-Server unit restarted successfully");
                set_running(true)?;
            }
            event_type => tracing::trace!(?event_type, "unhandled udev event"),
        }
    }

    tracing::info!("hotplug event stream ended");

    Ok(())
}
