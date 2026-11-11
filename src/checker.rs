use std::collections::HashMap;

use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::token::Span;

type CResult<T> = Result<T, Diagnostic>;

pub const BUILTIN_PRINTLN: &str = "println";

#[derive(Debug, Clone)]
pub struct FnSig {
    pub params: Vec<Type>,
    pub ret: Type,
}

struct Local {
    ty: Type,
    mutable: bool,
    is_param: bool,
}

struct Checker {
    functions: HashMap<String, FnSig>,
    scopes: Vec<HashMap<String, Local>>,
    current_ret: Type,
}

/// Type-checks the whole program and fills in `Expr::ty` for every expression.
pub fn check(program: &mut Program) -> CResult<()> {
    let mut checker = Checker {
        functions: HashMap::new(),
        scopes: Vec::new(),
        current_ret: Type::Void,
    };

    for func in &program.functions {
        checker.declare_function(func)?;
    }
    checker.check_main(program)?;

    for func in &mut program.functions {
        checker.check_function(func)?;
    }
    Ok(())
}

pub fn resolve_type(name: &TypeName) -> CResult<Type> {
    match name.name.as_str() {
        "i32" => Ok(Type::I32),
        "i64" => Ok(Type::I64),
        "bool" => Ok(Type::Bool),
        "str" => Err(Diagnostic::new(
            "`str` can't be used as a type yet; string literals only work as `println` arguments",
            name.span,
        )),
        other => Err(Diagnostic::new(
            format!("unknown type `{other}`"),
            name.span,
        )),
    }
}

fn mismatch(expected: Type, found: Type, span: Span) -> Diagnostic {
    Diagnostic::new(
        format!("mismatched types: expected `{expected}`, found `{found}`"),
        span,
    )
}

fn is_int_literal(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::Int(_) => true,
        ExprKind::Unary {
            op: UnaryOp::Neg,
            operand,
        } => matches!(operand.kind, ExprKind::Int(_)),
        _ => false,
    }
}

/// True if every path through the block ends in `return`.
fn block_returns(block: &Block) -> bool {
    block.stmts.iter().any(|stmt| match &stmt.kind {
        StmtKind::Return(_) => true,
        StmtKind::Block(inner) => block_returns(inner),
        StmtKind::If {
            then_block,
            else_block: Some(else_block),
            ..
        } => block_returns(then_block) && block_returns(else_block),
        _ => false,
    })
}

impl Checker {
    fn declare_function(&mut self, func: &Function) -> CResult<()> {
        if func.name == BUILTIN_PRINTLN {
            return Err(Diagnostic::new(
                format!("`{BUILTIN_PRINTLN}` is a built-in function and can't be redefined"),
                func.name_span,
            ));
        }
        if self.functions.contains_key(&func.name) {
            return Err(Diagnostic::new(
                format!("function `{}` is defined more than once", func.name),
                func.name_span,
            ));
        }

        let params = func
            .params
            .iter()
            .map(|p| resolve_type(&p.ty))
            .collect::<CResult<Vec<_>>>()?;
        let ret = match &func.ret {
            Some(ty) => resolve_type(ty)?,
            None => Type::Void,
        };
        self.functions
            .insert(func.name.clone(), FnSig { params, ret });
        Ok(())
    }

    fn check_main(&self, program: &Program) -> CResult<()> {
        let Some(main) = program.functions.iter().find(|f| f.name == "main") else {
            return Err(Diagnostic::new(
                "no `main` function found",
                Span { start: 0, end: 0 },
            ));
        };
        if !main.params.is_empty() {
            return Err(Diagnostic::new(
                "`main` can't take parameters",
                main.name_span,
            ));
        }
        let ret = self.functions["main"].ret;
        if ret != Type::I32 && ret != Type::Void {
            return Err(Diagnostic::new(
                format!("`main` must return `i32` or nothing, not `{ret}`"),
                main.name_span,
            ));
        }
        Ok(())
    }

    fn check_function(&mut self, func: &mut Function) -> CResult<()> {
        let sig = self.functions[&func.name].clone();
        self.current_ret = sig.ret;

        let mut params = HashMap::new();
        for (param, ty) in func.params.iter().zip(&sig.params) {
            let local = Local {
                ty: *ty,
                mutable: false,
                is_param: true,
            };
            if params.insert(param.name.clone(), local).is_some() {
                return Err(Diagnostic::new(
                    format!("parameter `{}` is declared more than once", param.name),
                    param.span,
                ));
            }
        }

        self.scopes.push(params);
        self.check_block(&mut func.body)?;
        self.scopes.pop();

        if sig.ret != Type::Void && !block_returns(&func.body) {
            return Err(Diagnostic::new(
                format!(
                    "not all paths in `{}` return a value of type `{}`",
                    func.name, sig.ret
                ),
                func.name_span,
            ));
        }
        Ok(())
    }

