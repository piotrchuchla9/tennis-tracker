//! Rdzen rigu Tennis Tracker.
//!
//! Cala logika bezsprzetowa: synchronizacja zegarow, protokol lacza,
//! manifest sesji, maszyna stanow nagrywania.
//!
//! Zasada nadrzedna: ten crate NIGDY nie odczytuje zegara samodzielnie.
//! Czas przychodzi z zewnatrz jako parametr, dzieki czemu testy sterują
//! jego uplywem bez czekania.

pub mod api;
pub mod clock;
pub mod device;
pub mod link;
pub mod remote;
pub mod session;

uniffi::setup_scaffolding!();
