#[cfg(feature = "python")]
mod python_support;
mod rust_support;

#[cfg(feature = "python")]
pub use python_support::PyTypeSupport;
pub use rust_support::TypeSupport;