    fn lookup(&self, name: &str) -> Option<&Local> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    fn check_block(&mut self, block: &mut Block) -> CResult<()> {
        self.scopes.push(HashMap::new());
        let result = block
            .stmts
            .iter_mut()
            .try_for_each(|stmt| self.check_stmt(stmt));
        self.scopes.pop();
        result
    }

    fn check_stmt(&mut self, stmt: &mut Stmt) -> CResult<()> {
        match &mut stmt.kind {
            StmtKind::Let {
                name,
                mutable,
                ty,
                value,
            } => {
                let declared = ty.as_ref().map(resolve_type).transpose()?;
                let found = self.check_expr(value, declared)?;
                match (declared, found) {
                    (_, Type::Str) => {
                        return Err(Diagnostic::new(
                            "string literals can only be passed to `println` for now",
                            value.span,
                        ));
                    }
                    (_, Type::Void) => {
                        return Err(Diagnostic::new(
                            "this expression doesn't produce a value",
                            value.span,
                        ));
                    }
                    (Some(declared), found) if declared != found => {
                        return Err(mismatch(declared, found, value.span));
                    }
                    _ => {}
                }
                let local = Local {
                    ty: declared.unwrap_or(found),
                    mutable: *mutable,
                    is_param: false,
                };
                // Shadowing a name in the same scope is allowed, like in Rust.
                self.scopes.last_mut().unwrap().insert(name.clone(), local);
            }
            StmtKind::Assign {
                name,
                name_span,
                value,
            } => {
                let Some(local) = self.lookup(name) else {
                    return Err(Diagnostic::new(
                        format!("cannot find variable `{name}`"),
                        *name_span,
                    ));
                };
                if !local.mutable {
                    let message = if local.is_param {
                        format!("cannot assign to parameter `{name}`; copy it with `let mut` first")
                    } else {
                        format!(
                            "cannot assign twice to immutable variable `{name}`; declare it with `let mut`"
                        )
                    };
                    return Err(Diagnostic::new(message, *name_span));
                }
                let target = local.ty;
                self.expect_expr(value, target)?;
            }
            StmtKind::If {
                cond,
                then_block,
                else_block,
            } => {
                self.expect_expr(cond, Type::Bool)?;
                self.check_block(then_block)?;
                if let Some(else_block) = else_block {
                    self.check_block(else_block)?;
                }
            }
            StmtKind::While { cond, body } => {
                self.expect_expr(cond, Type::Bool)?;
                self.check_block(body)?;
            }
            StmtKind::Return(value) => match (value, self.current_ret) {
                (None, Type::Void) => {}
                (None, ret) => {
                    return Err(Diagnostic::new(
                        format!("expected a return value of type `{ret}`"),
                        stmt.span,
                    ));
                }
                (Some(value), Type::Void) => {
                    return Err(Diagnostic::new(
                        "this function doesn't return a value",
                        value.span,
                    ));
                }
                (Some(value), ret) => self.expect_expr(value, ret)?,
            },
            StmtKind::Block(block) => self.check_block(block)?,
            StmtKind::Expr(expr) => {
                self.check_expr(expr, None)?;
            }
        }
        Ok(())
    }

    fn expect_expr(&mut self, expr: &mut Expr, expected: Type) -> CResult<()> {
        let found = self.check_expr(expr, Some(expected))?;
        if found != expected {
            return Err(mismatch(expected, found, expr.span));
        }
        Ok(())
    }

    /// `expected` is only a hint; it lets integer literals become `i64` when the context needs it.
    fn check_expr(&mut self, expr: &mut Expr, expected: Option<Type>) -> CResult<Type> {
        let ty = match &mut expr.kind {
            ExprKind::Int(value) => self.int_literal_type(*value, expected, expr.span)?,
            ExprKind::Bool(_) => Type::Bool,
            ExprKind::Str(_) => Type::Str,
            ExprKind::Var(name) => match self.lookup(name) {
                Some(local) => local.ty,
                None if self.functions.contains_key(name.as_str()) => {
                    return Err(Diagnostic::new(
                        format!("`{name}` is a function; call it with `{name}(...)`"),
                        expr.span,
                    ));
                }
                None => {
                    return Err(Diagnostic::new(
                        format!("cannot find variable `{name}`"),
                        expr.span,
                    ));
                }
            },
            ExprKind::Unary { op, operand } => self.check_unary(*op, operand, expected)?,
            ExprKind::Binary { op, lhs, rhs } => {
                self.check_binary(*op, lhs, rhs, expected, expr.span)?
            }
            ExprKind::Call { callee, args } => self.check_call(callee, args, expr.span)?,
        };
        expr.ty = Some(ty);
        Ok(ty)
    }

