//!     Parts common to both server and generator sides
mod credentials;
mod fcgisocketsetup;
mod minifcgi;
mod uploadedregioninfo;
mod impostorinfo;
mod testlogger;

pub use credentials::Credentials;
pub use fcgisocketsetup::init_fcgi;
pub use minifcgi::{Handler, Request, Response, run};
pub use uploadedregioninfo::{UploadedRegionInfo, HeightField};
pub use uploadedregioninfo::{elev_min_max_to_scale_offset, elev_to_u8, u8_to_elev};
pub use impostorinfo::{RegionImpostorReply, RegionImpostorData, RegionImpostorLod};
pub use testlogger::{test_logger};
