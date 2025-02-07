use crate::expr::Expr;

pub enum Statement {
	Expression { expression: Expr },
	Print { expression:Expr },
	Var { name: Token, initializer: Expr },
}