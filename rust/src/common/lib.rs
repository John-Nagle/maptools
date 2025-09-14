//!     Parts common to both server and generator sides
mod credentials;
mod fcgisocketsetup;
mod minifcgi;
mod uploadedregioninfo;

pub use credentials::Credentials;
pub use fcgisocketsetup::init_fcgi;
pub use minifcgi::{Handler, Request, Response, run};
pub use uploadedregioninfo::{UploadedRegionInfo, ElevsJson};
