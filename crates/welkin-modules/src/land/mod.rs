// SPDX-License-Identifier: GPL-3.0-or-later

//! Land-management subsystem: data structures, parcel grid, management
//! module, prim-count tracking, and dwell scoring.

pub mod data;
pub mod dwell;
pub mod grid;
pub mod management;
pub mod parcel;
pub mod prim_count;

pub use data::*;
pub use dwell::DwellModule;
pub use grid::{LAND_UNIT, ParcelGrid};
pub use management::LandManagementModule;
pub use parcel::LandParcel;
pub use prim_count::{PrimCountModule, PrimCounts};
