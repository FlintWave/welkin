// SPDX-License-Identifier: LGPL-3.0-or-later
//! LSL-to-Luau code generator.
//!
//! Walks a parsed [`ast::Script`] and emits Luau source code that the mlua
//! runtime can compile and execute.

use std::fmt::Write as _;

use crate::LslType;
use crate::ast::*;
use crate::runtime;

pub struct CodeGen {
    output: String,
    indent: usize,
}

impl CodeGen {
    pub fn new() -> Self {
        Self {
            output: String::with_capacity(4096),
            indent: 0,
        }
    }

    pub fn generate(script: &Script) -> String {
        let mut cg = Self::new();
        cg.emit_script(script);
        cg.output
    }

    fn write_indent(&mut self) {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
    }

    fn writeln(&mut self, s: &str) {
        self.write_indent();
        self.output.push_str(s);
        self.output.push('\n');
    }

    fn emit_script(&mut self, script: &Script) {
        self.output.push_str(runtime::RUNTIME_PRELUDE);
        self.output.push('\n');

        for gv in &script.globals {
            self.emit_global_var(gv);
        }
        if !script.globals.is_empty() {
            self.output.push('\n');
        }

        for func in &script.functions {
            self.emit_function(func);
            self.output.push('\n');
        }

        self.writeln("local __current_state = \"default\"");
        self.writeln("local __states = {");
        self.indent += 1;
        for state in &script.states {
            self.emit_state(state);
        }
        self.indent -= 1;
        self.writeln("}");
        self.output.push('\n');

        self.writeln("function __dispatch(event_name, ...)");
        self.indent += 1;
        self.writeln("local handler = __states[__current_state] and __states[__current_state][event_name]");
        self.writeln("if handler then handler(...) end");
        self.indent -= 1;
        self.writeln("end");
    }

    fn emit_global_var(&mut self, gv: &GlobalVar) {
        self.write_indent();
        let _ = write!(self.output, "local {} = ", gv.name);
        if let Some(init) = &gv.init {
            self.emit_expr(init);
        } else {
            self.emit_default_value(gv.ty);
        }
        self.output.push('\n');
    }

    fn emit_function(&mut self, func: &FuncDecl) {
        self.write_indent();
        let _ = write!(self.output, "local function {}(", func.name);
        self.emit_param_names(&func.params);
        self.output.push_str(")\n");
        self.indent += 1;
        self.emit_stmts(&func.body);
        self.indent -= 1;
        self.writeln("end");
    }

    fn emit_param_names(&mut self, params: &[Param]) {
        for (i, p) in params.iter().enumerate() {
            if i > 0 {
                self.output.push_str(", ");
            }
            self.output.push_str(&p.name);
        }
    }

    fn emit_state(&mut self, state: &State) {
        self.write_indent();
        let _ = write!(self.output, "[\"{}\"] = {{\n", state.name);
        self.indent += 1;
        for event in &state.events {
            self.emit_event_handler(event);
        }
        self.indent -= 1;
        self.writeln("},");
    }

    fn emit_event_handler(&mut self, event: &EventHandler) {
        self.write_indent();
        let _ = write!(self.output, "{} = function(", event.name);
        self.emit_param_names(&event.params);
        self.output.push_str(")\n");
        self.indent += 1;
        self.emit_stmts(&event.body);
        self.indent -= 1;
        self.writeln("end,");
    }

