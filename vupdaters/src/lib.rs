#![doc = include_str!("../README.md")]
pub mod cli;
pub mod daemon;
pub mod dialctl;

#[derive(miette::Diagnostic, Debug, thiserror::Error)]
#[error("{}", .msg)]
#[diagnostic()]
pub(crate) struct MultiError {
    msg: &'static str,
    #[related]
    errors: Vec<miette::Report>,
    max_errors: Option<usize>,
}

impl MultiError {
    pub(crate) fn with_max_errors(msg: &'static str, max_errors: usize) -> Self {
        Self {
            msg,
            errors: Vec::with_capacity(max_errors),
            max_errors: Some(max_errors),
        }
    }

    pub(crate) fn push_error(&mut self, error: impl Into<miette::Report>) -> Result<(), Self> {
        self.errors.push(error.into());
        if let Some(max_errors) = self.max_errors {
            if self.errors.len() >= max_errors {
                return Err(Self {
                    msg: self.msg,
                    errors: std::mem::take(&mut self.errors),
                    max_errors: None,
                });
            }
        }

        Ok(())
    }

    pub(crate) fn clear(&mut self) {
        self.errors.clear();
    }

    pub(crate) fn from_vec(errors: Vec<miette::Report>, msg: &'static str) -> miette::Result<()> {
        if errors.is_empty() {
            return Ok(());
        }

        if errors.len() == 1 {
            return Err(errors.into_iter().next().unwrap());
        }

        Err(MultiError {
            msg,
            errors,
            max_errors: None,
        }
        .into())
    }
}
