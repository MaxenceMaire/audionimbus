/*!
FMOD Studio integration.
*/

#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    rustdoc::broken_intra_doc_links
)]

use crate::phonon::*;

include!(concat!(env!("OUT_DIR"), "/phonon_fmod.rs"));
