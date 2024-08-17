pub mod fetch;
pub mod parse;

use std::collections::HashMap;

pub type PairInfo = HashMap<(char, char), usize>;

pub type LengthInfo = HashMap<(char, usize), usize>;
