pub mod fetch;
pub mod parse;
pub mod sheets;

use std::collections::HashMap;

pub type PairInfo = HashMap<(char, char), usize>;

pub type LengthInfo = HashMap<(char, usize), usize>;
