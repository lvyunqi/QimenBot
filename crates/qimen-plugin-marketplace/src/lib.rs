mod catalog;
mod client;
mod compatibility;
mod error;
mod install;
mod model;

pub use catalog::{load_catalog_directory, write_catalog_index};
pub use client::{CatalogLoad, CatalogSource, MarketplaceClient, SourceVerification};
pub use compatibility::{
    CompatibilityIssue, HostProfile, VersionCompatibility, compatible_versions, evaluate_version,
    select_latest_compatible,
};
pub use error::{MarketplaceError, Result};
pub use install::{
    ActiveFileTransaction, InstalledPlugin, InstalledVersion, MarketplaceLock, MarketplacePaths,
    PreparedInstall, sha256_file,
};
pub use model::{
    CatalogIndex, CatalogPlugin, DriverEventKind, DriverSupport, MessageScene, OutboundCapability,
    PluginAsset, PluginDriver, PluginKind, PluginManifest, ReleaseChannel, TrustLevel,
    VersionManifest, marketplace_asset_name,
};
