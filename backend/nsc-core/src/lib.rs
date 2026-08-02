//! `nsc-core` — the cryptographic layer.
//!
//! Phase 0 of the roadmap: own ring arithmetic, own NTT, RLWE, and textbook BFV.
//! No compiler, no noise analysis, no optimisation. The deliverable of this
//! phase is not the scheme — it is the **depth cliff**: a program that computes
//! correctly at low multiplicative depth and silently returns garbage at high
//! depth, with no error raised anywhere.
//!
//! Build the bug before building the thing that prevents it. Everything the
//! later phases add gets tested against that one artefact.

pub mod modulus;
pub mod ntt;
pub mod ring;

pub use modulus::Modulus;