use crate::ast::*;
use crate::ast_walker::{ast_walker, AstVisitor};
use crate::consts::Const;
use crate::proto::{Proto, ProtoContext};
use crate::{debuggable, error};

pub struct Compiler {
    debug: bool,
    proto_contexts: Vec<ProtoContext>,
}

pub struct CompileError(String);
type CompileResult = Result<Proto, CompileError>;

macro_rules! compile_error {
    ($self:ident, $msg:expr) => {{
        let stat = self.current_stat();
        let error_msg = format!("[compile error] %s at line [%d].", msg, stat.source.line);
        error!($self, CompileError, error_msg)
    }};
}

pub enum Index {
    ConstIndex(u32),
    RegIndex(u32),
    None,
}

impl Compiler {
    pub fn new() -> Self {
        Compiler {
            debug: false,
            proto_contexts: Vec::new(),
        }
    }

    pub fn run(&mut self, block: &Block) -> CompileResult {
        self.main_func(block)
    }

    fn main_func(&mut self, block: &Block) -> CompileResult {
        self.push_proto();
        self.proto().open();
        ast_walker::walk_block(block, self)?;
        self.proto().close();
        Ok(self.pop_proto())
    }

    fn push_proto(&mut self) {
        self.proto_contexts.push(ProtoContext::new());
    }

    fn pop_proto(&mut self) -> Proto {
        if let Some(context) = self.proto_contexts.pop() {
            return context.proto;
        }
        unreachable!()
    }

    // get current proto ref from stack
    fn proto(&mut self) -> &mut Proto {
        &mut self.context().proto
    }

    // get current proto context
    fn context(&mut self) -> &mut ProtoContext {
        if let Some(last) = self.proto_contexts.last_mut() {
            return last;
        }
        unreachable!()
    }

    fn adjust_assign(&mut self, num_left: usize, exprs: &Vec<Expr>) -> i32 {
        let extra = num_left as i32 - exprs.len() as i32;
        if let Some(last_expr) = exprs.last() {
            if last_expr.has_mult_ret() {
                // TODO : process multi return value
                todo!("process mult ret")
            }
        }

        if extra > 0 {
            let context = self.context();
            let from = context.get_reg_top();
            context.reverse_regs(extra as u32);
            context.proto.code_nil(from, extra as u32);
        }

        extra
    }

    // process expr and return const index or register index
    fn expr(&mut self, expr: &Expr) -> Index {
        let proto = self.proto();
        match expr {
            Expr::Int(i) => {
                let k = proto.add_const(Const::Int(*i));
                Index::ConstIndex(k)
            }
            Expr::Float(f) => {
                let k = proto.add_const(Const::Float(*f));
                Index::ConstIndex(k)
            }
            Expr::String(s) => {
                let k = proto.add_const(Const::Str(s.clone()));
                Index::ConstIndex(k)
            }
            Expr::Nil => Index::None,
            Expr::True => Index::None,
            Expr::False => Index::None,
            Expr::Name(name) => {
                if let Some(src) = proto.get_local_var(name) {
                    return Index::RegIndex(src);
                }
                // TODO : process upval and globals
                todo!()
            }
            Expr::BinExpr(bin) => self.compile_expr(expr),
            _ => todo!(),
        }
    }

    fn compile_expr(&mut self, expr: &Expr) -> Index {
        if let Some(k) = self.try_const_folding(expr) {
            Index::ConstIndex(self.proto().add_const(k))
        } else {
            // TODO : generate code
            Index::None
        }
    }

    // try constant folding expr
    fn try_const_folding(&self, expr: &Expr) -> Option<Const> {
        match expr {
            Expr::Int(i) => return Some(Const::Int(*i)),
            Expr::Float(f) => return Some(Const::Float(*f)),
            Expr::BinExpr(bin) => match bin.op {
                BinOp::Add
                | BinOp::Minus
                | BinOp::Mul
                | BinOp::Div
                | BinOp::IDiv
                | BinOp::Mod
                | BinOp::Pow
                | BinOp::BAnd
                | BinOp::BOr
                | BinOp::BXor
                | BinOp::Shl
                | BinOp::Shr => {
                    if let (Some(l), Some(r)) = (
                        self.try_const_folding(&bin.left),
                        self.try_const_folding(&bin.right),
                    ) {
                        if let Some(k) = self.apply_bin_op(bin.op, l, r) {
                            return Some(k);
                        }
                    }
                }
                _ => todo!(),
            },
            Expr::ParenExpr(expr) => return self.try_const_folding(&expr),
            _ => todo!(),
        }
        None
    }

