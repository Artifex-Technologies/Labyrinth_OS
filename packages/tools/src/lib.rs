//! Labyrinth OS's own programs in Rust, replacing the Python scripts of the same names (profile/airootfs/usr/local/bin) with the
//! same interface: arguments, output, files, environment variables and exit codes. Each module says what it is and where it is
//! not 1:1. Where a script would have stopped with a traceback (exit code 1), the Rust one stops too, with exit code 1 and one
//! line on standard error.
//!
//!   scan        labyrinth-scan-downloads    the malware scan of new files (always on: labyrinth-scan-watch.service)
//!   quarantine  labyrinth-quarantine        its quarantine and the way back
//!   appnet      labyrinth-appnet            per-app network rules (nftables)
//!   appnet_ask  labyrinth-appnet-ask        the ask mode's questions (always on: a root service)
//!   ask_agent   labyrinth-appnet-ask-agent  the ask mode's notifications (always on: a user service)
//!   launch      labyrinth-launch            the app launcher (dock, start menu, Settings)
//!   apps        labyrinth-apps              the optional software (Agora, the first login)
//!   selfcheck   labyrinth-selfcheck         is the desktop in a sane state (after an upgrade)
//!   ipconfig    ipconfig                    Windows' ipconfig from ip and NetworkManager
//!   playall     steam_playall               Steam Play for all titles in Steam's config.vdf (labyrinth-gaming setup)
//!   first_components  labyrinth-first-components  the optional parts chosen in the installer, at the first login
//!   agent       labyrinth-agent             the engineering agent's root actions, each checked (the gate sudo lets it run)
//!
//! The shell scripts' ports (sh: what they share):
//!   rollback         labyrinth-rollback            the whole system back to a snapshot, and back again
//!   bootfinish       labyrinth-install-bootfinish  the installer's last step in the installed system
//!   install_cleanup  labyrinth-install-cleanup     the live medium's pieces out of the installed system, the kernels back
//!   agent_setup      labyrinth-agent-setup         the engineering agent's own account and its sudo rules
//!   agent_window     labyrinth-agent-window        the owner opens the agent's broad root commands for some hours
//!   ecosystem        labyrinth-ecosystem-setup     Ollama, Sarina, Heimdall and Limbo wired up together
//!   hardware         labyrinth-hardware-setup      only the drivers and device tools this computer needs
//!   grub_theme       labyrinth-grub-theme          the boot menu theme
//!   security_setting labyrinth-security-setting    the security settings Settings asks for
//!   gaming           labyrinth-gaming              are Steam and Proton installed and working
//!   sarina           labyrinth-sarina              Sarina with a model server behind her
//!   sarina_setting   labyrinth-sarina-setting      Sarina's settings for Settings > Sarina
//!   sarina_small     labyrinth-sarina-reflect, -warm, -voice-setup
//!   rea              labyrinth-rea, labyrinth-rea-setup   REA in a sandbox, and its installation
//!   small            labyrinth-claude-code, labyrinth-install, labyrinth-notify-activate

pub mod agent;
pub mod agent_setup;
pub mod agent_window;
pub mod appnet;
pub mod appnet_ask;
pub mod apps;
pub mod ask_agent;
pub mod bootfinish;
pub mod ecosystem;
pub mod first_components;
pub mod gaming;
pub mod grub_theme;
pub mod hardware;
pub mod install_cleanup;
pub mod ipconfig;
pub mod launch;
pub mod playall;
pub mod py;
pub mod pycompat;
pub mod pyre;
pub mod quarantine;
pub mod rea;
pub mod rollback;
pub mod sarina;
pub mod sarina_setting;
pub mod sarina_small;
pub mod scan;
pub mod security_setting;
pub mod selfcheck;
pub mod sh;
pub mod small;

#[cfg(test)]
pub mod testutil;
