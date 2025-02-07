use std::collections::HashMap;
use crate::expr::StractValue;

pub struct Environment {
	values: HashMap<String, StractValue>,
}

impl Environment {
	pub fn new() -> Self {
		Self {
			values: HashMap::new()
		}
	}

	pub fn define(&mut self, name: String, value: StractValue)
	{
		self.values.insert(name, value);
	}

	pub fn get(&self, name: String) -> Option<&StractValue> {
		self.values.get(name)
	}
}