    fn int_literal_type(&self, value: i64, expected: Option<Type>, span: Span) -> CResult<Type> {
        let ty = match expected {
            Some(Type::I64) => Type::I64,
            _ => Type::I32,
        };
        if ty == Type::I32 && i32::try_from(value).is_err() {
            return Err(Diagnostic::new(
                format!("literal `{value}` doesn't fit in `i32`; annotate the variable as `i64`"),
                span,
            ));
        }
        Ok(ty)
    }

    fn check_unary(
        &mut self,
        op: UnaryOp,
        operand: &mut Expr,
        expected: Option<Type>,
    ) -> CResult<Type> {
        match op {
            UnaryOp::Neg => {
                // `-2147483648` is a valid i32 even though `2147483648` alone isn't.
                if let ExprKind::Int(value) = operand.kind {
                    let ty = match expected {
                        Some(Type::I64) => Type::I64,
                        _ => Type::I32,
                    };
                    if ty == Type::I32 && i32::try_from(-value).is_err() {
                        return Err(Diagnostic::new(
                            format!("literal `-{value}` doesn't fit in `i32`"),
                            operand.span,
                        ));
                    }
                    operand.ty = Some(ty);
                    return Ok(ty);
                }

                let ty = self.check_expr(operand, expected)?;
                if !ty.is_integer() {
                    return Err(Diagnostic::new(
                        format!("cannot negate a value of type `{ty}`"),
                        operand.span,
                    ));
                }
                Ok(ty)
            }
            UnaryOp::Not => {
                let ty = self.check_expr(operand, Some(Type::Bool))?;
                if ty != Type::Bool {
                    return Err(Diagnostic::new(
                        format!("`!` expects `bool`, found `{ty}`"),
                        operand.span,
                    ));
                }
                Ok(Type::Bool)
            }
        }
    }

    fn check_binary(
        &mut self,
        op: BinaryOp,
        lhs: &mut Expr,
        rhs: &mut Expr,
        expected: Option<Type>,
        span: Span,
    ) -> CResult<Type> {
        if matches!(op, BinaryOp::And | BinaryOp::Or) {
            for side in [lhs, rhs] {
                let ty = self.check_expr(side, Some(Type::Bool))?;
                if ty != Type::Bool {
                    return Err(Diagnostic::new(
                        format!("`{}` expects `bool`, found `{ty}`", op.symbol()),
                        side.span,
                    ));
                }
            }
            return Ok(Type::Bool);
        }

        let hint = if op.is_arithmetic() { expected } else { None };

        // Type the non-literal side first so that in `1 + x` the literal picks up x's type.
        let (lhs_ty, rhs_ty) = if is_int_literal(lhs) && !is_int_literal(rhs) {
            let rhs_ty = self.check_expr(rhs, hint)?;
            let lhs_ty = self.check_expr(lhs, Some(rhs_ty))?;
            (lhs_ty, rhs_ty)
        } else {
            let lhs_ty = self.check_expr(lhs, hint)?;
            let rhs_ty = self.check_expr(rhs, Some(lhs_ty))?;
            (lhs_ty, rhs_ty)
        };

        let allowed = |ty: Type| {
            ty.is_integer() || (ty == Type::Bool && matches!(op, BinaryOp::Eq | BinaryOp::Ne))
        };
        for (ty, side) in [(lhs_ty, &*lhs), (rhs_ty, &*rhs)] {
            if !allowed(ty) {
                return Err(Diagnostic::new(
                    format!("cannot apply `{}` to a value of type `{ty}`", op.symbol()),
                    side.span,
                ));
            }
        }
        if lhs_ty != rhs_ty {
            return Err(Diagnostic::new(
                format!("mismatched types: `{lhs_ty}` {} `{rhs_ty}`", op.symbol()),
                span,
            ));
        }

        Ok(if op.is_comparison() {
            Type::Bool
        } else {
            lhs_ty
        })
    }

