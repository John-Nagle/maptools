//!     Parts common to both server and generator sides
mod credentials;
mod fcgisocketsetup;
mod minifcgi;
mod uploadedregioninfo;

pub use credentials::Credentials;
pub use fcgisocketsetup::init_fcgi;
pub use minifcgi::{Handler, Request, Response, run};
pub use uploadedregioninfo::{UploadedRegionInfo, ElevsJson, HeightField};
pub use uploadedregioninfo::{elev_min_max_to_scale_offset, elev_to_u8, u8_to_elev};