    fn emit_stmts(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            self.emit_stmt(stmt);
        }
    }

    fn emit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Block(stmts, _) => {
                self.writeln("do");
                self.indent += 1;
                self.emit_stmts(stmts);
                self.indent -= 1;
                self.writeln("end");
            }
            Stmt::VarDecl { ty, name, init, .. } => {
                self.write_indent();
                let _ = write!(self.output, "local {} = ", name);
                if let Some(init) = init {
                    self.emit_expr(init);
                } else {
                    self.emit_default_value(*ty);
                }
                self.output.push('\n');
            }
            Stmt::If { cond, then_body, else_body, .. } => {
                self.write_indent();
                self.output.push_str("if (");
                self.emit_expr(cond);
                self.output.push_str(") ~= 0 then\n");
                self.indent += 1;
                self.emit_stmt(then_body);
                self.indent -= 1;
                if let Some(else_body) = else_body {
                    self.writeln("else");
                    self.indent += 1;
                    self.emit_stmt(else_body);
                    self.indent -= 1;
                }
                self.writeln("end");
            }
            Stmt::For { init, cond, update, body, .. } => {
                for expr in init {
                    self.write_indent();
                    self.emit_expr(expr);
                    self.output.push('\n');
                }
                self.write_indent();
                self.output.push_str("while ");
                if let Some(cond) = cond {
                    self.output.push('(');
                    self.emit_expr(cond);
                    self.output.push_str(") ~= 0");
                } else {
                    self.output.push_str("true");
                }
                self.output.push_str(" do\n");
                self.indent += 1;
                self.emit_stmt(body);
                for expr in update {
                    self.write_indent();
                    self.emit_expr(expr);
                    self.output.push('\n');
                }
                self.indent -= 1;
                self.writeln("end");
            }
            Stmt::While { cond, body, .. } => {
                self.write_indent();
                self.output.push_str("while (");
                self.emit_expr(cond);
                self.output.push_str(") ~= 0 do\n");
                self.indent += 1;
                self.emit_stmt(body);
                self.indent -= 1;
                self.writeln("end");
            }
            Stmt::DoWhile { body, cond, .. } => {
                self.writeln("repeat");
                self.indent += 1;
                self.emit_stmt(body);
                self.indent -= 1;
                self.write_indent();
                self.output.push_str("until (");
                self.emit_expr(cond);
                self.output.push_str(") == 0\n");
            }
            Stmt::Jump { label, .. } => {
                self.write_indent();
                let _ = write!(self.output, "goto __label_{}\n", label);
            }
            Stmt::Label { name, .. } => {
                self.write_indent();
                let _ = write!(self.output, "::__label_{}::\n", name);
            }
            Stmt::Return { value, .. } => {
                self.write_indent();
                if let Some(val) = value {
                    self.output.push_str("return ");
                    self.emit_expr(val);
                    self.output.push('\n');
                } else {
                    self.output.push_str("return\n");
                }
            }
            Stmt::StateChange { target, .. } => {
                self.writeln("do");
                self.indent += 1;
                self.writeln("local __old_state = __current_state");
                self.write_indent();
                let _ = write!(self.output, "__current_state = \"{}\"\n", target);
                self.writeln("if __states[__old_state] and __states[__old_state].state_exit then");
                self.indent += 1;
                self.writeln("__states[__old_state].state_exit()");
                self.indent -= 1;
                self.writeln("end");
                self.writeln("if __states[__current_state] and __states[__current_state].state_entry then");
                self.indent += 1;
                self.writeln("__states[__current_state].state_entry()");
                self.indent -= 1;
                self.writeln("end");
                self.indent -= 1;
                self.writeln("end");
            }
            Stmt::Expression(expr, _) => {
                self.write_indent();
                self.emit_expr(expr);
                self.output.push('\n');
            }
            Stmt::Empty(_) => {}
        }
    }

    fn emit_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::IntegerLit(v, _) => {
                let _ = write!(self.output, "{}", v);
            }
            Expr::FloatLit(v, _) => {
                if v.fract() == 0.0 && v.is_finite() {
                    let _ = write!(self.output, "{:.1}", v);
                } else {
                    let _ = write!(self.output, "{}", v);
                }
            }
            Expr::StringLit(s, _) => {
                self.emit_luau_string(s);
            }
            Expr::Ident(name, _) => {
                self.output.push_str(name);
            }
            Expr::VectorLit { x, y, z, .. } => {
                self.output.push_str("{x = ");
                self.emit_expr(x);
                self.output.push_str(", y = ");
                self.emit_expr(y);
                self.output.push_str(", z = ");
                self.emit_expr(z);
                self.output.push('}');
            }
            Expr::RotationLit { x, y, z, s, .. } => {
                self.output.push_str("{x = ");
                self.emit_expr(x);
                self.output.push_str(", y = ");
                self.emit_expr(y);
                self.output.push_str(", z = ");
                self.emit_expr(z);
                self.output.push_str(", s = ");
                self.emit_expr(s);
                self.output.push('}');
            }
            Expr::ListLit(items, _) => {
                self.output.push('{');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.emit_expr(item);
                }
                self.output.push('}');
            }
            Expr::BinaryOp { op, lhs, rhs, .. } => {
                self.emit_binary_op(*op, lhs, rhs);
            }
            Expr::UnaryOp { op, operand, .. } => {
                self.emit_unary_op(*op, operand);
            }
            Expr::PostfixOp { op, operand, .. } => {
                self.emit_postfix_op(*op, operand);
            }
            Expr::Assignment { op, target, value, .. } => {
                self.emit_assignment(*op, target, value);
            }
            Expr::FuncCall { name, args, .. } => {
                self.output.push_str(name);
                self.output.push('(');
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.emit_expr(arg);
                }
                self.output.push(')');
            }
            Expr::MemberAccess { object, member, .. } => {
                self.emit_expr(object);
                self.output.push('.');
                self.output.push_str(member);
            }
            Expr::TypeCast { target_type, expr, .. } => {
                self.emit_type_cast(*target_type, expr);
            }
            Expr::Paren(inner, _) => {
                self.output.push('(');
                self.emit_expr(inner);
                self.output.push(')');
            }
        }
    }

    fn emit_binary_op(&mut self, op: BinOp, lhs: &Expr, rhs: &Expr) {
        match op {
            BinOp::Add => {
                self.output.push('(');
                self.emit_expr(lhs);
                self.output.push_str(" + ");
                self.emit_expr(rhs);
                self.output.push(')');
            }
            BinOp::Concat => {
                self.output.push('(');
                self.emit_expr(lhs);
                self.output.push_str(" .. ");
                self.emit_expr(rhs);
                self.output.push(')');
            }
            BinOp::Sub => {
                self.output.push('(');
                self.emit_expr(lhs);
                self.output.push_str(" - ");
                self.emit_expr(rhs);
                self.output.push(')');
            }
            BinOp::Mul => {
                self.output.push('(');
                self.emit_expr(lhs);
                self.output.push_str(" * ");
                self.emit_expr(rhs);
                self.output.push(')');
            }
            BinOp::Div => {
                self.output.push('(');
                self.emit_expr(lhs);
                self.output.push_str(" / ");
                self.emit_expr(rhs);
                self.output.push(')');
            }
            BinOp::Mod => {
                self.output.push('(');
                self.emit_expr(lhs);
                self.output.push_str(" % ");
                self.emit_expr(rhs);
                self.output.push(')');
            }
            BinOp::Eq => {
                self.output.push_str("((");
                self.emit_expr(lhs);
                self.output.push_str(" == ");
                self.emit_expr(rhs);
                self.output.push_str(") and 1 or 0)");
            }
            BinOp::Ne => {
                self.output.push_str("((");
                self.emit_expr(lhs);
                self.output.push_str(" ~= ");
                self.emit_expr(rhs);
                self.output.push_str(") and 1 or 0)");
            }
            BinOp::Lt => {
                self.output.push_str("((");
                self.emit_expr(lhs);
                self.output.push_str(" < ");
                self.emit_expr(rhs);
                self.output.push_str(") and 1 or 0)");
            }
            BinOp::Gt => {
                self.output.push_str("((");
                self.emit_expr(lhs);
                self.output.push_str(" > ");
                self.emit_expr(rhs);
                self.output.push_str(") and 1 or 0)");
            }
            BinOp::Le => {
                self.output.push_str("((");
                self.emit_expr(lhs);
                self.output.push_str(" <= ");
                self.emit_expr(rhs);
                self.output.push_str(") and 1 or 0)");
            }
            BinOp::Ge => {
                self.output.push_str("((");
                self.emit_expr(lhs);
                self.output.push_str(" >= ");
                self.emit_expr(rhs);
                self.output.push_str(") and 1 or 0)");
            }
            BinOp::LogicAnd => {
                self.output.push_str("(((");
                self.emit_expr(lhs);
                self.output.push_str(") ~= 0 and (");
                self.emit_expr(rhs);
                self.output.push_str(") ~= 0) and 1 or 0)");
            }
            BinOp::LogicOr => {
                self.output.push_str("(((");
                self.emit_expr(lhs);
                self.output.push_str(") ~= 0 or (");
                self.emit_expr(rhs);
                self.output.push_str(") ~= 0) and 1 or 0)");
            }
            BinOp::BitAnd => {
                self.output.push_str("bit32.band(");
                self.emit_expr(lhs);
                self.output.push_str(", ");
                self.emit_expr(rhs);
                self.output.push(')');
            }
            BinOp::BitOr => {
                self.output.push_str("bit32.bor(");
                self.emit_expr(lhs);
                self.output.push_str(", ");
                self.emit_expr(rhs);
                self.output.push(')');
            }
            BinOp::BitXor => {
                self.output.push_str("bit32.bxor(");
                self.emit_expr(lhs);
                self.output.push_str(", ");
                self.emit_expr(rhs);
                self.output.push(')');
            }
            BinOp::Shl => {
                self.output.push_str("bit32.lshift(");
                self.emit_expr(lhs);
                self.output.push_str(", ");
                self.emit_expr(rhs);
                self.output.push(')');
            }
            BinOp::Shr => {
                self.output.push_str("bit32.arshift(");
                self.emit_expr(lhs);
                self.output.push_str(", ");
                self.emit_expr(rhs);
                self.output.push(')');
            }
        }
    }

    fn emit_unary_op(&mut self, op: UnaryOp, operand: &Expr) {
        match op {
            UnaryOp::Neg => {
                self.output.push_str("(-");
                self.emit_expr(operand);
                self.output.push(')');
            }
            UnaryOp::Not => {
                self.output.push_str("__lsl_not(");
                self.emit_expr(operand);
                self.output.push(')');
            }
            UnaryOp::BitNot => {
                self.output.push_str("bit32.bnot(");
                self.emit_expr(operand);
                self.output.push(')');
            }
            UnaryOp::PreInc => {
                self.output.push_str("(function() ");
                self.emit_expr(operand);
                self.output.push_str(" = ");
                self.emit_expr(operand);
                self.output.push_str(" + 1; return ");
                self.emit_expr(operand);
                self.output.push_str(" end)()");
            }
            UnaryOp::PreDec => {
                self.output.push_str("(function() ");
                self.emit_expr(operand);
                self.output.push_str(" = ");
                self.emit_expr(operand);
                self.output.push_str(" - 1; return ");
                self.emit_expr(operand);
                self.output.push_str(" end)()");
            }
        }
    }

    fn emit_postfix_op(&mut self, op: PostfixOp, operand: &Expr) {
        match op {
            PostfixOp::PostInc => {
                self.output.push_str("(function() local __t = ");
                self.emit_expr(operand);
                self.output.push_str("; ");
                self.emit_expr(operand);
                self.output.push_str(" = __t + 1; return __t end)()");
            }
            PostfixOp::PostDec => {
                self.output.push_str("(function() local __t = ");
                self.emit_expr(operand);
                self.output.push_str("; ");
                self.emit_expr(operand);
                self.output.push_str(" = __t - 1; return __t end)()");
            }
        }
    }

    fn emit_assignment(&mut self, op: AssignOp, target: &Expr, value: &Expr) {
        self.emit_expr(target);
        match op {
            AssignOp::Assign => {
                self.output.push_str(" = ");
                self.emit_expr(value);
            }
            AssignOp::AddAssign => {
                self.output.push_str(" = ");
                self.emit_expr(target);
                self.output.push_str(" + ");
                self.emit_expr(value);
            }
            AssignOp::ConcatAssign => {
                self.output.push_str(" = ");
                self.emit_expr(target);
                self.output.push_str(" .. ");
                self.emit_expr(value);
            }
            AssignOp::SubAssign => {
                self.output.push_str(" = ");
                self.emit_expr(target);
                self.output.push_str(" - ");
                self.emit_expr(value);
            }
            AssignOp::MulAssign => {
                self.output.push_str(" = ");
                self.emit_expr(target);
                self.output.push_str(" * ");
                self.emit_expr(value);
            }
            AssignOp::DivAssign => {
                self.output.push_str(" = ");
                self.emit_expr(target);
                self.output.push_str(" / ");
                self.emit_expr(value);
            }
            AssignOp::ModAssign => {
                self.output.push_str(" = ");
                self.emit_expr(target);
                self.output.push_str(" % ");
                self.emit_expr(value);
            }
        }
    }

    fn emit_type_cast(&mut self, target: LslType, expr: &Expr) {
        match target {
            LslType::Integer => {
                self.output.push_str("__float2int(");
                self.emit_expr(expr);
                self.output.push(')');
            }
            LslType::Float => {
                self.output.push_str("__int2float(");
                self.emit_expr(expr);
                self.output.push(')');
            }
            LslType::String => {
                self.output.push_str("__tostring_lsl(");
                self.emit_expr(expr);
                self.output.push(')');
            }
            LslType::Key => {
                self.output.push_str("__str2key(");
                self.emit_expr(expr);
                self.output.push(')');
            }
            LslType::Vector | LslType::Rotation | LslType::List | LslType::Void => {
                self.emit_expr(expr);
            }
        }
    }

    fn emit_default_value(&mut self, ty: LslType) {
        match ty {
            LslType::Integer => self.output.push('0'),
            LslType::Float => self.output.push_str("0.0"),
            LslType::String => self.output.push_str("\"\""),
            LslType::Key => self.output.push_str("\"00000000-0000-0000-0000-000000000000\""),
            LslType::Vector => self.output.push_str("{x = 0, y = 0, z = 0}"),
            LslType::Rotation => self.output.push_str("{x = 0, y = 0, z = 0, s = 1}"),
            LslType::List => self.output.push_str("{}"),
            LslType::Void => {}
        }
    }

    fn emit_luau_string(&mut self, s: &str) {
        self.output.push('"');
        for ch in s.chars() {
            match ch {
                '"' => self.output.push_str("\\\""),
                '\\' => self.output.push_str("\\\\"),
                '\n' => self.output.push_str("\\n"),
                '\t' => self.output.push_str("\\t"),
                '\r' => self.output.push_str("\\r"),
                '\0' => self.output.push_str("\\0"),
                c => self.output.push(c),
            }
        }
        self.output.push('"');
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::parser::Parser;

    fn compile(src: &str) -> String {
        let tokens = lex(src).unwrap();
        let script = Parser::new(tokens).parse().unwrap();
        CodeGen::generate(&script)
    }

    #[test]
    fn gen_minimal_script() {
        let out = compile(r#"default { state_entry() { llSay(0, "Hello"); } }"#);
        assert!(out.contains("__current_state = \"default\""));
        assert!(out.contains("state_entry = function()"));
        assert!(out.contains("llSay(0, \"Hello\")"));
        assert!(out.contains("function __dispatch(event_name, ...)"));
    }

    #[test]
    fn gen_global_vars() {
        let out = compile(
            r#"integer g = 42; string name = "test"; default { state_entry() {} }"#,
        );
        assert!(out.contains("local g = 42"));
        assert!(out.contains("local name = \"test\""));
    }

    #[test]
    fn gen_global_function() {
        let out = compile(
            r#"integer add(integer a, integer b) { return a + b; } default { state_entry() {} }"#,
        );
        assert!(out.contains("local function add(a, b)"));
        assert!(out.contains("return (a + b)"));
    }

    #[test]
    fn gen_vector_literal() {
        let out = compile(
            r#"default { state_entry() { vector v = <1.0, 2.0, 3.0>; } }"#,
        );
        assert!(out.contains("{x = 1.0, y = 2.0, z = 3.0}"));
    }

    #[test]
    fn gen_rotation_literal() {
        let out = compile(
            r#"default { state_entry() { rotation r = <0.0, 0.0, 0.0, 1.0>; } }"#,
        );
        assert!(out.contains("{x = 0.0, y = 0.0, z = 0.0, s = 1.0}"));
    }

    #[test]
    fn gen_list_literal() {
        let out = compile(
            r#"default { state_entry() { list l = [1, "two", 3.0]; } }"#,
        );
        assert!(out.contains("{1, \"two\", 3.0}"));
    }

    #[test]
    fn gen_state_transition() {
        let out = compile(
            r#"default { touch_start(integer n) { state active; } } state active { state_entry() {} }"#,
        );
        assert!(out.contains("local __old_state = __current_state"));
        assert!(out.contains("__current_state = \"active\""));
        assert!(out.contains("__states[__old_state].state_exit"));
        assert!(out.contains("__states[__current_state].state_entry"));
    }

    #[test]
    fn gen_if_else() {
        let out = compile(
            r#"default { state_entry() { integer x = 1; if (x) { llSay(0, "yes"); } else { llSay(0, "no"); } } }"#,
        );
        assert!(out.contains("if (x) ~= 0 then"));
        assert!(out.contains("else"));
        assert!(out.contains("end"));
    }

    #[test]
    fn gen_for_loop() {
        let out = compile(
            r#"default { state_entry() { integer i; for (i = 0; i < 10; i++) { llSay(0, "loop"); } } }"#,
        );
        assert!(out.contains("i = 0"));
        assert!(out.contains("while (((i < 10) and 1 or 0)) ~= 0 do"));
    }

    #[test]
    fn gen_while_loop() {
        let out = compile(
            r#"default { state_entry() { integer x = 1; while (x) { x = 0; } } }"#,
        );
        assert!(out.contains("while (x) ~= 0 do"));
    }

    #[test]
    fn gen_do_while() {
        let out = compile(
            r#"default { state_entry() { integer x = 1; do { x = 0; } while (x); } }"#,
        );
        assert!(out.contains("repeat"));
        assert!(out.contains("until (x) == 0"));
    }

    #[test]
    fn gen_jump_label() {
        let out = compile(
            r#"default { state_entry() { jump done; @done; } }"#,
        );
        assert!(out.contains("goto __label_done"));
        assert!(out.contains("::__label_done::"));
    }

    #[test]
    fn gen_type_cast() {
        let out = compile(
            r#"default { state_entry() { string s = (string)42; } }"#,
        );
        assert!(out.contains("__tostring_lsl(42)"));
    }

    #[test]
    fn gen_member_access() {
        let out = compile(
            r#"default { state_entry() { vector v; float x = v.x; } }"#,
        );
        assert!(out.contains("v.x"));
    }

    #[test]
    fn gen_compound_assignment() {
        let out = compile(
            r#"default { state_entry() { integer x = 0; x += 5; } }"#,
        );
        assert!(out.contains("x = x + 5"));
    }

    #[test]
    fn gen_logical_operators() {
        let out = compile(
            r#"default { state_entry() { integer x = 1 && 0; integer y = 1 || 0; } }"#,
        );
        assert!(out.contains("~= 0 and"));
        assert!(out.contains("~= 0 or"));
    }

    #[test]
    fn gen_bitwise_operators() {
        let out = compile(
            r#"default { state_entry() { integer x = 3 & 1; integer y = 1 | 2; integer z = 3 ^ 1; } }"#,
        );
        assert!(out.contains("bit32.band(3, 1)"));
        assert!(out.contains("bit32.bor(1, 2)"));
        assert!(out.contains("bit32.bxor(3, 1)"));
    }

    #[test]
    fn gen_shift_operators() {
        let out = compile(
            r#"default { state_entry() { integer x = 1 << 4; integer y = 16 >> 2; } }"#,
        );
        assert!(out.contains("bit32.lshift(1, 4)"));
        assert!(out.contains("bit32.arshift(16, 2)"));
    }

    #[test]
    fn gen_unary_operators() {
        let out = compile(
            r#"default { state_entry() { integer x = -1; integer y = !0; integer z = ~0; } }"#,
        );
        assert!(out.contains("(-1)"));
        assert!(out.contains("__lsl_not(0)"));
        assert!(out.contains("bit32.bnot(0)"));
    }

    #[test]
    fn gen_pre_post_increment() {
        let out = compile(
            r#"default { state_entry() { integer x = 0; ++x; x++; } }"#,
        );
        assert!(out.contains("x = x + 1; return x end)()"));
        assert!(out.contains("local __t = x; x = __t + 1; return __t end)()"));
    }

    #[test]
    fn gen_default_values() {
        let out = compile(
            r#"default { state_entry() { integer a; float b; string c; key d; vector e; rotation f; list g; } }"#,
        );
        assert!(out.contains("local a = 0\n"));
        assert!(out.contains("local b = 0.0\n"));
        assert!(out.contains("local c = \"\"\n"));
        assert!(out.contains("local d = \"00000000-0000-0000-0000-000000000000\"\n"));
        assert!(out.contains("local e = {x = 0, y = 0, z = 0}\n"));
        assert!(out.contains("local f = {x = 0, y = 0, z = 0, s = 1}\n"));
        assert!(out.contains("local g = {}\n"));
    }

    #[test]
    fn gen_runtime_prelude_present() {
        let out = compile("default { state_entry() {} }");
        assert!(out.starts_with("-- LSL Runtime Helpers"));
        assert!(out.contains("local function __vec_add("));
        assert!(out.contains("local function __rot_mul("));
        assert!(out.contains("local function __lsl_not("));
    }

    #[test]
    fn gen_complex_script() {
        let src = r#"
integer gCount = 0;
string gName = "test";

integer factorial(integer n) {
    if (n <= 1) { return 1; }
    return n * factorial(n - 1);
}

default {
    state_entry() {
        llSay(0, "Starting");
        gCount = factorial(5);
    }
    touch_start(integer num) {
        ++gCount;
        vector pos = <1.0, 2.0, 3.0>;
        float x = pos.x;
        for (gCount = 0; gCount < 10; gCount++) {
            llSay(0, (string)gCount);
        }
        state active;
    }
}

state active {
    state_entry() {
        llSay(0, "Active");
    }
    touch_start(integer num) {
        state default;
    }
}
"#;
        let out = compile(src);
        assert!(out.contains("local gCount = 0"));
        assert!(out.contains("local gName = \"test\""));
        assert!(out.contains("local function factorial(n)"));
        assert!(out.contains("factorial((n - 1))"));
        assert!(out.contains("[\"default\"] = {"));
        assert!(out.contains("[\"active\"] = {"));
        assert!(out.contains("__current_state = \"active\""));
        assert!(out.contains("__current_state = \"default\""));
        assert!(out.contains("__tostring_lsl(gCount)"));
    }

    #[test]
    fn gen_string_escapes() {
        let out = compile(r#"default { state_entry() { string s = "hello\nworld\"test"; } }"#);
        assert!(out.contains(r#"local s = "hello\nworld\"test""#));
    }

    #[test]
    fn gen_comparison_returns_int() {
        let out = compile(
            r#"default { state_entry() { integer x = 1 == 1; } }"#,
        );
        assert!(out.contains("((1 == 1) and 1 or 0)"));
    }

    #[test]
    fn gen_empty_statement() {
        let out = compile(r#"default { state_entry() { ; } }"#);
        assert!(out.contains("state_entry = function()"));
        assert!(out.contains("end,"));
    }

    #[test]
    fn gen_return_void() {
        let out = compile(r#"default { state_entry() { return; } }"#);
        assert!(out.contains("return\n"));
    }
}

#[cfg(test)]
mod concat_tests {
    use crate::compile_lsl;

    fn lua(src: &str) -> String {
        compile_lsl(src).expect("compiles")
    }

    #[test]
    fn string_plus_string_uses_lua_concat() {
        let out = lua(r#"default { state_entry() { string s = "a" + "b"; llSay(0, "x" + s); } }"#);
        assert!(out.contains(" .. "), "expected concatenation in: {out}");
    }

    #[test]
    fn numbers_keep_arithmetic_add() {
        let out = lua("default { state_entry() { integer i = 1; i = i + 2; } }");
        assert!(out.contains(" + ") && !out.contains(" .. "), "{out}");
    }

    #[test]
    fn string_compound_add_concatenates() {
        let out = lua(r#"default { state_entry() { string s = "a"; s += "b"; } }"#);
        assert!(out.contains(" .. "), "{out}");
    }
}
