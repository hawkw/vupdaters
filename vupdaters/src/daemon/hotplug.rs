use futures::stream::StreamExt;
use miette::{Context, IntoDiagnostic};
use std::convert::TryInto;
use tokio_udev::{AsyncMonitorSocket, EventType, MonitorBuilder};
use zbus_systemd::{systemd1, zbus};

const VU_SERVER_UNIT: &str = "VU-Server.service";

#[tracing::instrument(level = tracing::Level::INFO, name = "hotplug")]
pub(crate) async fn run() -> miette::Result<()> {
    let dbus_conn = zbus::Connection::system()
        .await
        .into_diagnostic()
        .context("failed to connect to dbus")?;
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
                .restart_unit(VU_SERVER_UNIT.to_string(), "replace".to_string())
                .await
                .into_diagnostic()
                .context("failed to restart VU-Server unit")?;
            tracing::info!("VU-Server unit restarted successfully");
        }
    }

    Ok(())
}
