pub mod local;
pub mod manifest;
pub mod profile;
pub mod scaffold;

pub use local::{LocalConfig, discover_repo};
pub use manifest::{
    AnvilMeta, Firewall, Harden, Hooks, Link, Manifest, Packages, Profile, SshHarden, SysctlEntry,
};
pub use profile::{ResolvedProfile, resolve_profiles, select_profile_names};
pub use scaffold::write_starter_manifest;
