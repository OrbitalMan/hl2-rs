//! The developer console's parts every app shares, following the Source SDK's `CConsolePanel`
//! (`vgui2/vgui_controls/consoledialog.cpp`): the commands and cvars an app registers and runs
//! ([`command`]), their completion ([`complete`]) and the input line with its history
//! ([`editor`]). The app supplies the command table, the context its commands run on and the
//! names their arguments complete from.
pub mod command;
pub mod complete;
pub mod editor;