    fn check_call(&mut self, callee: &str, args: &mut [Expr], span: Span) -> CResult<Type> {
        if callee == BUILTIN_PRINTLN {
            let [arg] = args else {
                return Err(Diagnostic::new(
                    format!("`println` takes 1 argument but {} were given", args.len()),
                    span,
                ));
            };
            let ty = self.check_expr(arg, None)?;
            if ty == Type::Void {
                return Err(Diagnostic::new(
                    "this expression doesn't produce a value",
                    arg.span,
                ));
            }
            return Ok(Type::Void);
        }

        let Some(sig) = self.functions.get(callee).cloned() else {
            return Err(Diagnostic::new(
                format!("cannot find function `{callee}`"),
                span,
            ));
        };
        if args.len() != sig.params.len() {
            let plural = if sig.params.len() == 1 { "" } else { "s" };
            return Err(Diagnostic::new(
                format!(
                    "`{callee}` takes {} argument{plural} but {} were given",
                    sig.params.len(),
                    args.len()
                ),
                span,
            ));
        }
        for (arg, param_ty) in args.iter_mut().zip(&sig.params) {
            self.expect_expr(arg, *param_ty)?;
        }
        Ok(sig.ret)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::parse;

    fn check_src(src: &str) -> CResult<Program> {
        let mut program = parse(Lexer::new(src).tokenize().unwrap()).unwrap();
        check(&mut program)?;
        Ok(program)
    }

    fn error_of(src: &str) -> String {
        check_src(src).unwrap_err().message
    }

    #[test]
    fn valid_program() {
        check_src(
            "fn fib(n: i32) -> i32 {
                if n < 2 { return n; }
                return fib(n - 1) + fib(n - 2);
            }
            fn main() -> i32 {
                let mut i = 0;
                let big: i64 = 5_000_000_000;
                let neg = -2147483648;
                while i < 10 && !false { i = i + 1; }
                println(\"done\");
                println(big * 2);
                println(neg == 0);
                return fib(i);
            }",
        )
        .unwrap();
    }

    #[test]
    fn literals_take_i64_from_context() {
        let program = check_src(
            "fn main() {
                let x: i64 = 1;
                let y = 2 + x;
            }",
        )
        .unwrap();
        let StmtKind::Let { value, .. } = &program.functions[0].body.stmts[1].kind else {
            panic!();
        };
        assert_eq!(value.ty, Some(Type::I64));
    }

    #[test]
    fn immutable_assignment() {
        assert_eq!(
            error_of("fn main() { let x = 1; x = 2; }"),
            "cannot assign twice to immutable variable `x`; declare it with `let mut`"
        );
        assert!(error_of("fn f(a: i32) { a = 2; } fn main() {}").contains("parameter `a`"));
    }

    #[test]
    fn type_mismatches() {
        assert_eq!(
            error_of("fn main() { let x: i32 = true; }"),
            "mismatched types: expected `i32`, found `bool`"
        );
        assert_eq!(
            error_of("fn main() { let a: i64 = 1; let b: i32 = 2; let c = a + b; }"),
            "mismatched types: `i64` + `i32`"
        );
        assert_eq!(
            error_of("fn main() { if 1 { } }"),
            "mismatched types: expected `bool`, found `i32`"
        );
        assert!(error_of("fn main() { let x = true + 1; }").contains("cannot apply `+`"));
    }

    #[test]
    fn literal_range() {
        assert!(error_of("fn main() { let x = 3_000_000_000; }").contains("doesn't fit in `i32`"));
        assert!(error_of("fn main() { let x = -2147483649; }").contains("doesn't fit in `i32`"));
    }

    #[test]
    fn missing_return() {
        assert_eq!(
            error_of("fn f(x: i32) -> i32 { if x > 0 { return 1; } } fn main() {}"),
            "not all paths in `f` return a value of type `i32`"
        );
        check_src("fn f(x: i32) -> i32 { if x > 0 { return 1; } else { return 2; } } fn main() {}")
            .unwrap();
    }

    #[test]
    fn calls() {
        assert_eq!(
            error_of("fn main() { foo(); }"),
            "cannot find function `foo`"
        );
        assert_eq!(
            error_of("fn f(a: i32) {} fn main() { f(1, 2); }"),
            "`f` takes 1 argument but 2 were given"
        );
        assert!(
            error_of("fn f() {} fn main() { let x = f(); }").contains("doesn't produce a value")
        );
    }

    #[test]
    fn scopes() {
        assert_eq!(
            error_of("fn main() { { let x = 1; } let y = x; }"),
            "cannot find variable `x`"
        );
        check_src("fn main() { let x = 1; let x = x == 1; if x { } }").unwrap();
    }

    #[test]
    fn strings_only_in_println() {
        assert!(error_of("fn main() { let s = \"hi\"; }").contains("only be passed to `println`"));
    }

    #[test]
    fn main_rules() {
        assert_eq!(error_of("fn foo() {}"), "no `main` function found");
        assert!(error_of("fn main() -> bool { return true; }").contains("must return `i32`"));
    }
}
