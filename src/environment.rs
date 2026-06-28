use crate::expr::{CallableImpl, LiteralValue, NativeFunctionImpl};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub struct SlotData {
	pub value: LiteralValue,
	pub hotlink_listeners: Vec<Rc<RefCell<SlotData>>>,
}

type Slot = Rc<RefCell<SlotData>>;

fn newSlot(value: LiteralValue) -> Slot {
	Rc::new(RefCell::new(SlotData { value, hotlink_listeners: vec![] }))
}

#[derive(Clone)]
pub struct Environment {
    pub values: Rc<RefCell<HashMap<String, Slot>>>,
    locals: Rc<RefCell<HashMap<usize, usize>>>,
    pub enclosing: Option<Box<Environment>>,
	pub is_production: bool,
}

#[allow(non_snake_case)]
fn clockImpl(_args: &Vec<LiteralValue>) -> LiteralValue {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .expect("Could not get system time")
        .as_millis();

    LiteralValue::Number(now as f64 / 1000.0)
}

#[allow(non_snake_case)]
fn getGlobals() -> Rc<RefCell<HashMap<String, Slot>>> {
    let mut env = HashMap::new();
    let fun_impl = NativeFunctionImpl {
        name: "clock".to_string(),
        arity: 0,
        fun: Rc::new(clockImpl),
    };
    let callable_impl = CallableImpl::NativeFunction(fun_impl);
    env.insert("clock".to_string(), newSlot(LiteralValue::Callable(callable_impl)));

    Rc::new(RefCell::new(env))
}

impl Environment {
    pub fn new(locals: HashMap<usize, usize>, is_production: bool) -> Self {
        Self {
            values: getGlobals(),
            locals: Rc::new(RefCell::new(locals)),
            enclosing: None,
			is_production,
        }
    }

    pub fn resolve(&self, locals: HashMap<usize, usize>) {
        // self.locals = locals --! Bad because it wont update enclosing
        for (key, val) in locals.iter() {
            self.locals.borrow_mut().insert(*key, *val);
        }
    }

    pub fn enclose(&self) -> Environment {
        Self {
            values: Rc::new(RefCell::new(HashMap::new())),
            locals: self.locals.clone(),
            enclosing: Some(Box::new(self.clone())),
			is_production: self.is_production,
        }
    }

    pub fn define(&self, name: String, value: LiteralValue) {
        self.values.borrow_mut().insert(name, newSlot(value));
    }

	pub fn defineAliased(&self, name: String, existing_slot: Slot) {
		self.values.borrow_mut().insert(name, existing_slot);
	}

	pub fn registerHotlink(source_slot: &Slot, listener_slot: Slot) {
		source_slot.borrow_mut().hotlink_listeners.push(listener_slot);
	}

	pub fn getSlot(&self, name: &str, distance: Option<usize>) -> Option<Slot> {
		if let None = distance {
			match &self.enclosing {
				None => self.values.borrow().get(name).cloned(),
				Some(env) => env.getSlot(name, distance),
			}
		} else {
			let distance = distance.unwrap();
			if distance == 0 {
				self.values.borrow().get(name).cloned()
			} else {
				match &self.enclosing {
					None => panic!("Tried to resolve a variable that was defined deeper than the current environment depth"),
					Some(env) => {
						assert!(distance > 0);
						env.getSlot(name, Some(distance - 1))
					}
				}
			}
		}
	}

    pub fn get(&self, name: &str, expr_id: usize) -> Option<LiteralValue> {
        let distance = self.locals.borrow().get(&expr_id).cloned();
        self.getInternal(name, distance)
    }

	#[allow(non_snake_case)]
    pub fn getThisInstance(&self, super_id: usize) -> Option<LiteralValue> {
        let distance = self
            .locals
            .borrow()
            .get(&super_id)
            .cloned()
            .expect("Could not find 'this' even though 'super' was defined");
        self.getInternal("this", Some(distance - 1))
    }

	#[allow(non_snake_case)]
    pub fn getDistance(&self, expr_id: usize) -> Option<usize> {
        self.locals.borrow().get(&expr_id).cloned()
    }

	#[allow(non_snake_case)]
    fn getInternal(&self, name: &str, distance: Option<usize>) -> Option<LiteralValue> {
        self.getSlot(name, distance).map(|slot| slot.borrow().value.clone())
    }

	#[allow(non_snake_case)]
    pub fn assignGlobal(&self, name: &str, value: LiteralValue) -> bool {
        self.assignInternal(name, value, None)
    }

    pub fn assign(&self, name: &str, value: LiteralValue, expr_id: usize) -> bool {
        // ! Important that this ID matches with the resolver
        let distance = self.locals.borrow().get(&expr_id).cloned();
        self.assignInternal(name, value, distance)
    }

	#[allow(non_snake_case)]
    fn assignInternal(&self, name: &str, value: LiteralValue, distance: Option<usize>) -> bool {
        match self.getSlot(name, distance) {
			Some(slot) => {
				Self::writeSlot(&slot, value);
				true
			}
			None => false,
		}
    }

	#[allow(non_snake_case)]
	fn writeSlot(slot: &Slot, value: LiteralValue) {
		let listeners = {
			let mut data = slot.borrow_mut();
			data.value = value.clone();
			data.hotlink_listeners.clone()
		};
		for listener in listeners {
			Self::writeSlot(&listener, value.clone());
		}
	}

    #[allow(dead_code)]
    pub fn dump(&self, indent: usize) -> String {
        let mut result = String::new();
        for (key, val) in self.values.borrow().iter() {
            for _ in 0..indent {
                result.push_str(" ");
            }
            result.push_str(&format!("{}: {:?}\n", key, val.borrow().value));
        }
        if let Some(env) = &self.enclosing {
            result.push_str(&env.dump(indent + 2));
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn try_init() {
        let _environment = Environment::new(HashMap::new());
    }
}
