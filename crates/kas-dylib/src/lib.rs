// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License in the LICENSE-APACHE file or at:
//     https://www.apache.org/licenses/LICENSE-2.0

//! KAS GUI dylib
//!
//! Using this library forces dynamic linking, which can make builds much
//! faster. It may be preferable only to use this in debug builds.

#![allow(unused_imports)]

use kas_core;
use kas_theme;
use kas_wgpu;
use kas_widgets;