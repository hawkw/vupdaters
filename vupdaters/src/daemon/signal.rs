// The `SignalAction::Reload` variant is currently only used on Linux systems.
#[cfg_attr(windows, allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SignalAction {
    // Basically the only two things we do when handling a signal:
    /// Reload the config file and restart dial managers.
    ///
    /// This action is performed on receipt of a SIGHUP on Unix systems.
    Reload,
    /// Shut down the daemon.
    Shutdown,
}

pub(super) use self::signal_impl::SignalListener;

#[cfg(unix)]
mod signal_impl {
    use super::SignalAction;
    use miette::{Context, IntoDiagnostic};
    use tokio::signal::unix::{signal, Signal, SignalKind};

    #[derive(Debug)]
    pub(crate) struct SignalListener {
        // Reload config file on SIGHUP
        sighup: Signal,
        // Shutdown on SIGINT/SIGTERM/SIGQUIT
        sigint: Signal,
        sigterm: Signal,
        sigquit: Signal,
    }

    impl SignalListener {
        pub(crate) fn new() -> miette::Result<Self> {
            let sighup = signal(SignalKind::hangup())
                .into_diagnostic()
                .context("failed to start listening for SIGHUP")?;
            let sigint = signal(SignalKind::interrupt())
                .into_diagnostic()
                .context("failed to start listening for SIGINT")?;
            let sigterm = signal(SignalKind::terminate())
                .into_diagnostic()
                .context("failed to start listening for SIGTERM")?;
            let sigquit = signal(SignalKind::quit())
                .into_diagnostic()
                .context("failed to start listening for SIGQUIT")?;

            Ok(Self {
                sighup,
                sigint,
                sigterm,
                sigquit,
            })
        }

        #[must_use]
        pub(crate) async fn next_signal(&mut self) -> SignalAction {
            tokio::select! {
                _ = self.sighup.recv() => {
                    tracing::info!("Received SIGHUP, reloading config");
                    SignalAction::Reload
                }
                _ = self.sigint.recv() => {
                    tracing::info!("Received SIGINT, shutting down");
                    SignalAction::Shutdown
                }
                _ = self.sigterm.recv() => {
                    tracing::info!("Received SIGTERM, shutting down");
                    SignalAction::Shutdown
                }
                _ = self.sigquit.recv() => {
                    tracing::info!("Received SIGQUIT, shutting down");
                    SignalAction::Shutdown
                }
            }
        }
    }
}

#[cfg(windows)]
mod signal_impl {
    use super::SignalAction;
    use miette::{Context, IntoDiagnostic};
    use tokio::signal::windows::{ctrl_c, CtrlC};

    #[derive(Debug)]
    pub(crate) struct SignalListener {
        ctrl_c: CtrlC,
        // TODO(eliza): what are the other Windows signals supposed to do?
    }

    impl SignalListener {
        pub(crate) fn new() -> miette::Result<Self> {
            let ctrl_c = ctrl_c()
                .into_diagnostic()
                .context("failed to start listening for Ctrl-C")?;

            Ok(Self { ctrl_c })
        }

        #[must_use]
        pub(crate) async fn next_signal(&mut self) -> SignalAction {
            self.ctrl_c.recv().await;
            tracing::info!("Received Ctrl-C, shutting down");
            SignalAction::Shutdown
        }
    }
}
