use super::HotplugSettings;
use futures::stream::StreamExt;
use miette::{Context, IntoDiagnostic};
use std::convert::TryInto;
use tokio_udev::{AsyncMonitorSocket, EventType, MonitorBuilder};
use zbus_systemd::{systemd1, zbus};

#[tracing::instrument(
    level = tracing::Level::INFO,
    name = "hotplug", skip(settings),
    fields(hotplug_service = %settings.hotplug_service),
    err(Display),)]
pub(crate) async fn run(settings: HotplugSettings) -> miette::Result<()> {
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
        .match_subsystem("usb-serial")
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
        tracing::debug!(
            event_type = %event.event_type(),
            event.device = %device.syspath().display(),
            "saw a hotplug event",
        );

        if event.event_type() == EventType::Bind {
            tracing::info!(
                device.syspath = %device.syspath().display(),
                "USB-serial device bound, trying to restart VU-Server..."
            );
            manager
                .restart_unit(hotplug_service.clone(), "replace".to_string())
                .await
                .into_diagnostic()
                .context("failed to restart VU-Server unit")?;
            tracing::info!("VU-Server unit restarted successfully");
        }
    }

    tracing::info!("hotplug event stream ended");

    Ok(())
}
