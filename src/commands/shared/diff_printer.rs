use std::cell::RefMut;
use std::fmt::Write as _;
use std::io::Write;

use colored::Colorize;
use lazy_static::lazy_static;

            write!(oid_range, "{:o}", a.mode.unwrap()).unwrap();