    fn apply_bin_op(&self, op: BinOp, l: Const, r: Const) -> Option<Const> {
        match op {
            BinOp::Add => l.add(r),
            BinOp::Minus => l.sub(r),
            BinOp::Mul => l.mul(r),
            BinOp::Div => l.div(r),
            BinOp::IDiv => l.idiv(r),
            BinOp::Mod => l.mod_(r),
            BinOp::Pow => l.pow(r),
            BinOp::BAnd => l.band(r),
            BinOp::BOr => l.bor(r),
            BinOp::BXor => l.bxor(r),
            BinOp::Shl => l.shl(r),
            BinOp::Shr => l.shr(r),
            _ => unreachable!(),
        }
    }

    // process expr and save to register
    fn expr_and_save(&mut self, expr: &Expr, reg: u32) {
        let index = self.expr(expr);
        let proto = self.proto();
        match index {
            Index::ConstIndex(k) => proto.code_const(reg, k),
            Index::RegIndex(src) => proto.code_move(reg, src),
            Index::None => match expr {
                Expr::Nil => proto.code_nil(reg, 1),
                Expr::True => proto.code_bool(reg, true),
                Expr::False => proto.code_bool(reg, false),
                _ => unreachable!(),
            },
        }
    }

    fn get_assinable_reg(&mut self, assignable: &Assignable) -> u32 {
        match assignable {
            Assignable::Name(name) => self.proto().get_local_var(name).unwrap(),
            Assignable::ParenExpr(expr) => todo!(),
            Assignable::SuffixedExpr(expr) => todo!(),
        }
    }

    debuggable!();
}

impl AstVisitor<CompileError> for Compiler {
    // compile local stat
    fn local_stat(&mut self, stat: &LocalStat) -> Result<(), CompileError> {
        let proto = self.proto();
        for name in stat.names.iter() {
            proto.add_local_var(name);
        }
        for expr in stat.exprs.iter() {
            let reg = self.context().reverse_regs(1);
            self.expr_and_save(expr, reg);
        }
        self.adjust_assign(stat.names.len(), &stat.exprs);
        Ok(())
    }

    // compile assign stat
    fn assign_stat(&mut self, stat: &AssignStat) -> Result<(), CompileError> {
        let last_use_temp_reg = stat.right.len() != stat.left.len();
        let mut to_move: Vec<(u32, u32)> = Vec::new();

        // normal move
        // the last right one direct move to left register
        for (i, expr) in stat.right.iter().enumerate() {
            if i != stat.right.len() - 1 || last_use_temp_reg {
                let reg = self.context().reverse_regs(1);
                self.expr_and_save(expr, reg);
                if i < stat.left.len() {
                    let target = self.get_assinable_reg(&stat.left[i]);
                    to_move.push((target, reg));
                }
            } else {
                let reg = self.get_assinable_reg(&stat.left[i]);
                self.expr_and_save(expr, reg);
            };
        }

        // nil move
        let reg = self.context().get_reg_top();
        let extra = self.adjust_assign(stat.left.len(), &stat.right);
        if extra > 0 {
            let left_start = stat.left.len() as i32 - extra;
            for i in 0..extra {
                let target = self.get_assinable_reg(&stat.left[(left_start + i) as usize]);
                let src = (reg as i32 + i) as u32;
                to_move.push((target, src));
            }
        }

        // apply moves
        for (target, src) in to_move.iter().rev() {
            self.proto().code_move(*target, *src);
            self.context().free_reg(1);
        }

        // free extra regs
        if extra < 0 {
            self.context().free_reg(-extra as u32);
        }

        Ok(())
    }
}
