/// SSH-backed platform probing for remote onboarding.
pub(crate) mod detect;
pub(crate) mod transport;

#[cfg(test)]
mod detect_ut;
#[cfg(test)]
mod transport_ut;
